//! Intelligence (Ferrum's Logic Platform, W7): questions about certified metrics, in plain words.
//!
//! * `intent`: a question becomes a typed request about the certified metrics the asker may read (a
//!   value, or a series), a question back, or a plain no. Nothing in a question becomes a query or an
//!   action. [`Rules`] understands questions without a model; a model provider is held to the same type.
//! * `ask`: the plan said before anything is read, the answer read through Metrics' own endpoint with the
//!   asker's rights, the evidence behind it (who certified the metric, its lineage, when it was worked
//!   out), and every question kept for the person who asked; a proposal kept as a draft Solution for an
//!   ordinary review, never approved or activated here.
//! * `site`: `/api/app/intelligence/ask`, `questions` and `propose`.
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
}

/// This app's reports: none.
pub fn reports() -> Vec<Arc<dyn ReportSource>> {
    vec![]
}

/// Upkeep: none yet.
pub async fn maintenance(_k: &Kernel) -> Result<()> {
    Ok(())
}

/// Asks as `ctx` with a given provider: for a host that sets one up, and for evaluations.
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
