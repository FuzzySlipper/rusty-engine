# Persistent Studio concurrency exploration

Task 6748 evaluated whether the persistent generic Studio service should become
a concurrent-session owner. The recommendation is **no-go for a shared-address
multi-session implementation now**. Keep the service deliberately
single-session and use independently started hosts for work that must overlap.

This is a point-in-time exploration, not a new authority layer. The durable
operating rule remains in [Persistent generic Studio service](../topics/studio-service.md).

## Useful concurrency profiles

| Profile | Value | Required isolation |
|---|---|---|
| One interactive user with several tabs | Low | None; tabs intentionally observe one active project |
| Two independent projects used by a human and automation | Real | Separate host ports and settings roots |
| Two agents changing one project checkout | Unsafe without coordination | Separate project copies/worktrees as well as separate hosts |
| Several persistent users behind one address | Not currently evidenced | Per-session adapter, settings, routing, limits, cleanup, and visible identity |

Hash and revision guards remain useful stale-write rejection. They do not make
the first three shared-state profiles isolated, and a new service routing layer
would not make concurrent writes to the same downstream files safe.

## Reproduced behavior

The focused process-level probe is
[`studio/test/studio-concurrency.test.ts`](../../studio/test/studio-concurrency.test.ts).
It starts the real generic HTTP host and two root-local protocol-15 adapters.
Its two clients use independent HTTP connection pools.

Against one host, client A opened project A. Client B then opened project B.
Client A's subsequent status and adapter `describe` requests both observed
project B and adapter B. The host had closed adapter A before publishing the
second open. This confirms redirection, rather than merely inferring it from
the process-wide `StudioAdapterHost` field layout.

Against two hosts on different loopback ports and settings roots, the same two
clients retained project A and project B independently. Terminating both hosts
caused every fixture adapter process group to exit within the bounded cleanup
window. Each fixture adapter starts a recorded descendant that shares its OS
process-group identity. Before every close, the probe confirms that the leader,
descendant, and negative process-group identity are live. During cleanup, the
descendant records that its leader has exited and the probe requires the
descendant and group identity to remain live before it awaits the host result.
Immediately after a project switch or host shutdown returns, it then requires
all three identities to be absent. A leader-only cleanup therefore cannot
satisfy the assertion. The production close path also reports an error if the
group remains after its final `SIGKILL` wait.

The focused command is:

```bash
pnpm --dir studio exec tsx --test test/studio-concurrency.test.ts
```

On the 2026-08-10 descendant-bearing development run, the test completed in
1.49 seconds and
reported these Linux `VmRSS` samples:

| Shape | Host RSS | Adapter-leader RSS | Live topology |
|---|---:|---:|---|
| Shared host after a project switch | 109,684 KiB | 69,880 KiB | 1 host + 1 adapter group |
| Isolated host A | 95,152 KiB | 69,468 KiB | 1 host + 1 adapter group |
| Isolated host B | 95,868 KiB | 69,408 KiB | 1 host + 1 adapter group |

These samples bound the tested Node/tsx fixture shape, not every downstream
Rust adapter. The evidence-only descendant sentinel is excluded from the RSS
column. The stable resource conclusion is topological: each concurrently
isolated user adds one host and one adapter process group, while project
switching on one host returns to one group. Startup, switch, host shutdown, and
group-wide adapter cleanup are all exercised by the checked probe.

## Options considered

### Keep one session and make isolation explicit

This is the lowest-complexity option and the recommendation. Existing status
already exposes the selected root, file, adapter, and source identity. The
service documentation explicitly directs concurrent agents and automation to
unique loopback ports and settings roots, and the new checked probe prevents
that advice from drifting away from actual host behavior.

A lease or warning banner could make accidental overlap louder, but it would
still require ownership, expiry, recovery, and client identity rules. It would
also reject work without isolating it. There is not enough current evidence to
add that protocol and operational state.

### Session-key adapter slots in one host

Keeping several `StudioAdapterHost` instances in one process sounds smaller
than it is. The session key must be carried consistently by initial HTML
navigation, status, adapter requests, user settings, resource reads, and every
reconnect. The host would need per-session limits, idle expiry, project-switch
transactions, crash containment, and complete cleanup proof. A missing key on
one route would silently recover today's split-authority problem.

### Process pool behind a shared address

A supervising proxy could provide stronger failure containment and preserve
one bookmarked address, but it adds routing, lifecycle, resource quotas,
health aggregation, and orphan cleanup. It still cannot isolate writes to the
same project checkout. That is a significant product and operations feature,
not a small extension of the trusted-local service.

## Go/no-go threshold

Reopen multi-session development only when all of the following are true:

1. At least two independent interactive Studio sessions on one machine are a
   recurring workflow, not an occasional test or agent overlap.
2. Separate loopback hosts and settings roots have a measured material cost or
   usability failure beyond their roughly linear process/memory cost.
3. A named owner is willing to define session identity, route completeness,
   limits, expiry, crash recovery, and shutdown acceptance without weakening
   downstream file ownership.
4. The implementation includes a two-client real-host probe that proves no
   cross-session root, adapter, settings, resource, or mutation crossover.

Until that threshold is met, a shared-address session manager would cost more
authority and lifecycle complexity than the demonstrated demand warrants.
