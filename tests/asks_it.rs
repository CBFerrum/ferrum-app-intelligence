//! Asking is one operation for a person's screen and for an agent: `intelligence.question.create`, through the gate,
//! with a receipt. What is kept is worked out from the question with the asker's own rights; nothing an asker sends
//! besides the question is kept. Against a real PostgreSQL (`FERRUM_INTELLIGENCE_ASKS_TEST_DATABASE_URL`).

use ferrum_kernel::capability::Invocation;
use ferrum_kernel::{Ctx, Error, Kernel, ListQuery};
use ferrum_testkit::{Kit, app, database, obj, refused, run, uid};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::OnceCell;

static KIT: OnceCell<Kit> = OnceCell::const_new();

async fn kit() -> &'static Kit {
    KIT.get_or_init(|| async {
        let apps = [app!("metrics", ferrum_app_metrics), app!("intelligence", ferrum_app_intelligence)];
        Kit::boot(&database("FERRUM_INTELLIGENCE_ASKS_TEST_DATABASE_URL"), &apps).await
    })
    .await
}

/// A workspace with three company customers (the rep owns two), certified metrics over them, and its people.
struct World {
    k: Arc<Kernel>,
    admin: Ctx,
    director: Ctx,
    rep: Ctx,
    /// Reads certified metrics, but is not given asking.
    reader: Ctx,
    /// May ask and read metrics, but reads no customer.
    blind: Ctx,
}

async fn world(slug: &str) -> World {
    let kit = kit().await;
    let ws = kit.workspace(slug, &["metrics", "parties", "intelligence"], json!({"timezone": "UTC"})).await;
    let (k, admin) = (kit.k.clone(), ws.admin.clone());
    let rule = |p: Value| serde_json::from_value::<ferrum_meta::PermissionDef>(p).unwrap();
    k.set_role_permission(&admin, "Sales Rep", &rule(json!({"entity": "Customer", "read": true, "create": true, "if_owner": true}))).await.unwrap();
    k.set_role_permission(&admin, "Sales Director", &rule(json!({"entity": "Customer", "read": true, "create": true}))).await.unwrap();
    let at = |who: &str| format!("{who}@{slug}.test");
    let author = ws.user(&at("analyst"), &["Metric Author", "Sales Director"]).await;
    let steward = ws.user(&at("steward"), &["Data Steward", "Sales Director"]).await;
    let director = ws.user(&at("director"), &["Metric Reader", "Intelligence User", "Sales Director"]).await;
    let rep = ws.user(&at("rep"), &["Metric Reader", "Intelligence User", "Sales Rep"]).await;
    let reader = ws.user(&at("reader"), &["Metric Reader", "Sales Director"]).await;
    let blind = ws.user(&at("blind"), &["Metric Reader", "Intelligence User"]).await;
    for (who, name) in [(&rep, "Acme"), (&rep, "Globex"), (&director, "Initech")] {
        k.insert(who, "Customer", obj(json!({"customer_name": name, "customer_type": "Company", "payment_terms_days": 30}))).await.unwrap();
    }
    let made = k
        .insert(
            &author,
            "Metric",
            obj(json!({"metric_name": "Company customers", "definition": {"entity": "Customer", "filters": [["customer_type", "eq", "Company"]], "measures": [{"fn": "count"}]}, "series_by": "created_at"})),
        )
        .await
        .unwrap();
    k.run_action(&steward, "Metric", uid(&made), "certify", Value::Null).await.unwrap();
    k.insert(&author, "Metric", obj(json!({"metric_name": "Secret draft", "definition": {"entity": "Customer", "measures": [{"fn": "count"}]}})))
        .await
        .unwrap();
    World { k, admin, director, rep, reader, blind }
}

/// The screen's way: `POST /api/app/intelligence/ask`, with a key when one is given.
async fn ask_route(k: &Kernel, who: &Ctx, question: &str, key: Option<&str>) -> Result<Value, Error> {
    let site = k.hooks.site("intelligence").expect("intelligence's endpoint").clone();
    let mut headers = serde_json::Map::new();
    if let Some(key) = key {
        headers.insert("idempotency-key".into(), json!(key));
    }
    let req = ferrum_kernel::SiteRequest {
        tenant: who.tenant,
        user: Some(who.clone()),
        method: "POST".into(),
        path: "/ask".into(),
        query: Default::default(),
        headers,
        body: json!({"question": question}).to_string().into_bytes(),
        client_ip: "203.0.113.9".into(),
    };
    let r = site.handle(k, req).await?;
    Ok(serde_json::from_slice(&r.body).unwrap())
}

/// An agent's way (MCP's `create_record` maps to it): the operation itself.
fn asking(values: Value, key: &str) -> Invocation {
    Invocation::new("intelligence.question.create", json!({"values": values})).key(key)
}

async fn questions(k: &Kernel, admin: &Ctx) -> Vec<ferrum_kernel::Doc> {
    k.list(admin, "Question", &ListQuery { limit: Some(500), ..Default::default() }).await.unwrap().rows
}

fn value(answer: &Value) -> Option<&str> {
    answer["result"]["value"].as_str()
}

#[test]
fn asking_is_the_create_operation_with_a_receipt() {
    run(async {
        let w = world("receipt").await;
        let a = ask_route(&w.k, &w.director, "What is Company customers?", Some("ask-0001-receipt")).await.unwrap();
        assert_eq!(value(&a), Some("3"), "{a}");
        // The same key and question, asked as the operation: the gate answers the receipt the route's call left.
        let again = w.k.invoke(&w.director, asking(json!({"question": "What is Company customers?"}), "ask-0001-receipt")).await.unwrap();
        assert!(again.repeated, "the route asked through the gate, under its key: {again:?}");
        assert_eq!(questions(&w.k, &w.admin).await.len(), 1);
    });
}

#[test]
fn one_key_twice_keeps_one_question() {
    run(async {
        let w = world("onekey").await;
        let first = ask_route(&w.k, &w.director, "What is Company customers?", Some("ask-0002-onekey")).await.unwrap();
        let second = ask_route(&w.k, &w.director, "What is Company customers?", Some("ask-0002-onekey")).await.unwrap();
        assert_eq!(first["plan"], second["plan"]);
        refused(ask_route(&w.k, &w.director, "Company customers by month", Some("ask-0002-onekey")).await, "input changed", "");
        assert_eq!(questions(&w.k, &w.admin).await.len(), 1, "one question for one key");
        // Without a key each asking is its own question.
        ask_route(&w.k, &w.director, "What is Company customers?", None).await.unwrap();
        assert_eq!(questions(&w.k, &w.admin).await.len(), 2);
    });
}

#[test]
fn kept_fields_cannot_be_given() {
    run(async {
        let w = world("forged").await;
        let forged = json!({
            "question": "What is Company customers?", "asked_by": "someone@else.test", "outcome": "Declined", "understood_by": "a model",
            "plan": "nothing", "reason": "made up", "answer": {"status": "answered", "result": {"value": "999"}},
        });
        let made = w.k.invoke(&w.rep, asking(forged, "ask-0003-forged")).await.unwrap().result;
        assert_eq!(made["asked_by"], "rep@forged.test", "{made}");
        assert_eq!((made["outcome"].as_str(), made["understood_by"].as_str()), (Some("Answered"), Some("rules")), "{made}");
        assert_eq!(value(&made["answer"]), Some("2"), "worked out, with the rep's own rights: {made}");
        assert!(made["reason"].is_null() && made["plan"].as_str().is_some_and(|p| p.contains("your own rights")), "{made}");
        // Kept as worked out, and not changed afterwards by its asker.
        let id = uid(&made);
        assert!(
            w.k.update(&w.rep, "Question", id, obj(json!({"question": "something else"})), None).await.is_err(),
            "an asker does not change a kept question"
        );
    });
}

#[test]
fn each_person_reads_with_their_own_rights() {
    run(async {
        let w = world("rights").await;
        let rep = w.k.invoke(&w.rep, asking(json!({"question": "what is company customers"}), "ask-0004-rep")).await.unwrap().result;
        let director = w.k.invoke(&w.director, asking(json!({"question": "what is company customers"}), "ask-0004-dir")).await.unwrap().result;
        assert_eq!((value(&rep["answer"]), value(&director["answer"])), (Some("2"), Some("3")));
        // Each sees only their own questions.
        let mine = w.k.list(&w.rep, "Question", &ListQuery::default()).await.unwrap().rows;
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0]["asked_by"], "rep@rights.test");
    });
}

#[test]
fn metric_reader_without_intelligence_user_is_refused() {
    run(async {
        let w = world("reader").await;
        refused(ask_route(&w.k, &w.reader, "What is Company customers?", None).await, "forbidden", "");
        refused(w.k.invoke(&w.reader, asking(json!({"question": "What is Company customers?"}), "ask-0005-reader")).await, "forbidden", "");
        assert!(questions(&w.k, &w.admin).await.is_empty(), "nothing kept");
    });
}

/// A figure the person may not read is a refusal or a plain no, never a number, a count or a "0": a metric they cannot
/// read (a draft), and a certified metric over records they cannot read. The same for the screen and for an agent.
#[test]
fn a_person_without_the_metric_gets_a_refusal_not_a_number() {
    run(async {
        let w = world("without").await;
        for (n, (who, question)) in [(&w.rep, "what is Secret draft"), (&w.blind, "What is Company customers?")].into_iter().enumerate() {
            let screen = ask_route(&w.k, who, question, None).await;
            let agent = w.k.invoke(who, asking(json!({"question": question}), &format!("ask-0006-without-{n}"))).await.map(|r| r.result["answer"].clone());
            for got in [screen, agent] {
                match got {
                    Ok(answer) => {
                        assert_ne!(answer["status"], "answered", "{question}: {answer}");
                        assert!(answer.get("result").is_none() && !answer.to_string().contains("\"value\""), "no figure at all: {answer}");
                    }
                    Err(e) => assert!(matches!(e, Error::Forbidden(_)), "{question}: {e}"),
                }
            }
        }
    });
}

#[test]
fn an_agents_create_answers_as_the_screen_does() {
    run(async {
        let w = world("agent").await;
        let screen = ask_route(&w.k, &w.director, "company customers by month for the last 3 months", None).await.unwrap();
        let agent = w.k.invoke(&w.director, asking(json!({"question": "company customers by month for the last 3 months"}), "ask-0007-agent")).await.unwrap();
        let kept = &agent.result["answer"];
        for key in ["status", "intent", "plan", "understood_by"] {
            assert_eq!(kept[key], screen[key], "{key}: {kept} / {screen}");
        }
        assert_eq!(kept["result"]["points"].as_array().map(Vec::len), Some(3));
        assert_eq!(kept["result"]["points"], screen["result"]["points"]);
        assert_eq!(agent.result["question"], "company customers by month for the last 3 months");
    });
}

/// Moves a customer's making `months` back, as if it had been made then: what a series by `created_at` counts.
async fn made_months_ago(k: &Kernel, admin: &Ctx, name: &str, months: i32) {
    let table = k.entity(admin, "Customer").unwrap().table.clone();
    let mut tx = k.db.tenant_tx(admin.tenant).await.unwrap();
    // The table's name is the kernel's own (`EffEntity::table`), never input.
    sqlx::query(sqlx::AssertSqlSafe(format!("update {table} set created_at = created_at - make_interval(months => $2) where customer_name = $1")))
        .bind(name)
        .bind(months)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

#[test]
fn change_compares_the_last_two_complete_periods() {
    run(async {
        let w = world("change").await;
        // Two months ago: Globex (the rep's); last month: Acme (the rep's) and Initech (the director's); this month: none.
        made_months_ago(&w.k, &w.admin, "Globex", 2).await;
        made_months_ago(&w.k, &w.admin, "Acme", 1).await;
        made_months_ago(&w.k, &w.admin, "Initech", 1).await;
        let director = ask_route(&w.k, &w.director, "company customers compared to last month", None).await.unwrap();
        assert_eq!(director["intent"]["kind"], "change", "{director}");
        let r = &director["result"];
        assert_eq!((r["from"]["value"].as_str(), r["to"]["value"].as_str()), (Some("1"), Some("2")), "{director}");
        assert_eq!((r["change"].as_str(), r["change_percent"].as_str()), (Some("1"), Some("100")), "{director}");
        assert!(director["plan"].as_str().unwrap().contains("compare the last two complete months"), "{director}");
        // The rep's own rights: one of theirs each month, no change.
        let rep = w.k.invoke(&w.rep, asking(json!({"question": "company customers compared to last month"}), "ask-0008-change")).await.unwrap().result;
        assert_eq!((rep["answer"]["result"]["change"].as_str(), rep["answer"]["result"]["change_percent"].as_str()), (Some("0"), Some("0")), "{rep}");
    });
}
