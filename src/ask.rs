//! Answering a question: the certified metrics the asker may read, what the question is taken to mean,
//! the plan said before anything is read, the answer read through Metrics' own endpoint with the asker's
//! rights, and the evidence behind it. A question back when it is not sure, and a plain no, with why, when
//! it cannot answer: an honest abstention is an answer. Every question is kept, for the person who asked.

use crate::intent::{Intent, MAX_QUESTION, MetricInfo, Provider, Understood};
use chrono::Utc;
use ferrum_domain::util::elevated;
use ferrum_kernel::{Ctx, Doc, Error, Filter, Kernel, ListQuery, Result, SiteRequest};
use serde_json::{Map, Value, json};

/// The certified metrics `ctx` may read, by name, and what Metrics says about each.
async fn readable(k: &Kernel, ctx: &Ctx) -> Result<Vec<Doc>> {
    let q = ListQuery { filters: vec![Filter::eq("status", "Certified")], limit: Some(500), ..Default::default() };
    match k.list(ctx, "Metric", &q).await {
        Ok(page) => Ok(page.rows),
        Err(Error::Forbidden(_)) => Err(Error::Forbidden("asking needs a role that reads certified metrics (Metric Reader)".into())),
        Err(e) => Err(e),
    }
}

/// One of Metrics' endpoints, as `ctx`: the same rights a person has reading it there.
async fn metrics(k: &Kernel, ctx: &Ctx, path: &str, query: Value) -> Result<Value> {
    let site = k.hooks.site("metrics").ok_or_else(|| Error::invalid("Metrics is not installed here"))?.clone();
    let req = SiteRequest {
        tenant: ctx.tenant,
        user: Some(ctx.clone()),
        method: "GET".into(),
        path: path.into(),
        query: query.as_object().cloned().unwrap_or_default(),
        headers: Map::new(),
        body: vec![],
        client_ip: "127.0.0.1".into(),
    };
    let r = site.handle(k, req).await?;
    serde_json::from_slice(&r.body).map_err(|e| Error::internal(format!("Metrics answered with something that is not JSON: {e}")))
}

fn text(d: &Doc, k: &str) -> Option<String> {
    d.get(k).and_then(Value::as_str).map(str::to_string)
}

/// Asks, answers and keeps the question. The answer is one of `answered`, `ask_back` or `declined`.
pub async fn ask(k: &Kernel, ctx: &Ctx, provider: &dyn Provider, question: &str) -> Result<Value> {
    let question = question.trim();
    if question.is_empty() || question.chars().count() > MAX_QUESTION {
        return Err(Error::field("question", format!("is 1 to {MAX_QUESTION} characters")));
    }
    let docs = readable(k, ctx).await?;
    let infos: Vec<MetricInfo> = docs.iter().filter_map(|d| Some(MetricInfo { name: text(d, "metric_name")?, unit: text(d, "unit") })).collect();
    let understood = provider
        .understand(question, &infos)
        .unwrap_or_else(|e| Understood::Decline { reason: format!("What should understand questions could not ({}), so nothing was guessed.", e.0) });
    let (outcome, intent, plan, answer, reason) = match understood {
        Understood::AskBack { question: back, options } => {
            ("Asked Back", Value::Null, None, json!({"status": "ask_back", "question": back, "options": options}), None)
        }
        Understood::Decline { reason } => ("Declined", Value::Null, None, json!({"status": "declined", "reason": reason}), Some(reason)),
        Understood::Intent { intent, ignored } => {
            let plan = intent.plan();
            match answer_for(k, ctx, &intent, &docs).await {
                Ok(mut a) => {
                    a["plan"] = json!(plan);
                    a["understood_by"] = json!(provider.name());
                    if !ignored.is_empty() {
                        a["notes"] =
                            json!([format!("The question also asked to {}: nothing here changes anything, so only the metric was read.", ignored.join(", "))]);
                    }
                    ("Answered", intent.json(), Some(plan), a, None)
                }
                // Metrics' own refusal is the reason: no value now, deprecated, not certified.
                Err(e) => {
                    let reason = format!("{e}");
                    ("Declined", intent.json(), Some(plan), json!({"status": "declined", "reason": reason}), Some(reason))
                }
            }
        }
    };
    let record = json!({
        "question": question, "asked_by": ctx.email, "outcome": outcome, "understood_by": provider.name(),
        "intent": intent, "plan": plan, "answer": answer, "reason": reason,
    });
    k.insert(&elevated(ctx), "Question", record.as_object().cloned().unwrap_or_default()).await?;
    Ok(answer)
}

/// What Metrics says for the request, with the evidence behind it.
async fn answer_for(k: &Kernel, ctx: &Ctx, intent: &Intent, docs: &[Doc]) -> Result<Value> {
    let doc =
        docs.iter().find(|d| text(d, "metric_name").as_deref() == Some(intent.metric())).ok_or_else(|| Error::invalid("that metric is not readable now"))?;
    let evidence = json!({
        "metric": intent.metric(), "certified_by": text(doc, "certified_by"), "certified_at": text(doc, "certified_at"),
        "definition": text(doc, "certified_hash"), "lineage": text(doc, "uses"), "read_with": "your own rights to the records it counts",
    });
    match intent {
        Intent::Value { metric } => {
            let v = metrics(k, ctx, "/value", json!({"metric": metric})).await?;
            let mut evidence = evidence;
            evidence["as_of"] = v["as_of"].clone();
            Ok(json!({"status": "answered", "intent": intent.json(), "result": {"value": v["value"], "unit": v["unit"]}, "evidence": evidence}))
        }
        Intent::Series { metric, period, periods } => {
            let s = metrics(k, ctx, "/series", json!({"metric": metric, "bucket": period.word(), "periods": periods.to_string()})).await?;
            let points = s["points"].as_array().cloned().unwrap_or_default();
            let mut evidence = evidence;
            evidence["as_of"] = s["as_of"].clone();
            let chart = json!({
                "type": "line", "x": points.iter().map(|p| p["start"].clone()).collect::<Vec<_>>(),
                "y": points.iter().map(|p| p["value"].clone()).collect::<Vec<_>>(),
                "partial": points.iter().map(|p| p["partial"].clone()).collect::<Vec<_>>(),
                "unit": s["unit"],
            });
            Ok(json!({"status": "answered", "intent": intent.json(), "result": {"points": points, "unit": s["unit"]}, "chart": chart, "evidence": evidence}))
        }
    }
}

/// A proposal: a draft Solution that starts an ordinary reviewed process about a metric, kept as the
/// person who asked for it. It goes through the same compiler and review as any other draft; nothing here
/// can review, approve or activate one, and its author can never approve it.
pub async fn propose(k: &Kernel, ctx: &Ctx, metric: &str, why: &str) -> Result<Value> {
    let docs = readable(k, ctx).await?;
    if !docs.iter().any(|d| text(d, "metric_name").as_deref() == Some(metric)) {
        return Err(Error::field("metric", "is a certified metric you can read"));
    }
    let why: String = why.trim().chars().take(500).collect();
    let id = format!("workspace.look_into_{}", metric.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>());
    let source = json!({
        "schema_version": "ferrum.solution/1", "kind": "Solution", "id": id, "version": "0.1.0",
        "label": format!("Look into {metric}"), "requires": [],
        "description": format!("Proposed from Intelligence by {} on {}: {why}", ctx.email, Utc::now().date_naive()),
        "processes": [{
            "id": "look_into", "label": format!("Look into {metric}"),
            "trigger": {"event": "", "business_key": "trigger.id"}, "entry": "look", "max_steps": 20,
            "nodes": [
                {"type": "human_task", "id": "look", "label": format!("Look into {metric}"), "role": "Data Steward", "material": "trigger", "timeout": "P5D",
                 "next": "done", "on_reject": "done", "on_timeout": "attention"},
                {"type": "end", "id": "done", "label": "Looked into", "outcome": "completed"},
                {"type": "suspend", "id": "attention", "label": "Needs attention", "reason": "Nobody looked into it in time."}
            ]
        }]
    })
    .to_string();
    let draft = k.create_logic_draft(ctx, &source).await?;
    let check = match k.compile_solution(ctx, &source).await? {
        Ok(_) => json!({"ok": true}),
        Err(diagnostics) => json!({"ok": false, "diagnostics": serde_json::to_value(&diagnostics).unwrap_or_default()}),
    };
    Ok(
        json!({"draft": draft, "check": check, "next": "Choose what starts it and who looks, check it, and keep it as a release: someone else reviews it before it runs."}),
    )
}
