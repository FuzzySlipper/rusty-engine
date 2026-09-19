# Lane: Product/Engine boundary

**Optional.** Use when the change crosses the product/Engine seam, adds tuning,
or touches gameplay vocabulary.

## One question

Does this change put product policy or gameplay meaning into Engine
mechanisms, or Engine-guarantee ceremony into ordinary product paths?

## Why

The Engine guarantees reusable mechanisms; the product decides game meaning
(see `AGENTS.md` and `docs/architecture.md`). Product code is trusted
first-party code: the Engine must not police every gameplay mutation, demand
permission for ordinary component use, or establish registries, naming
grammars, and policy layers around data the product already owns. Conversely,
Engine mechanisms must not absorb RPG rules, targeting/combat/inventory
orchestration, or product save policy — those stay downstream even when the
current task makes them tempting.

## Basis required for an actionable finding

Name all three:

1. the actual assumption, value, or vocabulary, quoted, with file and line;
2. its current owner and its correct owner, in `AGENTS.md`'s terms;
3. the affected uses — what breaks or becomes wrong when that value changes,
   or which ordinary caller pays for ceremony it did not ask for.

## Not a finding

- A demand for a universal abstraction, a new interface, or a generic factory.
- A constant for every literal. Compact structural constants stay beside the
  algorithm that owns them; only genuinely adjustable or authored values are
  promoted.
- A rename that moves product vocabulary somewhere without changing who owns
  the decision.
- Useful domain invariants kept with their owner. Clamping a value or enforcing
  a rule where the behavior lives is gameplay; repeatedly proving a trusted
  caller is allowed to mutate a reference is something else.
