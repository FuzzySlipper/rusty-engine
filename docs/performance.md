# Performance diagnostics

Rusty Engine keeps one checked performance probe that divides a frame-sized
operation into independently attributable layers:

- `rust-appearance-call-stage` measures Rust service staging with an 8 MiB
  retained renderer resource. Resource bytes must be shared, not copied, when a
  transactional C# call begins.
- `managed-csharp-update` measures a stable allocation-free managed update
  without Rust or browser work.
- `csharp-rust-crossover` measures the normal generated NativeAOT callback,
  Engine service transaction, and output conversion.
- `product-dev-host-http` adds the local product-host HTTP admission path.
- `browser-renderer-submission` measures explicit submissions through the real
  renderer surface and reports draw/resource statistics, backing resolution,
  renderer identity, and pacing classification.

Run the complete probe from the repository root:

```sh
./scripts/run-performance-regression.sh
```

Each result is printed as a single `RUSTY_PERF` JSON record. Override the
crossover sample count with `RUSTY_PERF_ITERATIONS` (1 through 256). The browser
probe deliberately runs with the repository Playwright configuration, which
currently selects SwiftShader; its renderer and actual backing resolution are
part of the record rather than hidden environmental assumptions.

## Interpreting regressions

Compare records from the same machine, build mode, browser, and renderer. There
is intentionally no universal millisecond gate: a hardware browser, CI
software rasterizer, and developer laptop are different performance classes.
Use the first lane that regressed to localize investigation:

1. Rust staging points to Engine transaction or resource ownership work.
2. Managed C# points to downstream-style update cost or managed allocation.
3. Crossover points to generated ABI, lifecycle, or Engine service commit cost.
4. HTTP points to product-host transport or synchronization.
5. Renderer submission points to backend realization, scene complexity, GPU
   completion, or resolution/pacing policy.

The probe is deliberately bounded and synthetic. A live product diagnostic is
still needed to identify which lane a particular slow scene is exercising; the
probe prevents already-fixed costs from silently returning.
