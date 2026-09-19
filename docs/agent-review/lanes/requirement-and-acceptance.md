# Lane: Requirement and acceptance

**Optional.** Use when the task carries explicit acceptance criteria.

## One question

Does the artifact deliver every clause of the task's stated contract?

## Basis required for an actionable finding

For each gap, give all three:

1. the clause, quoted from the task description or acceptance list;
2. the current state of the artifact, with file and line;
3. what is missing or contradicted.

Work clause by clause. A task with six acceptance clauses gets six verdicts, even
when five of them pass. State which clauses you verified as satisfied and how, so
the root agent can tell a checked clause from an unchecked one.

## Not a finding

- A requirement the task does not state. Ambition is not a defect; new behavior
  belongs in a new task.
- A superseded clause, when the task or a user correction explicitly replaces it.
- A wording preference about the task description itself.
- An acceptance criterion the change satisfies through a different mechanism than
  the description implied, when the delivery method was not itself required.

## Note

When a clause is ambiguous, say what the ambiguity is and which reading you
applied. Do not resolve it silently in either direction.
