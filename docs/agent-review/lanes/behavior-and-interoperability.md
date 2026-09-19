# Lane: Behavior and interoperability

**Optional.** Use when the task specifies behavior, real callers, persistence, or
save/UI contracts.

## One question

Does this implement the task's full specified behavior through the required
shared operations?

## Basis required for an actionable finding

Show at least one concrete instance:

- a missing branch or unhandled case;
- a no-op or unconditional success;
- an input that is accepted and then ignored;
- a caller that is declared or named but never connected;
- an incompatible state contract between two owners;
- a donor-semantic mismatch against the referenced predecessor source.

Name the file and line for the gap, and the caller, state, or donor symbol that
proves it matters.

## Not a finding

- A passing demonstration as evidence of completeness. A demo that works does not
  close this lane.
- Behavior the task did not specify. That belongs to the requirement lane, or to
  a follow-up task.
- A caller that does not exist yet, when the task explicitly lands a prerequisite
  primitive before its consumer. A primitive may precede its consumer; it may not
  be a stub, a hardcoded example, or a partial adapter.
- Broader interactive or browser evaluation as a completion requirement.
