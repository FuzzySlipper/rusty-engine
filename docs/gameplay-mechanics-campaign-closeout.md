# Gameplay mechanics campaign closeout

This document records the completed GM0-GM7 evidence for the optional
`gameplay-mechanics` and `gameplay-rules` surfaces. It is a committed evidence
index, not a live task mirror. The owning contracts remain
[design.md](design.md), [Gameplay mechanics](code-map/gameplay-mechanics.md),
and [Optional gameplay rules](gameplay-rules-contract.md).

## Result

The campaign supports three deliberately different compositions without
placing their game policy in Engine:

| Composition | Reusable Engine mechanism | Downstream authority retained |
| --- | --- | --- |
| Realtime shooter | Components, equipment sources, effects, tracks, damage, and receipts | Hit admission, weapons, fixed ticks, death/score consequences, product save, input, and presentation |
| Infrastructure builder | The same components and named services, with durability, production, modules, damage, repair, and explicit expiry | Day/phase clock, building meaning, production policy, and complete state |
| Rusty D20 | Optional opaque rule packages plus components, mechanics services, and deterministic RNG | D20 schema and compiler, actions, reactions, turns, semantic content, complete save, host, and UI |

Objects carry typed facts, named services own mechanisms, and downstream games
own meaning and orchestration. No `MechanicsState`, universal rule IR,
scheduler, ambient event bus, replay runtime, global registry, service locator,
or complete-save owner was added.

## Final ownership and dependency direction

```text
mechanics-only downstream Rust
  -> entity-state
  -> gameplay-mechanics

rules-heavy downstream Rust
  -> gameplay-rules (optional opaque package support)
  -> entity-state / gameplay-mechanics / svc-rng as explicitly chosen

optional build-time authoring
  TypeScript candidate data
  -> canonical package artifact
  -> gameplay-rules admission
  -> downstream semantic compiler
  -> downstream runtime and product UI
```

`gameplay-mechanics` has no normal or build dependency on `gameplay-rules`,
Node, TypeScript, a browser, Rusty D20, or a UI package. Its only new GM7
measurement dependency is `stats_alloc`, and that dependency is test-only.
`gameplay-rules` is a sibling, not a layer above mechanics. Its payload remains
opaque, and it publishes nothing into an entity store or global catalog.

The seven durable mechanics components remain ordinary `entity-state`
components. Services validate and publish exact component slots or the narrow
homogeneous replacement/containment operation they advertise. Catalogs and
rule packages are immutable admitted definitions; requests and receipts are
transient typed values. Source-attributed stat ledgers, package diagnostics,
and operation receipts explain the owning decision without requiring a global
event or replay record.

## Exact reviewed evidence

Every row below reached `looks_good` at the exact head shown. Gate numbers are
the terminal exact-SHA Den/GitHub evidence; later unrelated repository commits
do not replace these revisions.

| Task | Reviewed result | Exact head | Review round | Terminal gate |
| --- | --- | --- | ---: | --- |
| Engine #6284, GM0 | Crate and component/service contract, full-span arithmetic and bounded preflight fixes | `44a729d548c2a9d873a0f7a919c89f0d212731b7` | 3427 | 2068, `verify` + `verify-studio` |
| Engine #6285, GM1 | Catalog, attributed sources, stats, and tracks | `78f8156b4ba3f1a4f18e58fd8418d118f9a755b8` | 3431 | 2071, `verify` + `verify-studio` |
| Engine #6286, GM2 | Explicit active-effect lifecycle and source activation | `99fc3c925e10283c25082b4602444ecae539e20f` | 3434 | 2078, `verify` |
| Engine #6287, GM3 | Damage, restoration, atomicity, and complete receipts | `41e58433e2a86093cc0448431ec14ebbb5ae190e` | 3436 | 2080, `verify` |
| Engine #6288, GM4 | Inventory, unique items, containment, and equipment | `c2b6fdb173ea81447b87b750d8654b812468905c` | 3441 | 2082, `verify` |
| Engine #6289, GM5 | Persistence, inspection, local cost, direct compositions, and mechanics donor audit | `1ee4af531c848e3931cce33b22dc63405d48e3e7` | 3445 | 2085, `verify` |
| Demo #6290, GM6 | Exact provider adoption by the loading-bay realtime product | `741812348f1a99a4e13415467c928a2a0dc32a43` | 3456 | 2094, full product `verify` |
| Engine #6311, GR0 | Frozen optional rules boundary and complete rules donor audit | `d2fd6dac032cfeceaecdfa2b829b57b302feda4e` | 3460 | 2100, `verify` |
| Engine #6322, GM7I-A | Prospective equipment source/track-bound validation | `11b41a0501f2d0c543f6e2fa79d20ba5c300722e` | 3462 | 2103, `verify` |
| Engine #6314, GM7I | Infrastructure builder falsification fixture | `5c72920edf38709b34f05b91b2a4c675fda8bfb7` | 3464 | 2105, `verify` |
| Engine #6312, GR1 | Bounded Rust package substrate | `ea287cc97ba2e2a350ec5eab012b6d9860d33ecd` | 3467 | 2109, `verify` |
| Engine #6313, GR2 | Isolated generated TypeScript authoring support | `fb608e323a8b44a55195f5720101224ff37fd5db` | 3470 | 2114, `verify` + `verify-rules` |
| Rusty D20 #6316, D20B0 | Real Rust host and exact rusty-engine-ui-derived shell | `1c8aef17994cfa4b1628767f66e1b5ecbfd90ced` | 3484 | 2143, all five required jobs |
| Rusty D20 #6317, D20R0 | Downstream semantic kernel, mechanics/RNG binding, and persistence | `75697ead3ab47f4a63bac89f33a408ce92dfa764` | 3490 | 2147, all five required jobs |
| Rusty D20 #6318, D20A0 | Immutable TypeScript authoring SDK and two substantial content compositions | `22accfd2849e836a7aa79d76076df6ad35a4800a` | 3500 | 2156, all six required jobs |
| Rusty D20 #6319, D20G0 | Interactive browser slice and fresh-process save contract | `793dd6037d99091d958f675c98b35320b9aca307` | 3506 | 2159, all six required jobs |

The D20 bootstrap resolves Rusty Engine at
`fb608e323a8b44a55195f5720101224ff37fd5db`. Its UI donor is exactly
`rusty-engine-ui` revision
`68ddfa5430ec3bc2cf7ca96963982db9511e79ba`. No Engine build or test reads
either sibling repository.

## Consumer proof

### Realtime loading bay

The exact demo revision uses the provider as its only live mechanics owner:
unique weapons are ordinary contained item entities, equipment activates
protection sources, vitality is a track, temporary invulnerability is an
explicit effect, and immediate hit resolution calls `DamageService`. The demo
routes depletion facts and detailed receipts into its own death, score, and
presentation consequences. Its candidate session owns wider product
atomicity; schema 19 owns the complete save.

The approved product proof included 71 focused Rust tests, full product
verification, and the real browser campaign through normal controls. That
campaign exercised cooldown-aware hits, protection, held-input death,
save/restore through product controls, and a fresh host. Legacy schemas 10-18
migrate only when provider fields are genuinely absent; future provider fields
in an old envelope fail before mutation.

### Infrastructure builder

The checked `gm7_builder` fixture composes only public components and
`EquipmentService`, `EffectService`, `StatService`, `TrackService`, and
`DamageService`. It models production, stat-bounded durability, contained and
equipped modules, a temporary improvement, impact/corrosion damage, repair,
caller-owned day/phase expiry, reconcile/retry, strict snapshot reopen, and
deterministic continued mutation. Rejected ownership, capacity, stale,
arithmetic, quota, and prospective-bound operations compare the canonical
snapshot, global revision, containment, and all seven mechanics slot revisions
to prove non-mutation.

The fixture discovered one reusable gap: equipment source removal could
invalidate a stat-bounded track. Engine #6322 fixed that at
`EquipmentService`; the fixture did not acquire a turn model, actor concept,
D20 vocabulary, scheduler, or `gameplay-rules` dependency. It remains a
bounded headless falsification fixture, not an external consumer, shipped
builder product, or rules-promotion vote.

### Rusty D20

Rusty D20 is a downstream game, not an Engine facade. Its Rust code owns the
D20 candidate schema, source-correlated semantic compiler, abilities,
defenses, damage types, resources, armor, reactions, actions, turn policy,
complete session/save, and product projection. Admitted definitions drive the
canonical `EntityState`, mechanics services, and `svc-rng`; there is no shadow
gameplay store.

The isolated TypeScript workspace authors immutable data candidates. Its
callbacks execute during authoring and are never persisted. It contains no
runtime evaluator, mutable gameplay state, service locator, language bridge,
browser transport, or game session. Rust strictly admits the generic package,
compiles D20 meaning, and owns execution. The exact #6318 proof compiled Steel
Guard and Ember Ward artifacts with Node unavailable and retained exact source
correlation for invalid content. Its `content-only addition uses the published
authoring surface` regression demonstrates that a new definition assembled
from existing D20 primitives changes TypeScript content/artifacts only; it
does not require an Engine edit or a new Rust semantic primitive.

The exact #6319 path is real input to the product host, authoritative mutation
through the named product service, strict projection, TypeScript decode/store,
and rendered UI in the
durable donor-derived shell. Real Chromium covered empty/start, preview,
reaction, resolve, receipt, turn, stale two-page rejection, network/invalid
failure, mobile overflow, and save/reopen. Independent fresh-process proof
confirmed completed-action save/reopen and deterministic next-roll
continuation. Pending previews are deliberately unsaveable: preview-only and
Parry-reacted attempts return typed HTTP 422 before live or file mutation, and
the UI disables and explains Save.

No other RPG imports Rusty D20. A different RPG may use `gameplay-mechanics`
directly, opt into the opaque `gameplay-rules` envelope with its own payload
compiler, or use neither.

## Measured costs and hard bounds

[`builder-evidence-v1.json`](../fixtures/gameplay-mechanics/builder-evidence-v1.json)
is checked by the GM7 fixture. Its source visits and bytes are contract-level
observations. Allocation values are release observations, not performance
budgets.

| Observation | Simple: 1 module + 1 effect | Stressed fixture: 8 modules + 1 effect |
| --- | ---: | ---: |
| Stat decisions | 2 | 9 |
| Equipment entries visited | 1 | 8 |
| Item components read | 1 | 8 |
| Effect entries/source activations visited | 1 / 1 | 1 / 1 |
| Canonical snapshot bytes | 3,484 | 8,595 |
| Release allocation calls per stat evaluation | 38 | 108 |
| Release reallocation calls per stat evaluation | 0 | 9 |
| Release allocated bytes per stat evaluation | 3,160 | 9,210 |
| Release net reallocated bytes per stat evaluation | 0 | 4,160 |

The release allocation observation is the stable result of two consecutive
isolated runs of:

```bash
cargo test --release -p gameplay-mechanics --test gm7_builder \
  checked_builder_evidence_records_bounded_costs_sizes_and_non_claims \
  --locked -- --nocapture --test-threads=1
```

It used `rustc 1.96.0 (ac68faa20 2026-05-25)`, release optimization, and
`x86_64-unknown-linux-gnu`. The test-only `stats_alloc` wrapper observes the
System allocator. Compiler, target, and allocator changes require a fresh
observation rather than treating these values as a compatibility limit. Clone
counts remain unclaimed.

API amplification is fixed at the named-service boundary: stat evaluation is
one public service call and zero component writes; a one-part damage apply is
one public service call and one `TracksComponent` replacement. Damage does not
emit N public calls for its N internal stages.

The base GM5 path with 2,048 unrelated entities still reports zero intrinsic,
effect, equipment, item, and request-source visits for a simple stat evaluation
and one-part damage. There is no global entity scan. With no attached source
components, rules package, Node process, authoring workspace, registry, or
session is initialized or traversed.

Current mechanics receipt/work quotas include 32 equipment assignments, 64
active effects, 256 expanded effect sources, 32 request sources, 8 damage
parts, 256 stat decisions, 256 damage decisions, and 128 damage facts. Catalog
and component limits remain defined beside their owners and are routed from
the [mechanics code map](code-map/gameplay-mechanics.md). Exact/one-over and
late-failure tests reject before publication.

The optional rules boundary admits at most 4 MiB per canonical package and 64
packages/16 MiB per resolved set. Per-package limits are 32 dependencies, 64
sources, 4,096 provenance records, 100,000 JSON nodes, depth 64, and 1 MiB per
JSON string. Set limits are 512 dependencies, 1,024 sources, 16,384 provenance
records, and 400,000 JSON nodes; diagnostics are capped at 256. The Rust and
TypeScript contract suites test exact-limit admission and one-over rejection,
including early collection preflight before aggregate expansion.

## Donor disposition

Both literal Asha audits use `asha-rpg` commit
`e4d6d1afb5b8387de4ff805d73b2041df29ee590`, tree
`3efee336ff8c6c9aeea2c37035d5258bfdf88847`, and 152 tracked paths.

- The mechanics ledger records 3 adopted evidence rows, 12 adapted/rewritten
  rows, and 137 exclusions.
- The separate rules ledger accounts for all 152 paths again because its
  smaller semantic-neutral owner is a distinct disposition.
- Checkers validate both complete path sets without a donor checkout.
- No donor implementation, crate, package, compiled RPG vocabulary, session,
  scheduler, event/replay topology, or sibling path enters Engine.
- The Rusty D20 UI donor pin is the separate exact revision recorded above;
  donor UI structure does not transfer product authority into Engine.

See [donor provenance](migration/donor-provenance.md) and the machine-readable
mechanics and rules disposition ledgers for every literal path.

## Acceptance mapping

| Campaign acceptance | Closure evidence |
| --- | --- |
| Mechanics-only consumers avoid rules, Node, UI, and D20 | Normal dependency tree and GM0-GM6 exact revisions; zero rules/session traversal on the base path |
| Another RPG need not import D20 | Optional opaque sibling contract, direct Rust package construction, downstream-owned semantic compiler |
| Builder stays genre-neutral | GM7I fixture and GM7I-A owner fix; no rules dependency or RPG vocabulary |
| D20 is an interactive downstream consumer | D20B0-D20G0 exact chain, real Rust host, Chromium, and fresh-process evidence |
| TypeScript remains authoring data only | Generated plain-data contract, boundary audit, Node-free Rust compilation, no persisted callbacks/runtime |
| No universal runtime entered Engine | Design/source audit and complete donor exclusions; direct named services remain the only execution boundary |
| Exact revisions, gates, limits, and donors are durable | Tables and measured evidence in this document plus checked ledgers/fixture |
| No acceptance gap is relabeled | The only cross-composition provider gap was fixed in #6322; pending-save loss was fixed downstream in #6319 |
| Agents without Den can navigate the result | This closeout, design, code maps, fixtures, and donor ledgers are committed and linked from the atlas |

## Deliberate stopping point

The campaign is complete at the common mechanics, bounded rules-package seam,
builder falsification, and one real rules-heavy product path. Broader Rusty D20
content/UI work, a real builder product, game-specific save migration, and
additional genre consumers remain downstream expansion. They are not missing
Engine acceptance work.

New Engine work still requires concrete evidence. In particular this closeout
does not justify a universal rules language, D20 framework, scheduler,
behavior graph, replay/certification system, generic mod platform, or UI
roadmap.
