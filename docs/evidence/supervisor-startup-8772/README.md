# Stopping or replacing a runtime that is still starting (#8772)

## Before

`RuntimeLaunch::start` blocked in `RuntimeProcess::wait_ready` until the
runtime printed `RUSTY_RUNTIME ready`, exited, or hit its 30-second deadline.
While it waited, the supervisor did not see a termination signal, stdin EOF or
a restage. With `--debugger` there is no deadline, so the wait could last
forever.

## Change

The blocking wait is gone. `start` only spawns the runtime, and the
supervisor's existing loop moves it forward with `RuntimeProcess::poll_startup`.
Each pass of the loop, at the same poll interval as before:

- **Ready:** the supervisor stops answering 503 and sends `serve`.
- **Exited or past the deadline:** the start failed.

The loop checks the termination flag and supervisor commands on every pass,
so they now reach a starting runtime too. Stop and replace use the existing
bounded stop: close stdin, wait up to 10 seconds, then kill. A runtime that is
still loading exits by itself once it reads EOF.

Start failures behave as before:

- **The first runtime:** a failed start still ends the supervisor with a
  nonzero exit.
- **A later start** (an automatic restart or a restage): it pauses and waits
  for the next restage. A restage sent while the first runtime is still loading
  counts as a later start.

The headless browser now launches once the first runtime serves. Before, it
launched when `start` returned, which was the same moment.

## Exercises

**Held runtime** (`scripts/startup_exercise.py`). The runtime child is sent
SIGSTOP as soon as it appears, so it never reports ready. The Product is the
#8753 fault product with no fault.

| Case | Before | After |
|---|---|---|
| SIGINT to the supervisor group | exits only at the 30 s startup deadline (29.1 s after the signal) | exits after 10.1 s, the bounded stop: logs `shutdown reason=termination-signal`, then kills the runtime |
| Close supervisor stdin | exits at the deadline (29.1 s) | exits after 10.1 s, `reason=supervisor-stdin-closed`, runtime killed |
| `replace-runtime` frame | ignored; the supervisor exits at the deadline, and the replacement never serves | the held runtime is killed after 10 s, and the replacement serves at 10.1 s |

In every case the supervisor exits 1: the held runtime could not dispose and
was killed, and the exit reports that.

**Existing paths** (`results/supervisor-exercise-8766-paths.json`). This
reruns `docs/evidence/runtime-owned-io-8766/scripts/supervisor_exercise.py` on
this change:

| Path | Result |
|---|---|
| Replace a serving runtime | new incarnation in 0.19 s; the old runtime reaped |
| Crash, then crash again | one automatic restart, then a pause with 503 on the API and the page |
| Recovery by restage | serves again |
| Stdin EOF | exit 0 in 0.06 s; the runtime reaped |
| Crash when launched without `--supervised` | exit 1, `DEV_HOST_RUNTIME_EXIT` |

**Start failures** (`results/restage-failure.txt`):

- a Product whose assembly is empty, as the first runtime: the supervisor
  exits 1 with `DEV_HOST_RUNTIME_EXIT: the runtime exited during startup`, as
  before;
- the same Product restaged: `DEV_HOST_RUNTIME_START`, then
  `DEV_HOST_RUNTIME_PAUSED` with 503. The next good restage serves after 0.4 s.

## Limits

A runtime that cannot answer, because it is stopped or hung while loading, is
killed after 10 seconds instead of immediately. That is the bounded stop the
task asked for. A slow loader gets the time to exit on EOF and dispose.

## Validation

- `cargo test -p csharp-product-runtime` passes.
- `cargo clippy -p csharp-product-runtime --all-targets --no-deps -- -D warnings`
  is clean.
