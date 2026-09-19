# Lane: Test claims

**Optional.** Use when the change adds or edits tests, or claims verification.

## One question

Do the tests assert the behavior the change claims, and would they fail if that
behavior regressed?

## Basis required for an actionable finding

Name all three:

1. the claim being made — from the task, the commit message, or the handoff;
2. the test and assertion that is supposed to establish it, with file and line;
3. the specific regression that would still pass.

For a missing assertion, state the mutation — the concrete edit to the
implementation that the suite would not notice.

## What to probe

- An assertion that mirrors the implementation's branches instead of exposing a
  meaningful mistake.
- A test that passes for the wrong reason: an early return, a skipped body, a
  catch that swallows, or a fixture that makes the path unreachable.
- A claim established by a passing demonstration rather than by an assertion.
- A retired-path test kept green by retaining production behavior nobody wants.
- A test whose name states more than its assertions check.

## Not a finding

- Low coverage in general, or missing tests for code this change did not touch.
- A request for a different test framework, layout, or naming style.
- Absent broad integration or browser suites. Focused compilation and semantic
  checks appropriate to the change are the standard here.
