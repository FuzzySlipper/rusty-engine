# Rusty Engine

Rusty Engine is an external architecture lab for a smaller, object-centric successor to Asha's gameplay runtime spine.

The experiment keeps reusable world invariants in a headless Rust kernel, then compares two ordinary game-code hosts over that kernel:

- direct Rust services;
- trusted executable TypeScript using bounded read batches and one atomic command batch.

The architecture charter is [asha-object-centric-successor-spike.md](./asha-object-centric-successor-spike.md).

## Donor checkout

During the spike, mature Asha foundation crates are referenced rather than copied. The repository expects this sibling layout:

```text
parent/
  asha-engine/
  rusty-engine/
```

The referenced Asha snapshot and paths are recorded in [docs/donor-provenance.md](./docs/donor-provenance.md).

## First milestone

The first milestone is implemented and intentionally headless:

1. entity/capability kernel;
2. bounded views and atomic mutation batches;
3. explicit facts, snapshots, and projection;
4. Rust service-owned security door;
5. equivalent batched TypeScript game-code host.

Install the pinned TypeScript tools, build the native bridge, and run every gate with:

```bash
pnpm install
pnpm run verify
```

Run the individual headless proofs with:

```bash
cargo run -q -p game-host --bin headless-door
pnpm run measure:door
```

The measured comparison and its current limitations are in
[docs/experiment-results.md](./docs/experiment-results.md). This milestone is evidence that the
smaller center can execute; it is not yet a host-selection verdict or a claim that the larger
successor case has been proved.
