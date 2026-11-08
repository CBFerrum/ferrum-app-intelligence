//! Answering a question: the certified metrics the asker may read, what the question is taken to mean,
//! the plan said before anything is read, the answer read through Metrics' own endpoint with the asker's
//! rights, and the evidence behind it. A question back when it is not sure, and a plain no, with why, when
//! it cannot answer: an honest abstention is an answer. Every question is kept, for the person who asked.

use crate::intent::{Intent, MAX_QUESTION, MetricInfo, Provider, ProviderError, Understood};
use chrono::Utc;
use ferrum_domain::util::elevated;
use ferrum_kernel::{Ctx, Doc, DocHook, Error, Filter, HookCtx, Kernel, ListQuery, Result, SiteRequest};
use serde_json::{Map, Value, json};
use std::time::Duration;

/// The certified metrics `ctx` may read, by name, and what Metrics says about each.
async fn readable(k: &Kernel, ctx: &Ctx) -> Result<Vec<Doc>> {
    let q = ListQuery { filters: vec![Filter::eq("status", "Certified")], limit: Some(500), ..Default::default() };
    match k.list(ctx, "Metric", &q).await {
        Ok(page) => Ok(page.rows),
        Err(Error::Forbidden(_)) => Err(Error::Forbidden("asking needs a role that reads certified metrics (Metric Reader)".into())),
        Err(e) => Err(e),
    }
}

/// [`readable`], for the site.
pub(crate) async fn readable_for(k: &Kernel, ctx: &Ctx) -> Result<Vec<Doc>> {
    readable(k, ctx).await
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
/// Longest a question may take, understanding and reading together (FLC-AI-005).
pub const ASK_SECS: u64 = 20;
/// Most times a provider is asked about one question: once, and twice again when what it said cannot be used.
pub const MAX_TRIES: usize = 3;

pub async fn ask(k: &Kernel, ctx: &Ctx, provider: &dyn Provider, question: &str) -> Result<Value> {
    ask_within(k, ctx, provider, question, Duration::from_secs(ASK_SECS)).await
}

/// What the provider makes of the question (FLC-AI-005): asked again, at most [`MAX_TRIES`] in all, when it names
/// a metric the asker cannot read; still unusable after that, the question is declined, never guessed at.
async fn understand(provider: &dyn Provider, question: &str, infos: &[MetricInfo]) -> Understood {
    let unusable = |said: &std::result::Result<Understood, ProviderError>| match said {
        Ok(Understood::Intent { intent, .. }) if !infos.iter().any(|m| m.name == intent.metric()) => {
            Some(format!("there is no certified metric this person can read called {}", intent.metric()))
        }
        _ => None,
    };
    let mut said = provider.understand(question, infos).await;
    let mut tries = 1;
    while let Some(problem) = unusable(&said) {
        if tries >= MAX_TRIES {
            return Understood::Decline { reason: format!("What it understood named no metric you can read, after {MAX_TRIES} tries, so nothing was read.") };
        }
        tries += 1;
        said = provider.repair(question, infos, &problem).await;
    }
    said.unwrap_or_else(|e| Understood::Decline { reason: format!("What should understand questions could not ({}), so nothing was guessed.", e.0) })
}

/// The workspace's accepted examples, phrase and the metric it means (FLC-AI-007): proposed by whoever asks, used
/// only once someone other than the proposer has accepted it, and only here: examples are the workspace's records.
async fn accepted_examples(k: &Kernel, ctx: &Ctx) -> Result<Vec<(String, String)>> {
    let q = ListQuery { filters: vec![Filter::eq("status", "Accepted")], limit: Some(200), ..Default::default() };
    let rows = k.list(&elevated(ctx), "Example", &q).await?.rows;
    Ok(rows.iter().filter_map(|r| Some((text(r, "phrase")?.to_lowercase(), text(r, "metric")?))).collect())
}

/// The question with each accepted example's phrase replaced by the metric it means, where the asker may read that
/// metric: an example never reaches what its asker cannot. What was used, for the record.
fn with_examples(question: &str, examples: &[(String, String)], infos: &[MetricInfo]) -> (String, Vec<String>) {
    let mut q = question.to_string();
    let mut used = vec![];
    for (phrase, metric) in examples {
        if phrase.trim().is_empty() || !infos.iter().any(|m| &m.name == metric) {
            continue;
        }
        if let Some(at) = q.to_lowercase().find(phrase.as_str()) {
            q.replace_range(at..at + phrase.len(), metric);
            used.push(format!("{phrase} means {metric}"));
        }
    }
    (q, used)
}

/// [`ask`] within `budget`: past it, what was under way is dropped and the question declined (FLC-AI-005).
pub async fn ask_within(k: &Kernel, ctx: &Ctx, provider: &dyn Provider, question: &str, budget: Duration) -> Result<Value> {
    let question = question.trim();
    if question.is_empty() || question.chars().count() > MAX_QUESTION {
        return Err(Error::field("question", format!("is 1 to {MAX_QUESTION} characters")));
    }
    let docs = readable(k, ctx).await?;
    let infos: Vec<MetricInfo> = docs.iter().filter_map(|d| Some(MetricInfo { name: text(d, "metric_name")?, unit: text(d, "unit") })).collect();
    let examples = accepted_examples(k, ctx).await?;
    let (asked, used) = with_examples(question, &examples, &infos);
    let work = async {
        let understood = understand(provider, &asked, &infos).await;
        respond(k, ctx, provider, understood, &docs).await
    };
    let (outcome, intent, plan, mut answer, reason) = match tokio::time::timeout(budget, work).await {
        Ok(done) => done,
        Err(_) => {
            let reason = format!("It did not finish within {} s, so it stopped and read nothing more.", budget.as_secs());
            ("Declined", Value::Null, None, json!({"status": "declined", "reason": reason}), Some(reason))
        }
    };
    if !used.is_empty() {
        answer["examples_used"] = json!(used);
    }
    let record = json!({
        "question": question, "asked_by": ctx.email, "outcome": outcome, "understood_by": provider.name(),
        "intent": intent, "plan": plan, "answer": answer, "reason": reason,
    });
    k.insert(&elevated(ctx), "Question", record.as_object().cloned().unwrap_or_default()).await?;
    Ok(answer)
}

type Responded = (&'static str, Value, Option<String>, Value, Option<String>);

/// What comes of what was understood: an answer read through Metrics, a question back, or a plain no.
async fn respond(k: &Kernel, ctx: &Ctx, provider: &dyn Provider, understood: Understood, docs: &[Doc]) -> Responded {
    match understood {
        Understood::AskBack { question: back, options } => {
            ("Asked Back", Value::Null, None, json!({"status": "ask_back", "question": back, "options": options}), None)
        }
        Understood::Decline { reason } => ("Declined", Value::Null, None, json!({"status": "declined", "reason": reason}), Some(reason)),
        Understood::Intent { intent, ignored } => {
            let plan = intent.plan();
            match answer_for(k, ctx, &intent, docs).await {
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
    }
}

/// Whether a kept answer still stands (FLC-AI-005's sibling, FLC-SEM-009): answered from a certified definition, it
/// says so when that metric is no longer certified as it was, so an old answer is never read as today's.
pub(crate) fn staleness(question: &Doc, docs: &[Doc]) -> Option<String> {
    let answer = question.get("answer")?;
    let metric = answer.get("evidence")?.get("metric")?.as_str()?;
    let was = answer.get("evidence")?.get("definition")?.as_str()?;
    match docs.iter().find(|d| text(d, "metric_name").as_deref() == Some(metric)) {
        Some(d) if text(d, "certified_hash").as_deref() == Some(was) => None,
        Some(_) => Some(format!("Answered under an earlier certified definition of {metric}: asked again, it may differ.")),
        None => Some(format!("{metric} is not certified as it was when this was answered: asked again, it may differ, or not be answered.")),
    }
}

/// An example a person proposes, and its acceptance by someone else (FLC-AI-007).
pub struct ExampleHook;

#[async_trait::async_trait]
impl DocHook for ExampleHook {
    fn name(&self) -> &'static str {
        "intelligence.example"
    }

    fn fills(&self) -> &'static [&'static str] {
        &["proposed_by", "status", "accepted_by"]
    }

    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        if text(doc, "proposed_by").is_none() && !hc.ctx.email.is_empty() {
            doc.insert("proposed_by".into(), json!(hc.ctx.email.to_lowercase()));
            doc.insert("status".into(), json!("Proposed"));
        }
        Ok(())
    }

    // What was accepted is what is used: an accepted example is changed by proposing another.
    async fn validate_change(&self, _: &mut HookCtx<'_>, old: Option<&Doc>, doc: &Doc) -> Result<()> {
        if let Some(o) = old
            && text(o, "status").as_deref() == Some("Accepted")
            && (text(o, "phrase") != text(doc, "phrase") || text(o, "metric") != text(doc, "metric"))
        {
            return Err(Error::Conflict("an accepted example is not changed: propose another".into()));
        }
        Ok(())
    }

    async fn action(&self, hc: &mut HookCtx<'_>, name: &str, doc: &Doc, _: &Value) -> Result<Option<Value>> {
        if name != "accept" {
            return Ok(None);
        }
        if text(doc, "proposed_by").as_deref() == Some(hc.ctx.email.to_lowercase().as_str()) {
            return Err(Error::Forbidden("someone other than its proposer accepts an example".into()));
        }
        let id = ferrum_domain::util::id(doc, "id").ok_or_else(|| Error::internal("an example without an id"))?;
        let values = json!({"status": "Accepted", "accepted_by": hc.ctx.email.to_lowercase()});
        hc.kernel.set_values_tx(&elevated(hc.ctx), hc.tx, "Example", id, values.as_object().cloned().unwrap_or_default()).await?;
        Ok(Some(json!({"accepted": id})))
    }
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
