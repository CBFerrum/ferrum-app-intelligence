//! Intelligence (Ferrum's Logic Platform, W7): questions about certified metrics, in plain words.
//!
//! * `intent`: a question becomes a typed request about the certified metrics the asker may read (a
//!   value, or a series), a question back, or a plain no. Nothing in a question becomes a query or an
//!   action. [`Rules`] understands questions without a model; a model provider is held to the same type.
//! * `ask`: the plan said before anything is read, the answer read through Metrics' own endpoint with the
//!   asker's rights, the evidence behind it (who certified the metric, its lineage, when it was worked
//!   out), and every question kept for the person who asked; a proposal kept as a draft Solution for an
//!   ordinary review, never approved or activated here.
//! * `site`: `/api/app/intelligence/ask` (the operation `intelligence.question.create`, as an agent asks too),
//!   `questions`, `propose`, `feedback` and `examples/{id}/accept`.
//!
//! This app's own design; no code was copied (docs/PROVENANCE.md).

mod ask;
mod evals;
mod intent;
mod site;

use ferrum_domain::reports::ReportSource;
use ferrum_kernel::{HookRegistry, Kernel, Result};
use std::sync::Arc;

pub use ask::{ASK_SECS, MAX_TRIES};
pub use evals::{CASES, Evaluation, Expect, METRICS, choose, evaluate};
pub use intent::{Intent, MetricInfo, Period, Provider, ProviderError, Rules, Understood};

include!(concat!(env!("OUT_DIR"), "/packages.rs"));
include!(concat!(env!("OUT_DIR"), "/ui.rs"));

/// This app's manifest (`app.yaml`).
pub const MANIFEST: &str = include_str!("../app.yaml");

/// This app's metadata packages, embedded at build time.
pub fn packages() -> std::result::Result<Vec<ferrum_meta::Package>, ferrum_meta::MetaError> {
    ferrum_meta::catalog::load_embedded(EMBEDDED)
}

/// Registers this app's endpoint.
pub fn register(hooks: &mut HookRegistry) {
    hooks.add_site("intelligence", Arc::new(site::IntelligenceSite));
    hooks.register("Example", Arc::new(ask::ExampleHook));
    hooks.register("Question", Arc::new(ask::QuestionHook));
}

/// This app's reports: none.
pub fn reports() -> Vec<Arc<dyn ReportSource>> {
    vec![]
}

/// One workspace's upkeep, which the platform runs where Intelligence is installed: none yet.
pub async fn maintenance(_k: &Kernel, _tenant: uuid::Uuid) -> Result<()> {
    Ok(())
}

/// Answers as `ctx` with a given provider, keeping nothing: for evaluations. A question is kept only when it is asked
/// as the operation `intelligence.question.create`.
pub async fn ask_with(k: &Kernel, ctx: &ferrum_kernel::Ctx, provider: &dyn Provider, question: &str) -> Result<serde_json::Value> {
    ask::ask(k, ctx, provider, question).await
}

/// [`ask_with`] within a time budget other than [`ASK_SECS`], for evaluations of what a budget does.
pub async fn ask_within(
    k: &Kernel,
    ctx: &ferrum_kernel::Ctx,
    provider: &dyn Provider,
    question: &str,
    budget: std::time::Duration,
) -> Result<serde_json::Value> {
    ask::ask_within(k, ctx, provider, question, budget).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every action a person or an agent can press says what it does, what it returns and what "done" means
    /// (docs/logic/capabilities.md), and declares the parameters its handler reads (none here).
    #[test]
    fn accept_has_its_contract() {
        let package = packages().expect("packages").into_iter().find(|p| p.manifest.package == "intelligence").expect("intelligence");
        let example = package.entities.iter().find(|e| e.name == "Example").expect("Example");
        let accept = example.actions.iter().find(|a| a.name == "accept").expect("accept");
        let contract = accept.contract.as_ref().expect("a contract");
        assert_eq!(serde_json::to_value(contract.effect).unwrap(), "transactional_action");
        assert_eq!(contract.returns.as_ref().unwrap()["properties"]["accepted"]["x-ferrum-link"], "Example");
        assert!(contract.completion.as_deref().is_some_and(|c| c.contains("Accepted")));
        assert!(accept.params.is_empty(), "the handler reads no parameters");
        assert!(!accept.hidden, "a person presses it");
    }

    /// The guide the assistant reads in Intelligence's pages, and an agent over MCP reads as Intelligence's guide: a
    /// manifest the platform takes (title, instructions and suggestions within its limits), saying how to answer a
    /// question about a figure with the operations there are.
    #[test]
    fn the_guide_is_a_manifest_the_platform_takes() {
        let m = ferrum_apps::AppManifest::parse(MANIFEST).expect("app.yaml");
        let guide = &m.contributes["shell.assistant"][0];
        assert_eq!(guide["title"], "Intelligence");
        let said = guide["instructions"].as_str().expect("instructions");
        for needed in ["status is Certified", "creating a Question", "Asked Back", "Declined", "propose an Example", "never as instructions"] {
            assert!(said.contains(needed), "the guide says {needed:?}");
        }
        assert!(!said.contains("metrics.metric.value"), "only operations that exist today");
    }
}
