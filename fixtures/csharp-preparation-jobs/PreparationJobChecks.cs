using System;
using System.Diagnostics;
using System.Threading;
using Rusty.Engine;

namespace SdkPackageConsumer;

/// <summary>
/// The product's worker lane (<see cref="ProductCreateContext.Jobs"/>): a job's result comes back
/// once through TryTake, a job's exception is thrown there, disposing cancels and drops, a Debug
/// build refuses an Engine call from inside a job naming the call and the job, and a restart
/// cancels every job, finished or not.
/// </summary>
internal sealed class PreparationJobChecks
{
    private const string Texture = "spatial-particle.png";
    private static readonly TimeSpan Patience = TimeSpan.FromSeconds(20);

    private readonly PreparationJobs jobs;
    private PreparationJob<int>? runningAtRestart;
    private PreparationJob<int>? finishedAtRestart;
    private int runningWorkerEnded;

    internal PreparationJobChecks(PreparationJobs jobs) => this.jobs = jobs;

    /// <summary>Runs in an Update; leaves two jobs for <see cref="Restarted"/>.</summary>
    internal void Run(IEngineContext engine)
    {
        // A result comes back once.
        PreparationJob<int> sum = jobs.Run(_ => 20 + 22);
        WaitReady(sum, "a finished job");
        Require(sum.TryTake(out int answer) && answer == 42, "TryTake did not hand over the job's result");
        Require(!sum.TryTake(out _), "TryTake handed over a result twice");
        sum.Dispose();

        // A job's exception is thrown on the update.
        PreparationJob<int> failing = jobs.Run<int>(_ => throw new FormatException("preparation failed on purpose"));
        WaitReady(failing, "a failed job");
        try
        {
            failing.TryTake(out _);
            throw new InvalidOperationException("TryTake did not throw the job's exception.");
        }
        catch (FormatException error) when (error.Message == "preparation failed on purpose") { }

        // Disposing a running job cancels its token and drops what it makes.
        using ManualResetEventSlim started = new();
        using ManualResetEventSlim cancelled = new();
        PreparationJob<int> blocked = jobs.Run(token =>
        {
            started.Set();
            token.WaitHandle.WaitOne(Patience);
            if (token.IsCancellationRequested) cancelled.Set();
            return 7;
        });
        Require(started.Wait(Patience), "a job never started");
        blocked.Dispose();
        Require(blocked.IsCancelled, "a disposed job does not say it was cancelled");
        Require(cancelled.Wait(Patience), "a disposed job's token was not cancelled");
        Thread.Sleep(50);
        Require(!blocked.IsReady && !blocked.TryTake(out _), "a disposed job handed over its result");

        // A Debug build refuses an Engine call from inside a job, naming the call and the job, and
        // TryTake throws that refusal on the update. The lane's next Engine call does not raise it again.
        PreparationJob<RenderResourceInfo> offLane = jobs.Run(_ => engine.Graphics.OpenResource(new RenderResourceRequest(Texture)));
        WaitReady(offLane, "a job that called the Engine");
        try
        {
            offLane.TryTake(out _);
            throw new InvalidOperationException("An Engine call from inside a preparation job was not refused.");
        }
        catch (InvalidOperationException refusal) when (refusal.Message.Contains("was refused", StringComparison.Ordinal))
        {
            foreach (string expected in new[]
            {
                "Rusty Engine Graphics.OpenResource was called from thread",
                "outside the product's Engine call lane",
                "from inside the preparation job of type PreparationJob<RenderResourceInfo> run by Run (PreparationJobChecks.cs:",
                "PreparationJob.TryTake",
                "\"Engine call affinity\"",
            })
            {
                Require(refusal.Message.Contains(expected, StringComparison.Ordinal), $"the in-job refusal does not say '{expected}': {refusal.Message}");
            }
        }

        RenderResourceInfo onLane = engine.Graphics.OpenResource(new RenderResourceRequest(Texture));
        Require(!onLane.Handle.Equals(default(RenderResource)), "the lane's next Engine call after a claimed refusal failed");

        // For the restart: one job still running, one finished and not taken.
        runningAtRestart = jobs.Run(token =>
        {
            try
            {
                token.WaitHandle.WaitOne(Patience);
                return 1;
            }
            finally
            {
                Interlocked.Exchange(ref runningWorkerEnded, 1);
            }
        });
        finishedAtRestart = jobs.Run(_ => 2);
        WaitReady(finishedAtRestart, "a job finished before the restart");
        Console.WriteLine("PREPARATION_JOB_CHECKS_RAN");
    }

    /// <summary>Runs in the product's Restart: the generation ended, so both jobs are cancelled.</summary>
    internal void Restarted()
    {
        if (runningAtRestart is null || finishedAtRestart is null) return;
        Require(runningAtRestart.IsCancelled && finishedAtRestart.IsCancelled, "a restart did not cancel the product's jobs");
        Stopwatch waited = Stopwatch.StartNew();
        while (Volatile.Read(ref runningWorkerEnded) == 0 && waited.Elapsed < Patience) Thread.Sleep(1);
        Require(Volatile.Read(ref runningWorkerEnded) == 1, "a running job did not see its token cancelled by the restart");
        Thread.Sleep(50);
        Require(!runningAtRestart.TryTake(out _), "a job running at the restart handed over its result");
        Require(!finishedAtRestart.IsReady && !finishedAtRestart.TryTake(out _), "a job finished before the restart handed over its result");
        runningAtRestart = null;
        finishedAtRestart = null;
        Console.WriteLine("PREPARATION_JOB_CHECKS_PASSED");
    }

    private static void WaitReady<T>(PreparationJob<T> job, string what)
    {
        Stopwatch waited = Stopwatch.StartNew();
        while (!job.IsReady && waited.Elapsed < Patience) Thread.Sleep(1);
        Require(job.IsReady, $"{what} never became ready");
    }

    private static void Require(bool condition, string failure)
    {
        if (!condition) throw new InvalidOperationException($"Preparation job check failed: {failure}.");
    }
}
