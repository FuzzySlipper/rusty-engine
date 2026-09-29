# Managed hardware exceptions under the CoreCLR host (#8753)

A null dereference in product C# killed `rusty-product-host` with SIGSEGV
instead of throwing `NullReferenceException`. It now throws, in the direct host
and in the supervised runtime that `rusty dev` uses.

## Cause

CoreCLR raises `NullReferenceException` from its SIGSEGV handler, which runs
on the thread's alternate signal stack. CoreCLR allocates that stack (16 KiB
here) only for a thread that has none.

Rust std had already given every thread one. At startup std installs its own
SIGSEGV/SIGBUS handler for its stack-overflow message, and from then on each
thread std starts, including `main`, gets an 8 KiB alternate stack. CoreCLR
reused those stacks. Its handler overflowed 8 KiB into std's guard page, and
the second fault killed the process. `results/signal-trace.txt` shows it on
the main thread:

- the null dereference: `SEGV_MAPERR` at address 0;
- the overflow: `SEGV_ACCERR` at `…f7e928`, just below the 8 KiB stack at
  `…f7f000`.

The other leads in the task did not apply:

- the signal masks from the old worker are gone since #8766;
- no thread is special. Every Rust thread that calls into the product (main,
  the realtime scheduler, each connection thread) had the same 8 KiB stack.

A division by zero (SIGFPE) already worked before the fix, because CoreCLR's
SIGFPE handler does not use the alternate stack.

## Fix

`rust/crates/csharp-product-runtime/src/main.rs`: std installs its handler,
and so creates those stacks, only if SIGSEGV and SIGBUS still have their
default action when std initializes, before `main`. So:

- an `.init_array` constructor sets both to ignored;
- `main` sets them back to default as its first statement.

No thread gets a std alternate stack, and CoreCLR allocates its own on each
thread it enters. Nothing else changed; the supervisor's process-group setup
stays as it was.

The cost: std's "thread has overflowed its stack" message is gone from this
binary. A Rust stack overflow still ends in SIGSEGV, but without that line.

## NativeAOT

NativeAOT was not affected. Its handler also ran on std's 8 KiB stacks, but it
fits in them. It caught both exceptions before and after the change. After the
change its threads have no alternate stack, so its handler runs on the thread
stack. Not tested: a *managed* stack overflow under NativeAOT may now die
without the runtime's "Stack overflow" line, since that handler needs an
alternate stack. Such an overflow was never catchable.

## Exercises

**Direct host (`--exercise`), CoreCLR.** This uses the packaged smoke
consumer. `fixtures/csharp-hardware-exceptions/HardwareExceptionChecks.cs`
catches three exceptions inside the first `Update`:

- a product null dereference;
- `CameraView.SetActiveCamera(null!)`, the generated-call case from the task;
- an integer division by zero.

| | Before | After |
|---|---|---|
| CoreCLR `--exercise` | exit 139 before the catch runs | `HARDWARE_EXCEPTION_CHECKS_PASSED`; exercise passes |
| NativeAOT `--exercise` | passes | passes |

The fixture is now part of `scripts/test-csharp-sdk-package.sh`
(`--coreclr-smoke`, and `--aot` for the NativeAOT leg).

**Supervised runtime, as under `rusty dev`.** `scripts/fault-product` is an
ordinary SDK product. Every update catches a null dereference and a division
by zero. With `FAULT_AT=240` it lets one of them escape at update 240.

`scripts/fault-exercise.mjs` launches it `--supervised` with a browser
attached. It adapts the #8736 exception exercise.

| | Before, `null-reference` | After, `null-reference` | After, `divide-by-zero` |
|---|---|---|---|
| Runtime | SIGSEGV on update 1, restarted, SIGSEGV, paused | same instance, `running → faulted` | same |
| Caught before update 240 | none | 480 | 480 |
| Exception log | none | `CSHARP_PRODUCT_CALL: … NullReferenceException` with the `HardwareFault.Product.Update` frame | `… DivideByZeroException`, same frame |
| Browser | 503, bootstrap failed | stayed `ready` | stayed `ready` |
| Resume | 503 | accepted; simulation step 240 → 420 | accepted; 241 → 420 |
| Supervisor restarts | 2, then pause | 0 | 0 |

The results are in `results/*.json`. An uncaught hardware exception now takes
the ordinary #8736 path: it is logged with its stack trace, the lifecycle
faults, and the product can resume.

### Reproducing

1. Run `RUSTY_ENGINE_SDK_TEST_KEEP_WORK=1 scripts/test-csharp-sdk-package.sh --coreclr-smoke`.
   This produces the local package feed. `hostfxr` needs `DOTNET_ROOT` when
   `dotnet` is a user install.
2. Build `scripts/fault-product` against that feed. Give it a `NuGet.Config`
   that points at the feed, and an empty `content/` directory. Stage it with
   `dotnet msbuild Fault.csproj -t:StageRustyEngineCoreClrProduct`.
3. Put the host into a runtime-pack layout: `bin/rusty-product-host`,
   `share/browser` (as in `scripts/build-runtime-pack.sh`).
4. Run `node scripts/fault-exercise.mjs <host> <staged Product dir> <out.json> <label> 240 <null-reference|divide-by-zero>`.

## Validation

- `cargo clippy -p csharp-product-runtime --all-targets --no-deps -- -D warnings`
  is clean. Without `--no-deps`, it stops on the known `content-store` lint
  (#8757).
- `cargo test -p csharp-product-runtime`: 43 library and 27 host tests pass.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke --aot` passes, with
  `HARDWARE_EXCEPTION_CHECKS_PASSED` from both loaders.
