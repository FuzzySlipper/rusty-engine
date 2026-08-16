# Gameplay resolution two-beat overfitting report

Status: accepted provider shape after task 7032's Dagger and Doom consumers.

## Evaluated revisions

- Rusty Engine: `ea15f9a617f9f099655664cfe4c73ee4817a1de5`
- Rusty Dagger: `fbe5885b21257f9b4844131e56f7a7920a774cf5`
- Rusty Engine Demo: `e41297ccb4a9ec88909148b03b2ec533746035c0`

All three were developed and landed directly on `main`. The Engine foundation
landed first, then each downstream used the ordinary adjacent facade while the
public shape was revised.

## Boundary held before Beat 1

Engine owns the bounded attempt lifecycle, structural sequencing and
conditionals, phase/interceptor order, correlation and causation, quotas,
preview/apply, one downstream transaction boundary, and generic receipts and
traces. It does not know any gameplay noun or verb and has no authority over a
game's state, randomness, scheduling, persistence, presentation, or authoring
language.

Each game owns intent admission, fact gathering, predicates, opaque
operations, interceptors, evidence meaning, effects, semantic events,
transaction binding, errors, and all domain vocabulary. `gameplay-rules`
remains only the opaque package envelope; `gameplay-mechanics` remains a set of
leaf mechanisms; `entity-state` and downstream session types retain mutation
authority.

No implementation evidence moved a gameplay concept across that line.

## Beat 1: authored Dagger policy

Rusty Dagger exercises an authored spell-shaped resolution:

- TypeScript builders emit a deterministic `gameplay-rules` package; no Node
  or TypeScript runs at gameplay time.
- Dagger Rust semantically admits the payload into its own action, predicate,
  target-selector, operation, rule, item-interceptor, effect, and event types.
- Ember Lance supplies hit evidence, spends magicka, and damages the intent
  target. Ruby Ward contributes an ordered damage-reduction interceptor.
  Silence rejects a tagged action from authoritative actor conditions.
- Player and AI origins use the same policy and transaction path.
- Preview/apply, rejected attempts, staged mutation, semantic events, and the
  phase trace are visible through the Dagger diagnostic readout.

This path is catalog-driven, definition-oriented, and deliberately expressive
at the TypeScript authoring edge.

## Beat 2: realtime Doom hitscan

Rusty Engine Demo exercises its existing single-ray hitscan and automatic-fire
path. The fixed combat phase still validates the actor, weapon, ammo, cooldown,
pose, and timing. Loading Bay Rust precomputes one typed spatial hit,
world-block, or no-target evidence record from the live collision projection.
Its policy then uses Engine `Sequence` and `When` nodes to order fired, ammo,
hit/miss, damage, and cooldown operations. Its transaction clones and publishes
the candidate `GameSession` once, preserving the existing facts, game events,
death/drop consequences, and failure behavior.

This path is realtime, pose- and collision-dependent, immediate, and integrated
with an existing fixed tick and product loop. It has no authored spell graph,
stats, conditions, target collection, or Dagger type. Spread and projectile
weapons intentionally remain on their existing paths; this task needed one
bounded mechanically different action, not a combat rewrite.

## Public concepts retained

- `ResolutionRequest`, mode, identity, correlation, and causation.
- `ResolutionPolicy` admission, gather, check, plan, interceptor, predicate,
  operation, and before-commit borders.
- `Program::Sequence`, `Program::When`, and opaque `Program::Operation`.
  Both consumers exercise sequencing and conditional traversal without sharing
  operation vocabulary.
- Explicit evidence retained in the attempt receipt.
- `ResolutionTransaction` stage/commit/abort and one commit attempt.
- Bounded child resolutions and suspensions, which remain lifecycle mechanisms
  rather than a scheduler.
- Generic attempt/receipt/trace structures and deterministic phase records.
- Resolver replacement as a trait border; neither consumer needed to replace
  `StandardResolver`.

## Concepts removed or moved downstream

The Doom proof showed that Engine-owned target collection was a Dagger-shaped
assumption, not shared lifecycle structure. The final provider therefore
removed:

- `Program::ForEach`;
- the `Selector` and `Subject` policy associated types;
- the required `select` policy method;
- selected-subject quotas and `SubjectsSelected` structural trace records; and
- subject parameters on predicate and operation hooks.

Dagger still has an inspectable `intentTarget` authoring selector, but it is
compiled inside the opaque Dagger damage operation and resolved by Dagger
policy. Doom is not forced to manufacture target-selector types. This reduced
the provider by more than one hundred lines and made collection bounds the
responsibility of whichever downstream operation actually owns a collection.

The Doom transaction also forced one additive receipt change:
`ResolutionReceipt::into_commit`. Diagnostics could inspect a borrowed commit
status, but production code needed to recover a non-`Clone` downstream error by
value and return it through Loading Bay's existing `RuntimeError` path.

No consumer-specific mode flag, callback registry, or vocabulary was added.

## Assumptions not challenged by both consumers

- Dagger and the headless contract fixture exercise interceptors; Doom does
  not currently need one.
- The headless contract fixture exercises child resolutions and suspensions;
  neither production slice requires them yet.
- Dagger exercises preview; the shipped Doom attack uses apply only.
- Neither consumer replaces `StandardResolver`, though the trait border is
  compile-visible.
- The default quotas are conservative mechanism bounds, not tuned workload
  guarantees for either game.

These are residual hypotheses, not claims of universal fit. A future consumer
that cannot justify one should shrink or split the provider again.

## Rejected alternatives

- A universal attack/damage/item/stat vocabulary: it would duplicate downstream
  meaning and immediately fail the Doom/Dagger comparison.
- Keeping `ForEach` because Dagger used it: Doom showed that target selection is
  operation meaning, so retaining it would mistake first-consumer convenience
  for shared structure.
- Moving `GameSession`, Dagger state, RNG, collision, or tick scheduling into
  Engine: those are existing downstream authorities and are not needed by the
  lifecycle.
- Routing only a synthetic Doom fixture: the real pistol/automatic runtime path
  was necessary to expose owned transaction-error propagation and preserve
  existing consequences.
- Porting spread, projectiles, pickups, or enemy AI in the same change: that
  would obscure the bounded proof and turn lifecycle adoption into a combat
  rewrite.

## Residual risks and disposition

The generic type surface is necessarily broad because downstream receipts keep
typed intent, facts, evidence, effects, events, errors, suspensions, and trace
details. `StandardResolver` is also a substantial implementation and should be
split internally if it grows. These are review signals, not reasons to move
game meaning upstream.

The resulting seam is narrow enough to remain in the public Rusty Engine
facade. It has two mechanically different production consumers, no game or host
dependencies, and became smaller under the second consumer. Future work should
extend it only for host-neutral lifecycle evidence demonstrated by another real
consumer; otherwise the change belongs downstream.
