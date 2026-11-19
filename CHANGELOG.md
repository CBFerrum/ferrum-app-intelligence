# Changelog

## 0.5.0

Needs the same platform as 0.4.1. No database change, no interface change.

- **How a figure changed.** A question with a change word ("compared to last month", "growth", "vs") becomes a
  change: the last two complete periods of the metric's series (by the period named, by month when none is), both
  values, the change, and the change as a percent of the earlier one to two places, in exact decimals; no percent of
  a change from zero. The period not yet over is left out and the plan says so. Kept on the Question as intent
  `{kind: change, metric, period}`. The evaluation set has one more case.

## 0.4.1

Needs the same platform as 0.4.0. No database change, no interface change.

- **Intelligence's guide.** `app.yaml` contributes `shell.assistant`: how to answer a question about a figure here
  (list the certified metrics, ask by creating a Question, read the outcome, the plan, the evidence and the limits;
  never work a figure out from raw records; propose an Example). The assistant reads it in Intelligence's pages; Ferrum's
  MCP server will serve it to agents as Intelligence's guide when its guides change lands.

## 0.4.0

Needs the same platform as 0.3.2. No database migration. Package `intelligence` 0.3.0: Intelligence User may create a
Question; Question's `question` is set once, when it is asked.

- **Asking is an operation.** Asking is making a Question (`intelligence.question.create`), through the gate with a
  receipt, so a person's screen and an agent connected over MCP do the same thing. A hook works out everything but the
  question, with the asker's own rights and the rules; values sent for anything else are ignored. `POST ask` now asks
  through that operation (an `Idempotency-Key` header keeps one question for a request sent twice) and answers as
  before, with who certified the metric read from Metrics as the asker.
- **Asking needs Intelligence User** as well as Metric Reader: someone with Metric Reader alone is refused and nothing
  is kept.
- `ask_with` and `ask_within` answer and keep nothing; they are for evaluations.

## 0.3.2

Needs the same platform as 0.3.1. No database change, no interface change. Package `intelligence` 0.2.1.

- **Accept It has a contract.** `intelligence.example.accept` says what it does (a transactional action), what it
  returns (`{accepted: <the example's id>}`) and what done means, so a process or an agent knows what it gets. It takes
  no parameters.

## 0.3.1

Needs the same platform as 0.3.0. No database change, no interface change.

- **A question with certain letters no longer stops the answer.** An accepted example was looked for in the
  lower-cased question but replaced at the same place in the question as typed; a letter whose lower case is longer or
  shorter ("İ", "ẞ") moved that place, so the wrong words were replaced or the request failed. The question is now
  searched and replaced in one text. A question is still 1 to 500 characters, counted as characters, not bytes.
- **Erasure reaches every address this app keeps.** A person's erasure now blanks `asked_by` on their questions (it
  blanked the text and the answer but left the address), and `proposed_by` and `accepted_by` on examples (not declared
  before). A kept answer no longer copies who certified the metric: that address is Metrics' to keep and to erase. The
  answer a person gets still says who certified it; the kept record names the metric, its definition's fingerprint and
  when it was certified.

## 0.3.0

Needs Ferrum 0.4.0 (apps' upkeep runs one workspace at a time, on whichever replica is free). No database or interface change.

**Upkeep is one workspace's.** `maintenance(k, tenant)` is still empty; it takes the workspace the platform hands it.

## 0.2.4

Needs the same platform as 0.2.3. No database change, no interface change.

- A proposal's description says it was proposed on the workspace's day; it named the UTC day, so east or west of UTC it
  named yesterday or tomorrow for part of every day.

## 0.2.3

Needs the same platform as 0.2.2. No database change, no change to the app: its test suite opens the vault as a server does before booting the kernel, and no longer gives the kernel the old master key.

## 0.2.2

- The app has an icon (`circle-help`) in the launcher, the home grid and the bar, where it showed a letter. Nothing else changed.

## 0.2.1

- **Stale data is said (FLC-SEM-007).** When Metrics reads a value from external datasets older than their sources allow, the answer carries those datasets in its evidence and says, in its `limits`, that it may not be today's figure.
- The README has its changelog, as the platform's app contract asks.

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
