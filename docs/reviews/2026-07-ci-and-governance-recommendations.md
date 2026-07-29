# CI and governance recommendations for rusty-engine

Status: historical recommendation / reevaluated 2026-07-29
Author: system-architect review
Date: 2026-07
Scope: CI tiering, dependency and contract enforcement, reviewer prompts, and porting the
ASHA code map for external-agent onboarding.
See also: [../design.md](../design.md), [../migration-cluster-ledger.md](../migration/migration-cluster-ledger.md),
[../../scripts/verify.sh](../../scripts/verify.sh)

> This is a durable architectural recommendation, not a task queue. It records *why* each
> control should exist and at what hardness, so a future reader (including an external agent
> with no Den access) can judge changes against intent rather than restore ASHA topology by
> reflex.

## 2026-07-29 reevaluation and current disposition

This review remains useful reasoning, but its implementation counts and several
"today" claims no longer describe the repository. The supported exact-pin
Studio/demo integration was restored and independently approved in task #6258;
the latest `main` integration run was also green before the CI changes below
began. The current decisions are:

| Recommendation | Disposition | Current implementation or decision |
|---|---|---|
| Existing deterministic provider checks remain Tier 1 | **Implemented** | `scripts/verify.sh` retains standalone, isolation, donor/equivalence, render-completeness, locked metadata, format, test, clippy, and old-spine absence checks. |
| Add a hard Rust dependency-direction check | **Implemented** | `scripts/dependency_boundary_check.py` reads resolved Cargo package identities and normal/build reachability, including renamed and transitive dependencies. It enforces only explicit foundation, service, inspector-leaf, authority-to-projection, and renderer-neutral boundaries. `render-projection` is explicitly allowed to observe entity, spatial, voxel, and service facts. |
| Add a generic advisory for every new cross-tier edge | **Rejected** | A baseline or tier registry would become a second architecture source and create judgement-heavy drift. The curated maps and review retain ownership of non-forbidden edges. |
| Add the minimal render-model/TypeScript hard border proof | **Superseded by stronger existing coverage** | The committed comprehensive Rust retained-frame fixture is strictly decoded by `render-contracts`; Rust and TypeScript tests plus the hard render completeness inventory exercise both sides. No uncovered drift justifies schema code generation. |
| Add coarse code-map pages and atlas | **Implemented and expanded** | `docs/agent-code-atlas.md` now routes twelve curated owner maps covering all 30 Cargo workspace packages plus the isolated renderer, rules, and Studio workspaces. The old ten-page/28-crate count below is historical. |
| Add a code-map freshness advisory | **Implemented** | `scripts/code_map_freshness.py` checks Cargo-member assignments, stale crate paths, and resolving `Primary paths`. Drift remains non-blocking, while GitHub annotations and the step summary make it visible. |
| Add separate committed reviewer-prompt files | **Superseded** | Root `AGENTS.md`, `docs/design.md`, the atlas/maps, host-boundary ADR, migration ledgers, and Rust/TypeScript style guidance already carry the state-mutation, host-neutrality, donor, and border questions. Another prompt family would duplicate current sources. |
| Add a broad `advise.sh` for file size, names, and catch-all files | **Rejected** | These remain review signals in `docs/topics/development/rust-style.md`; they are not deterministic architecture failures. Only code-map freshness became an advisory. |
| Port Asha lanes, policy registries, generated inventories, replay/WASM gates, or broad protocol codegen | **Rejected** | These remain disproportionate to Rusty's direct-owner architecture and current agent model. |

The hard dependency checker is integrated into the provider gate. The former
`Cargo.toml` text grep for `engine-inspector` was removed from
`audit-standalone.sh` so renamed and transitive enforcement has one owner.
Focused synthetic tests cover positive, direct, renamed, transitive, build, and
advisory-drift cases. This reevaluation supersedes the suggested implementation
order in section 9; the historical discussion below is retained to explain the
tradeoffs.

---

## 1. Context: why rusty governance is lighter than ASHA, and where the risk moved

ASHA was shaped for **high fan-out swarms**: one SOTA planner holding the codebase shape in
context, many mid-range implementers working in narrow lanes. Its governance did two jobs:

1. stop a context-limited implementer from doing something wrong it literally could not see; and
2. make drift visible across many parallel lanes.

rusty-engine now operates under a different model: a **single SOTA implementer** that holds the
whole codebase in context, plus a **second SOTA agent** for architecture planning and review.
Larger context windows and lower relative cost removed most of the pressure that justified swarm
governance. Hard rails whose only purpose was "compensate for a small model in a narrow lane"
are now dead weight, and cutting them was correct.

**But the failure mode moved; it did not disappear.** With two strong agents the risk is no
longer ignorance — it is *shared plausible-but-wrong consensus*: the implementer makes a
reasonable-looking change, the reviewer reads the same diff and agrees, and both share the same
blind spot. A model can genuinely *want* to preserve determinism and still not notice its change
perturbed a hash.

So CI's remaining job is to be **the thing that is not an LLM**: the deterministic oracle for the
handful of properties LLMs are systematically bad at judging by reading a diff.

---

## 2. The decision rule: hard gate vs soft advisory vs no check

Make a check a **hard gate** only when all three hold:

1. the failure is **silent in a diff** — a good reviewer agent will not reliably catch it by eye;
2. the consequence is **corrupting or thesis-breaking** — data, determinism, a public border,
   standalone operation, or restoration of the deleted runtime spine; and
3. false positives are near-zero.

Make it **soft (advisory: print with rationale, do not fail)** when it concerns legibility,
naming, structure, file size, or layering *taste* — anywhere a SOTA model responds correctly to
"warning + why."

Use a third bucket deliberately: **no check at all**, because the in-context implementer plus
reviewer already cover it. Do not add a gate to re-prove something two strong agents reliably get
right.

Guiding asymmetry: **soften the checks an LLM judges well; keep hard the checks an LLM judges
badly.** This is not lowering the bar — it moves each check to the mechanism best suited to
enforce it. In particular the two silent cross-cutting cases below stay hard precisely because
"the model will probably get it right" compounds badly over months.

---

## 3. Recommended CI structure: three tiers

### Tier 1 — `verify.sh` (blocking; the deterministic oracle)

Everything currently in `verify.sh` already belongs here and should stay hard:

- `audit-standalone.sh` — no external Cargo paths, no sibling-repo coupling, no submodules.
- `audit-render-isolation.sh` / `audit-studio-isolation.sh` — workspace isolation.
- byte-for-byte converter reproducibility.
- the forbidden old-spine symbol grep (`RuntimeSession`, `GameplayFabric`, `ReplayRecord`, ...).
- `cargo metadata --locked`, `cargo fmt --check`, `cargo test`, `cargo clippy -D warnings`.

These are all LLM-blind-spot / corrupting-if-wrong / near-zero-false-positive. Keep them.

**Add to Tier 1:**

- **Render-model → render-contracts border golden** (see §4). Silent, cross-language, public border.
- **Forbidden-edge subset of the dependency check** (see §5). Silent, transitive.

Tier 1 must stay fast and deterministic. Nothing subjective enters it.

### Tier 2 — `advise.sh` (non-blocking; prints rationale for the reviewer agent)

New, and where all softening lives. Output is written *for the reviewing agent to read*, not as a
wall that blocks a push:

- new cross-tier dependency edges that are not outright forbidden (see §5);
- file-size signals (the 600/900-line judgement prompts from `rust-style.md`);
- naming/vocabulary notes (`Manager`/`Handler`/`Util`, catch-all `model.rs`/`types.rs` at crate root);
- code-map / README count drift (see §6).

Each line prints *why* it fired. A SOTA reviewer reads these and exercises judgement; a human is
not required to clear them.

### Tier 3 — `docs/reviews/prompts/` (committed reviewer prompts; guidance, not rails)

In the two-agent model the reviewer *is* the governance layer, so committed per-domain reviewer
checklists are the **single highest-leverage investment** here — and they are pure guidance,
which SOTA models respond to strongly. See §7.

---

## 4. Render-model → render-contracts border (keep HARD)

Today `render/packages/render-contracts/src/render.ts` is **hand-authored** TypeScript validated
only by decode/validation tests. ASHA generated this border from Rust (`protocol-codegen`,
reproducible bytes, `--check` in CI) specifically to remove drift risk.

The risk is a textbook silent cross-language failure: a Rust `render-model` field is renamed,
reordered, or retyped, and the hand-written TS interface silently diverges until some runtime
decode test happens to exercise the exact field. A reviewer editing the Rust side will not
reliably remember to hand-patch a TS interface.

**Recommendation (minimal, not full codegen):** have a Rust test emit a canonical golden
descriptor of the `render-model` retained-frame shape (field names, order, types, enum variants)
into a committed fixture. The TS `render-contracts` decode test asserts its own shape matches that
golden. Any Rust change forces a golden update; any TS drift fails the assertion. This restores
ASHA's guarantee for the one border rusty actually has, without reviving the protocol-family
machinery or a code generator.

Do **not** make this advisory. "The model will probably remember the TS side" is exactly the
compounding assumption this gate exists to remove.

---

## 5. Dependency direction (HARD for forbidden edges, ADVISORY for new edges)

I explicitly revise my first-pass advice here for the two-agent model. Do **not** port ASHA's
`ownership.toml` lane framework — that existed to *assign swarm workers*, and there is no swarm.

What review still cannot see reliably is the **transitive** edge (a violation three edges deep in
the graph). That is a genuine LLM blind spot and worth a deterministic check. Keep it lean —
roughly 40 lines over `cargo metadata`:

- **HARD-fail** the small set of truly-illegal edges:
  - anything depending *into* a `core-*` crate from above its tier in a way that inverts the
    foundation direction;
  - render crates depending on entity/spatial *state* authority;
  - any runtime/library crate depending on the `engine-inspector` read-only leaf
    (it must remain a dependency leaf — this is already asserted informally in
    [../inspection-and-diagnostics.md](../inspection-and-diagnostics.md)).
- **ADVISORY-print** any *new* cross-tier edge not on the forbidden list, with the edge named, for
  the reviewer agent to judge.

This gives deterministic protection for the corrupting cases and soft visibility for the
judgement cases, with no assignment-cell bureaucracy.

---

## 6. Port the ASHA code map (NEW PRIORITY — external-agent onboarding)

This is a first-class requirement for rusty that ASHA never had. ASHA was conceived as an
**in-house-only** engine; rusty is intended to be **friendly for agents to use even when they do
not have access to Den docs, prior assumptions, or campaign history**. The code map is the
artifact that makes the repository self-describing for exactly that reader.

ASHA's map had two parts:

1. an **atlas index** (`docs/agent-code-atlas.md`) routing to topic maps and per-area code maps; and
2. per-area **code-map pages** with a fixed schema:
   `Purpose / Owns / Does Not Own / Primary Paths / Public Downstream Surfaces /
   Private Or Forbidden Paths / Acceptance Gates And Goldens / Common Agent Mistakes /
   Follow-up Routing`.

That schema is precisely what an external agent needs: it answers "what is this, what may I touch,
what must I not touch, how do I prove my change, and where does follow-up go" without any Den
context.

Historical implementation note (2026-07-26, superseded by the 2026-07-29
reevaluation above): `docs/agent-code-atlas.md` and ten Rusty-specific owner
maps under `docs/code-map/` implemented the first curated portion. Rusty still
does not carry Asha's generated inventory.

**Recommendation for rusty:**

- Create `docs/code-map/` with one page per coarse owner, reusing the ASHA per-page schema. Natural
  pages for the current 28-crate layout: `entity-state`, `engine-spatial` (voxel authority +
  derived collision/nav/mesh/motion), `voxel-asset` + `voxel-convert`, `render-model` +
  `render-projection` + `render-presentation`, the `render/` TS workspace + hosts, `studio`,
  `content-store` + `asset-catalog` + `authored-scene` + `asset-import`, `environment-authoring`,
  `engine-inspector`, and the `core-*` / `svc-*` foundations.
- Add a top-level `docs/code-map.md` (or `agent-code-map.md`) index that links the pages and, in the
  spirit of the ASHA "How To Use" preamble, tells an external agent: read this first, follow links
  to code and gates, and if the map disagrees with code/tests, fix the map — do not force code to
  match stale prose.
- Keep the "Acceptance Gates" section of each page pointed at the real `scripts/verify*.sh` gates so
  an external agent can prove a change with the same commands CI uses.
- **Freshness check (ADVISORY, Tier 2):** a small script that verifies every code-map `Primary
  Paths` link resolves and that the set of documented owners matches the actual workspace members.
  Print drift; do not block. (ASHA blocked on a generated inventory; for rusty, external-agent
  *trust* in the map matters more than a hard gate, and advisory drift output is enough under a
  reviewing agent.)

Skip ASHA's 75 KB auto-generated `generated-inventory.md` — that volume served swarm navigation and
is churn for a single in-context agent. The hand-curated per-owner pages are the durable value.

---

## 7. Reviewer prompts (SOFT — highest leverage in the two-agent model)

Port a trimmed set of ASHA's `governance/reviewer-prompts/` as committed checklists the
architecture/review agent loads. These are pure guidance and cost almost nothing. Priority order:

1. **state-mutation-boundary** — is mutable authority owned by one visible service/system/atomic
   boundary; no ambient bus, reaction registry, or component callback (from `rust-style.md`).
2. **host-neutrality** — no HTTP/URL/fetch/browser-storage/DOM/WebGL/Playwright seam entering Rust
   or renderer-neutral packages; browser evidence proves browser-owned behavior only
   (Den ADR `rusty-engine/host-platform-and-browser-validation-boundary`).
3. **donor-disposition / absence-by-default** — new Asha-derived behavior has a named consumer, a
   disposition row, and does not restore the excluded runtime spine
   (from [../migration-cluster-ledger.md](../migration/migration-cluster-ledger.md)).
4. **rust-to-ts border** — render-model changes carry a golden update and a TS decode-test update
   (pairs with §4).

Place them under `docs/reviews/prompts/`. They are checklists, not rails: a SOTA reviewer applies
judgement, and the deterministic Tier-1 gates remain the backstop for anything the prompt-driven
review might rationalize past.

---

## 8. Explicitly do NOT port

These were proportionate to ASHA's swarm and heavy spine; they are churn or dead weight here:

- `ownership.toml` lane-assignment framework and per-lane assignment docs (no swarm to assign).
- the guardrail-policy registry (blocking/advisory/cost/trigger/fallback per gate) — right-sized
  only for a large multi-gate CI; rusty's `verify.sh` + isolated render/studio gates is correct.
- the full `protocol-*` generated-contract family and code generator — the single render border
  golden (§4) covers the one border that exists.
- WASM-as-canonical-determinism target and replay-as-prerequisite — meaningful only when the engine
  ships the runtime; rusty correctly keeps replay as a possible future *observer*, never a gate.
- source-shape policy JSON and vocabulary term-gravity gates — legibility taste; belongs in a
  reviewer prompt or advisory print, not a gate.
- the 75 KB generated code-map inventory (§6).

---

## 9. Suggested implementation order

1. **Reviewer prompts** (§7) and **code-map pages + index** (§6) — highest leverage for the actual
   operating model and the external-agent goal; pure documentation, no CI risk.
2. **Lean dependency checker** (§5) — forbidden-edge hard, new-edge advisory.
3. **Render border golden** (§4) — the one new hard cross-language gate.
4. **`advise.sh`** (§3, Tier 2) — collect the soft signals, including code-map freshness.

Items 1 and 4 are self-contained and touch no provider architecture. Items 2 and 3 add
deterministic protection for the two silent cross-cutting cases without reintroducing any deleted
topology.
