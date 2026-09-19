# Agent review workflow

Status: active convention, 2026-09-19. The files here are the packets
handed to reviewers; this directory owns them. The `drift` agent preset
owns route pinning (`subagent_review` pinned fresh-context route,
`subagent_audit` inherits the parent route). Where this directory
disagrees with a Den project document, Den wins.

Reviewers are persistent agents, not one-shot checks. An identified issue is
re-checked by the same reviewer in the same session, so round two verifies the
fix instead of rediscovering the problem from a blank context.

This is not an approval system and not an interactive gate. Reviewers report
source-backed findings; the root agent reconciles them and decides. It does
not replace the managed Den review path where a task requires one.

## Lane roster

Two lanes run on **every** task:

| Lane | Packet |
| --- | --- |
| Existing capability reuse — intra-Engine reinvention | [lanes/existing-capability-reuse.md](lanes/existing-capability-reuse.md) |
| Existing owner reuse — repo-local reinvention | [lanes/existing-owner-reuse.md](lanes/existing-owner-reuse.md) |

Both are always on for the same reason: agents skip capabilities that already
exist. New code gets written for something an existing Engine crate or service
already guarantees, or for something this repository already owns, instead of
extending it. Upstream the cost is higher than downstream: a duplicate here
becomes a second authority every product must reconcile with.

Optional lanes. Pick to a total of two to four reviewers, and pick lanes whose
questions can disagree with each other:

| Lane | Use when |
| --- | --- |
| [Product/Engine boundary](lanes/product-engine-boundary.md) | the change crosses the product/Engine seam, adds tuning, or touches gameplay vocabulary |
| [Behavior and interoperability](lanes/behavior-and-interoperability.md) | the task specifies behavior, real callers, persistence, or save/UI contracts |
| [Requirement and acceptance](lanes/requirement-and-acceptance.md) | the task carries explicit acceptance criteria |
| [Error and boundary paths](lanes/error-and-boundary-paths.md) | the change adds parsing, input handling, or failure paths |
| [Test claims](lanes/test-claims.md) | the change adds or edits tests, or claims verification |

Do not open a lane that repeats another lane's question in different words. Do
not run the full roster to be safe: a trivial task is two reviewers, and a task
that changes a boundary, a save contract, or an ownership seam is four.

## Choosing the reviewer tool

| Tool | Context | Use for |
| --- | --- | --- |
| `subagent_review` | fresh; does not see the conversation | adversarial and requirement lanes, where anchoring on the root agent's reasoning would weaken the check |
| `subagent_audit` | inherits the root agent's completed turns | lanes that need the change's rationale — reuse, boundary, interoperability |

Open every reviewer for a round in one message, one reviewer per lane, and keep
working while they run. Their reports arrive as settlement notices. Do not wait
and do not poll.

Give a fresh reviewer everything it needs: repository path, the exact artifact
under review, and the command that demonstrates the behavior. Name the lane
packet explicitly in the prompt; a reviewer gets the generic packet plus its own
lane file, never the other lanes'.

## Revision rounds

When a round's findings are addressed, `send_message` the same reviewer. State
what changed, what was deliberately left alone and why, and which finding ids to
re-check. The reviewer keeps its own history, so it can judge whether the fix
resolved the issue it actually raised.

Never open a new reviewer for a round of work an existing reviewer has already
seen; that discards the continuity that makes the second pass worth reading. A
settled reviewer is still a valid `send_message` target. Recover ids with
`list_agents` after a long session, and use `interrupt_agent` on a reviewer whose
lane no longer matters.

Open a fresh reviewer only when the revision is large enough that the old
reviewer's accumulated position is itself a source of bias, and say so when you
do.

## Authority on disagreement

Findings are claims to verify, not instructions to apply.

- A factual dispute is settled with evidence and a re-check in the reviewer's own
  session. That exchange is the point of persistence.
- A scope dispute is not the reviewer's call. The task's stated contract and the
  user decide; the root records the disposition and the reason.
- A reviewer's verdict never amends user intent, and an unresolved finding is
  never silently dropped — it is deferred explicitly, or declined with a reason.

## What reviewers must not report

Stylistic preferences, new scope, broad redesign proposals, invented acceptance
criteria, or interactive gates. A finding that is not backed by a file and line,
command output, or a command that reproduces it does not belong in the report.
