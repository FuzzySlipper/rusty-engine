# Lane: downstream

**Tasks, in order:** #8832 (CraftSurvive) now; then, once #8744, #8799 and
#8807 have landed: #8834 (Dungeon), #8835 (Underworld), #8838 (D20), #8839
(Roguelike), #8833 (Crawler), #8836 (Rifles), #8837 (Space).
Campaign #8831; read it first. It holds the per-product steps, the evidence
list and the gap classification. Shared protocol: [README.md](README.md).

Products that share a pin come next to each other (Dungeon and Underworld,
D20 and Roguelike), so the second one reuses the first one's findings. Each
product is its own repository, so if the gated list is long the owner may
split it across two instances without file conflicts.

## What this lane does

For each product: move to the current Engine pair with `rusty update`, apply
the migration notes, run the product's own checks and `rusty dev`, and turn
every gap into either a product migration or a filed Engine task.

- **CraftSurvive first.** It is 37 commits behind and is the early signal.
  Post a reusable migration summary on #8831, then move CraftSurvive again
  once the gate lands.
- **Gate.** abi lane (#8744, #8799) and spatial lane (#8807) change the ABI
  and product API. Watch those tasks in Den; don't migrate the gated products
  before they land.
- **Gaps are Engine tasks.** Don't rebuild an Engine mechanism in a product
  to get past one. File it with its AGENTS.md classification, under the owning
  campaign, or #8831 if none fits. Link it from the product task. If the owner
  has authorized the Engine change, another lane or the main lane implements
  it.
- **Remove what the reset made unnecessary.** For example: retries for removed
  guards, product revision bookkeeping for removed receipts, workarounds for
  removed caps.

## Files

- **Owns:** the product repositories, on each product's main branch:
  - `/home/agent/dev/rusty-craftsurvive`;
  - `rusty-dungeon`, `rusty-underworld`, `rusty-d20`, `rusty-roguelike`;
  - `rusty-crawler`, `rusty-rifles`, `rusty-space`;

  plus the evidence directories for these tasks under
  `docs/evidence/downstream-<product>-<task>/` in the Engine.
- **Leave alone:** Engine source. A fix there is a filed task, not an edit
  from this lane. Doom and Dagger belong to the campaign lanes, and the
  template to the cli lane.
- **Coordinate:** cli lane #8800 also edits
  `CraftSurvive.Game.csproj` (the SDK UI build target). Check its state before
  touching that file.

## Evidence

Per #8831:
- the pair moved from and to;
- the migrations applied;
- the check results and the `rusty dev` observation;
- the filed Engine tasks, or "none found".
