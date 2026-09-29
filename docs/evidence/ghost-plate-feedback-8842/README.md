# Ghost plate realization feedback from the runtime renderer (#8842)

When the runtime draws with `render-wgpu`, in the streaming mode or the desktop
window, no browser realizes ghost plates. The stream surface returned no
ghost plate readout, so the C# `ReadGhostPlate` never had a renderer
observation.

## Change

- **`csharp-product-runtime` (`frame_output.rs`).** After each product call,
  `FrameOutput::report` reads every realized plate from the renderer
  (`SceneDriver::ghost_plate_readouts`) and ingests them as the complete
  snapshot (`ingest_ghost_plate_realization_feedback`). The streaming and
  desktop modes share this driver path, as they share animation and video
  facts.
- **Field meanings for wgpu (`ghost_plate_fact`).** No field is removed:
  CraftSurvive's debug readout shows most of them.

  | Field | wgpu meaning |
  |---|---|
  | `current_sector`, `local_angular_offset_degrees` | The sector the last view drew, and that view's azimuth around the plate, which is what the browser reported too |
  | `source_matches` | `true`: a plate is built from its own descriptor's captured source |
  | `fallback_active`, `fallback_reason` | `false`, `None`: a plate whose capture failed is not realized and has no fact (the apply issue is a renderer diagnostic); no realized plate draws a stand-in |
  | `limitation_mask` | The Three lane's retained profiles, which the port keeps (frozen source and pose, whole-hierarchy relief, 8-bit shell depth, no readback, CPU time only): `SingleCaptureViewProfile` for one sector, else `DirectionalCaptureBankProfile` |
  | `preparation_cpu_milliseconds` | none |
  | `capture_cpu_submission_milliseconds` | The whole CPU build: the plate's isolated renderer and every sector's capture |
  | retained sectors, meshes, materials, borrowed textures | Sector captures, frozen source parts, frozen source materials, 0 |

- **`render-wgpu`.** `GhostPlateReadout` gains the frozen source's `parts` and
  `materials`.
- **`product-browser-host`.** The ghost plate reporter no longer reports when
  its surface has no ghost plate readout (`null`, the runtime-rendered
  surfaces). Its empty snapshot would otherwise have cleared the runtime's
  on every flush.

## Evidence

The chain from the drawn frame to C#:
1. **`render-wgpu` `tests/ghost.rs`.** The readout's `current_sector` is the
   sector the view drew: it snaps with the view azimuth, and a 4-sector plate
   seen from its second sector reads `(4, 1)`.
2. **`frame_output.rs`
   `a_drawn_ghost_plate_reports_its_sector_and_bank_profile`.** The readout maps
   to the fact with the drawn sector, the azimuth and the profile for 4 sectors
   and for 1.
3. **`csharp-engine-services`.** `presentation_read_ghost_plate` returns the
   ingested fact (`has_renderer_observation`, `current_sector`, …), and an
   ingested snapshot replaces the previous one
   (`ghost_plate_realization_snapshot_replaces_active_observations_with_empty`).

No product on the current pair uses ghost plates: CraftSurvive, the one that
does, is pinned to `6faeaaaa619c`. So no live product readout was taken.

**Checks.** `cargo test` for render-wgpu, render-stream and
csharp-product-runtime; workspace and stable clippy; fmt; the
product-browser-host tests (106).
