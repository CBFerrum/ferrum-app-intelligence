# Intelligence

Questions about certified metrics, in plain words, for [Ferrum](https://github.com/CBFerrum/Ferrum): answered
with the asker's own rights, with the plan followed and the evidence behind it; a question back when it is
not sure which metric was meant; a plain no, with why, when it cannot answer; and proposals that become
draft processes someone else reviews.

| | |
|---|---|
| 💬 App | `intelligence`, native, in the server |
| 🧩 Package | `intelligence` (needs `metrics`) |
| 🔌 Endpoints | `POST /api/app/intelligence/ask` `{question}`, `GET questions`, `POST propose` `{metric, why}` |
| 🔐 Role | Intelligence User (with Metric Reader to read metrics) |
| 📜 Licence | AGPL-3.0-only |

## How it answers

- **A typed request, never a query.** A question becomes a request about the certified metrics the asker
  may read, and only what Metrics can do with them: a value, or a value period by period. Nothing in a
  question becomes a query, a filter or an action.
- **The plan first, then the answer, then the evidence.** The answer is read through Metrics' own endpoint
  with the asker's rights, so two people may get two numbers from one question. It carries who certified the
  metric and when, its definition's fingerprint, its lineage, and when it was worked out; a series marks the
  period not yet over as partial. A value Metrics read from external data older than its source allows
  carries those datasets in its evidence and says, in its `limits`, that it may not be today's figure.
- **Not sure, it asks; cannot, it says.** Two metrics a question could mean are offered back; a question
  naming no metric the asker can read, a range too long to read as asked, a metric deprecated since or one
  without a value now is declined with the reason. An honest abstention is an answer.
- **Words that ask for actions are noticed, never obeyed.** "Then delete every customer" or "ignore your
  instructions" leaves only the metric read, and the answer says the rest was not acted on.
- **Every question is kept** for the person who asked: what it was taken to mean, the plan, the answer or
  the reason.
- **Proposals are drafts.** `propose` keeps a draft Solution about a metric as the person asking (a Solution
  Author), checked by the same compiler as any other. Nothing here reviews, approves or activates it, and
  its author never can.

## Understanding questions

`Rules`, in this app, understands questions without a model: a metric by its name, a period by its word
("by month", "weekly", "over time"), a stretch by a number ("last 6 months"). A model provider plugs in
behind the same `Provider` trait and is held to the same typed request; none is configured here, because no
model key is (Ferrum's docs/logic, BLOCK-MODEL-KEY). A provider that fails makes the answer a plain no:
nothing is guessed.

## Evaluations

`tests/evals_it.rs` runs fixed questions (values, per-person rights, series) and adversarial ones (injected
instructions, actions asked for, ranges too long, ambiguity, metrics hidden or revoked, a part of a composed
metric gone, a failing provider, readers without the role) against a real database.

## Changelog

| Version | Highlights |
|---|---|
| **0.2.2** | The app has an icon in the launcher and the bar. |
| **0.2.1** | An answer read from external data older than its source allows says so (`limits`, and the datasets in its evidence). |
| **0.2.0** | A bounded understanding loop, providers evaluated before they answer, examples accepted by someone else, and old answers that say so. |
| **0.1.1** | The summary fits the App Manager's 200 characters. |
| **0.1.0** | Questions about certified metrics answered with the asker's own rights, with the plan and the evidence. |
