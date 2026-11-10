# Engine call-lane refusal

`CallLaneChecks.cs` runs in the immutable SDK consumer's first admitted Update
via `scripts/test-csharp-sdk-package.sh --coreclr-smoke`. The consumer is a
Debug build, so its generated bind export turns the call-lane check on.

A named thread calls `Graphics.OpenResource`. The call must be refused before
it reaches the Engine, with a message that names the call and the thread and
points to [Engine call affinity](../../docs/world-streaming-contract.md#engine-call-affinity).
The same call on the Update's own thread must still succeed.
