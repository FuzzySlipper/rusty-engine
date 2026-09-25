# Bounded rope physics contract

Campaign #6992; design task #6993; consumers #6994, #6995 and CraftSurvive
#6996. This is an implementation contract, not a claim of available SDK APIs.
The executable solver probes are in
[`rope_probes.rs`](../rust/crates/svc-collision/tests/rope_probes.rs).

## Decision and source owners

Use Rapier 0.34 maximum-distance `RopeJoint`s in the existing derived dynamics
world. Keep canonical body poses, velocities and authored rope definitions in
Engine owners; rebuild joints deterministically alongside bodies on each
request. Do not persist Rapier joint handles, impulses or warm-start caches.
`svc-collision` remains the sole Rapier consumer; `engine-spatial` owns canonical
preparation/publication. Extend the existing `Dynamics` ABI/service/generator
family for C# use. Extend the existing character request and receipt for
character coupling, rather than adding an alternate controller or scheduler.

The pinned dependency's `dynamics/joint/rope_joint.rs` implements a coupled
linear limit on a generic joint, with no locked axes. `spherical_joint.rs`
locks translation at coincident local anchors but does not provide slack.
Spherical joints are suitable for rigid rods with freely rotating endpoints;
they are not substitutes for a maximum-distance tether. Multibody joints add
articulation topology and generalized-coordinate continuation that this bounded
rebuild service does not need. Use impulse joints for the first implementation.
Do not expose a generic joint graph to the product.

## Tethers and short ropes

A tether has a stable caller-selected ID, two explicit endpoints and a positive
maximum distance. An endpoint is either a world point or a dynamic body plus
local anchor. At least one endpoint must be dynamic; both body endpoints must
belong to the same dynamics world. Local anchors transform through the body's
full pose. A fixed endpoint becomes a private, collider-free fixed Rapier body;
it never consumes a product entity ID. Invalid, disabled, removed or foreign
body anchors produce an explicit invalid-anchor outcome, never a world-origin
fallback. Character endpoints use the separate coupling described below.

A short rope is an ordered chain of small colliding sphere bodies joined by
maximum-distance links. This deliberately models beads connected by massless
tethers, not solid rods or a continuous collision cable. Return every bead
center and both endpoint positions in order. Physical gaps between beads can
miss small geometry; diagnostic segments do not fill those collision gaps.
Use ordinary body mass, CCD, collision-group and friction mechanisms. Suppress
contacts between immediately connected beads; nonadjacent contacts remain
enabled unless the author explicitly disables rope self-collision with the
ordinary collision groups. Terrain contacts remain enabled. No automatic
subdivision or rendering-dependent particle count.

The initial probe envelope is eight moving beads, equal masses, 0.5 m spacing
and 0.05 m sphere radii. The implementation must additionally verify terrain
contact, nonadjacent contacts, off-center anchors and unequal masses before
claiming those behaviors. Chain bodies count against existing dynamics body
budgets; creation/removal of an Engine-owned chain is atomic. Do not create
hidden product-side bead bodies or synthesize their motion downstream.

## Time, lengths and diagnostics

Start at 60 Hz, four internal substeps per supplied tick and eight solver
iterations per substep. These are numerical integration subdivisions of the
existing caller-owned update, not another clock. Retain the current maximum
eight requested ticks; allow up to eight subdivisions and sixteen iterations
when explicitly selected, with a reported total work budget. Initial bounded
rope request limits are 64 tethers/chains and eight beads per chain. These
limits bound solver work; they are not hostile-input checks or ABI safeguards.
Outside the proven envelope return a typed budget/configuration outcome rather
than silently trimming bodies, points or substeps.

Generated C# tether/chain operations and `ConfigureRopes` preserve these failures
in `EngineCallException.Diagnostics`, including `dynamics-tether-budget-exceeded`
and `invalid-dynamics-rope-solver-configuration`. Rejected operations leave the
world unchanged. The generated call copies and releases the operation's owned
diagnostic receipt before throwing.

Use the pinned solver's default inelastic limit response. Do not expose
`RopeJoint.softness` as a claimed elastic-rope material: its public description
concerns locked degrees of freedom, and a rope has no locked axes. A spring or
motor needs separate behavior probes before admission. No motor, automatic
breakage or elastic spring is included in this first contract.

For live reeling, keep current and target maximum length separate. Approach the
target by at most the caller-selected reel speed times the admitted tick, with
an initial supported cap of 0.25 m/s. Lengthening uses the same explicit rate;
release removes the constraint immediately. Initial attachment outside the
requested radius reports out-of-reach, so a newly constructed overextended
constraint cannot inject an arbitrary positional correction. This does not
prevent a slack rope catching a fast outward-moving body inside its radius.
Reeling can perform physical work: do not assert conservation of energy while
shortening. A future faster rate requires measured acceptance, not increasing
the cap just to satisfy a downstream control.

Return actual endpoint separation, effective/target length, slack distance,
slack/taut/caught state, ordered points, and a sampled force proxy. Rapier 0.34
subdivides each solver iteration in time and retains only the last internal
substep's limit impulse. Divide that impulse by its internal substep duration
and take the maximum across observed outer subdivisions. This matches a hanging
mass's weight, but can miss catch peaks; it is neither a whole-tick accumulated
impulse nor a certified breaking tension. Specify units (metres, seconds,
kilograms, N); report substeps, iterations,
body/link counts and any exhausted budget. A caught transition compares prior
canonical slack state with this result, not cached Rapier state. Removal returns
a released receipt; missing anchors return invalidated. Downstream decides
whether any of those observations imply damage or rope breakage.

## Canonical state and publication

Store definitions, effective length and previous slack/taut state with a rope
revision in the existing Engine dynamics world. Prepare captures that revision,
the exact body component revisions and the existing collision-environment
identity. Validate all definitions before simulation. Commit checks those same
identities and publishes every body and rope readout together or rejects the
entire candidate. A second commit, length edit, body removal or environment
replacement invalidates the prior candidate. Never publish only some chain
bodies when a quota, finite-value or revision check fails.

Sort ropes by stable ID, beads by chain order and bodies by existing body ID.
Snapshot continuation includes definitions, effective lengths, transition state,
body transforms/linear and angular velocities and sleep flags. Restoring those
facts and rebuilding the derived solver must reproduce uninterrupted
rebuild-per-request execution with identical input order and fixed ticks. This
does not promise cross-version or cross-platform floating-point identity.
Wake connected bodies on attachment and length changes; preserve explicit
sleep facts otherwise. CCD applies to bead/body collision and does not turn
the space between beads into continuous collision geometry.

## Kinematic character coupling

One optional tether participates in the existing character step. Supply a
stable attachment ID, character-local attachment offset, fixed anchor or an
Engine-resolved dynamic anchor, effective/target length and previous attachment
state. A dynamic anchor observation carries body/world identity and revision,
world attachment point, point velocity (including angular motion), center of
mass and effective point-impulse response including inertia and locked axes. Resolve
it through Dynamics; C# must not duplicate transform/inertia calculations.

Integrate after controlled/external velocity, gravity and platform departure
have been composed, and before `move_and_slide`. Work relative to the anchor's
point velocity. When the predicted displacement exhausts slack, eliminate the
outward radial component and preserve the tangent. Reconcile the accepted
collision result against the length constraint through bounded collision sweeps;
never teleport a post-solve correction through geometry. Optional floor adhesion
must not snap outside the rope and create a compensating launch impulse; its
vertical eligibility uses total tethered motion. Apply correction within
the existing displacement/query/recovery budgets. If terrain makes the length
constraint infeasible, report unresolved constraint and actual separation
without corrupting the prior canonical controller state.

Store the final accepted total velocity coherently in the existing controlled
and external motion fields; do not add a separate swing velocity ledger.
Grounding, jumping, step-up, slope response, moving support and release continue
through the same controller. Release retains the accepted world velocity.
Explicitly test that ordinary planar input does not overwrite swing momentum.

Use caller-selected effective character mass to convert the accepted tether
velocity change into a reaction proposal. Share the relative correction using
the anchor point response and character mass; treating a light anchor as an
infinite mass before applying its reaction can create energy. Clamp both sides of this exchange
consistently to the existing maximum dynamic impulse: report saturation rather
than claiming an equal-and-opposite impulse when only one side was clamped.
The receipt includes radial/tangential velocity, correction, resolved endpoints,
attachment state and a revision-bound reaction impulse at the anchor point.
The product explicitly applies it through Dynamics at its chosen update order;
Engine does not mutate another owner or step dynamics inside a character step.
Stale proposals reject without mutation. With a saturated reaction, report any
unresolved distance; do not silently promise both a perfectly rigid constraint
and a capped physical impulse.

## Implemented dynamics slice (#6994)

The source now exposes generated Dynamics tether and Engine-owned chain APIs;
see [SDK use](csharp-sdk.md#bounded-dynamics-ropes). A chain has one fixed or
body-local anchor and a free terminal bead. Its ordered readout is the anchor
followed by bead centers; a second terminal attachment is not part of this
bounded API. Chain body creation/removal is atomic in the existing Dynamics
world. Canonical body snapshots plus captured tether definitions and solver
configuration provide rebuild continuation; Rapier state is never serialized.

Owning tests cover terrain contacts, adjacent suppression/nonadjacent collision
masks, unequal body masses and rotating off-center anchors, slack catch energy,
pendulum/release, reel direction changes, CCD selection, explicit sleep/wake,
invalid anchors/configuration, 64-rope/eight-bead limits, stale prepared commits,
fixed-point rebasing and snapshot/rebuild repetition. The generated fixture
exercises attachment, catch, release, chain points/removal and solver work counts
through the actual CoreCLR host. This source proof does not establish a published
SDK/runtime pair or CraftSurvive gameplay acceptance.

The #6995 source adds optional character tethers to the existing collision/slide
step, canonical attachment continuation, Engine-resolved dynamic observations
and explicit revision-bound reaction batches. The same displacement/query
budgets bound swept corrections. Native regressions cover fixed swing/release,
catch energy including a light dynamic anchor, capped momentum exchange, loaded
reeling, terrain obstruction, ground/ledge/platform transitions, deterministic
continuation and invalid inputs. See [character tether use](csharp-sdk.md#character-tethers).
The paired downstream playground and matched release adoption remain #6996.

## Evidence and remaining implementation proof

Run `cargo test -p svc-collision --test rope_probes -- --nocapture` from `rust/`.
The probes discard the entire solver world every 60 Hz tick and run ten seconds
per scenario, using four substeps/eight iterations. On the initial local run:

| Probe | Observation |
| --- | --- |
| Hanging 1 kg mass at 3 m | Maximum extension 2.49e-13 m; settled speed below 0.1 m/s |
| Slack catch, initial velocity (8, -15, 0) m/s | Maximum mechanical energy 134.689583 J versus initial 134.69 J |
| Pendulum from (2.4, -1.8, 0) m | Crosses opposite side; exact repeat; release preserves horizontal velocity |
| Loaded shortening/lengthening at 0.25 m/s | Maximum speed 0.4905 m/s; reaches both requested lengths |
| Eight-bead, 0.5 m chain | Maximum segment extension 2.18e-5 m |

These probes establish a starting solver strategy, not product feel or public
API completion. They use spherical center anchors and do not certify terrain
collision, angular continuation, unequal mass ratios, character coupling,
sleeping, ABI ownership or publication transactions. #6994 and #6995 own those
implementation checks and focused provider gates. They must also cover typed
invalid inputs, budget exhaustion, snapshot continuation and stale candidates.

CraftSurvive #6996 consumes the generated services with C# attachment/control
policy, debug segments and minimal markers. It owns normal-speed catch, swing,
release, terrain-contact, reeling and dynamic-anchor playtests, with subjective
feel reported separately from deterministic evidence. Publish and adopt an exact
matching SDK/runtime pair before that acceptance exercise. Rope meshes, ribbons,
tubes, textured cables, cloth, universal constraint graphs, full-body climbing,
animation and climbing rules remain outside this campaign.
