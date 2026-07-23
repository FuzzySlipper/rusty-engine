# Donor provenance

The lab references or selectively adapts Asha code only when that code sits below the architecture being tested.

Inspected donor repository: `git@github.com:FuzzySlipper/asha-engine.git`
Pinned source commit: `a431974330589761c9e35fc4f8a55996a1b5ee48`

| Local dependency/use | Asha source path | Treatment | Reason |
|---|---|---|---|
| `core-ids` | `engine-rs/crates/foundation/core-ids` | Sibling path dependency, unchanged | Mature typed identity newtypes; no high-level dependencies. |
| `core-math` | `engine-rs/crates/foundation/core-math` | Sibling path dependency, unchanged | Small deterministic vector values; no high-level dependencies. |
| `core-time` | `engine-rs/crates/foundation/core-time` | Sibling path dependency, unchanged | Stable tick values used by the lab scheduler; no scheduling policy. |

No Asha source has been copied into the repository at this milestone. Any future copy must add its original path, exact commit, copied/adapted symbols, reason a reference was unsuitable, and meaningful local changes here.
