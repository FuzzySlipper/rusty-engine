# Preparation jobs

`PreparationJobChecks.cs` runs in the SDK consumer's first admitted Update via
`scripts/test-csharp-sdk-package.sh --coreclr-smoke`, and again in the
NativeAOT run. The consumer is a Debug build, so its call-lane check is on.

A job's result comes back once through `TryTake`, and a job's exception is
thrown there. Disposing a running job cancels its token and drops what it
makes. An Engine call from inside a job is refused, naming the call and the
member, file and line that ran the job; `TryTake` throws the refusal, and the
lane's next call works. The runtime exercise then faults and restarts the
product: its `Restart` checks that a job still running and a job finished but
not taken are both cancelled and hand nothing over. The script requires
`PREPARATION_JOB_CHECKS_PASSED` in the CoreCLR exercise's output.
