# Changelog

## 0.2.0

- **Bounded (FLC-AI-005).** A provider naming a metric the asker cannot read is told why and asked again, at most three times, then the question is declined. A question takes at most 20 seconds, understanding and reading together; past that it stops and reads nothing more.
- **Evaluated before it answers (FLC-AI-008).** Every provider is held to one fixed and adversarial set (`evaluate`). One that answers confidently and wrongly fails it and is not put in front of anyone: `choose` keeps the rules answering.
- **Examples (FLC-AI-007).** People propose what a phrase means here (`POST feedback`). An example is used once an Intelligence Steward other than its proposer has accepted it, and only in this workspace. It never reaches a metric its asker cannot read.
- **Old answers say so (FLC-SEM-009).** A kept answer whose metric is no longer certified as it was says it may differ if asked again.

## 0.1.1

The app's summary fits the App Manager's 200 characters, so the app is listed there again; nothing else changed.

## 0.1.0

The first release (Ferrum's Logic Platform v2, W7). Questions about certified metrics become typed requests
answered through Metrics with the asker's own rights, with the plan said first and the evidence behind the
answer; ambiguous questions are asked back, unanswerable ones declined with the reason, and words asking
for actions noticed and never obeyed. Every question is kept for its asker. Proposals become draft
Solutions for an ordinary review. Rules understand questions without a model; evaluations on fixed and
adversarial questions are in tests/evals_it.rs.
