# Persistence queue

Package-consumer fixture for queued saves and cache stores
([persistence](../../docs/csharp-implicit-surfaces.md#explicit-capture-restore-and-live-inspection)).
Each run first loads its 64 keys, as a cache looks a key up before writing
it. Then every update saves an 8 KB blob to one of them, guarded by the
revision it replaces, and times the `Persistence.Save` call. It runs three times:

1. `written-durable`: written in the call to a durable store (120 updates),
   the baseline.
2. `queued-durable`: `Deferred` saves to a durable store (600 updates).
3. `queued-cache`: `Deferred` saves to a store opened with `Cache` (600
   updates).

After each run it waits until `ReadSaves` reports every ticket finished,
checks that `Load` reads each key's newest blob and revision and that a guard
naming an older revision is refused, then queues 256 saves in one update and
times how long they take to be written. Each update also times a
`ReadSaves` call, which touches no file, as a control for what the host's
load alone costs a call. Each run logs a `PERSISTENCE_QUEUE` line with the
Save call's p50, p99 and maximum, the control's p99 and maximum, and the
burst's time per write. The last line is `PERSISTENCE_QUEUE_PASSED`, or
`PERSISTENCE_QUEUE_FAILED` if a queued `Save` took 0.5 ms or more;
`persistence.report` returns the same lines. Judge the timings on a machine
with no other load: on a busy one the control shows the same outliers.

```bash
export RustyEngineFixtureSdkVersion=<pair-version>
dotnet restore CsharpPersistenceQueue.csproj --source /path/to/pair/sdk-feed
/path/to/pair/runtime-pack/bin/rusty dev \
  --project CsharpPersistenceQueue.csproj --runtime /path/to/pair/runtime-pack
```

The runtime steps once a page or an SSE client attaches.
