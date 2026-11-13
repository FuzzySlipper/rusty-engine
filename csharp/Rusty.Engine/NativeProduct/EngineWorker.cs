using System.Runtime.ExceptionServices;

namespace Rusty.Engine.NativeProduct;

/// <summary>
/// Runs product work off the Engine's call lane: preparation jobs and the heightfield and
/// vegetation samplers, on the .NET thread pool. The work reads only product data and calls no
/// Engine service; a Debug build's <see cref="EngineCallLane"/> names the work when it does.
/// </summary>
internal static class EngineWorker
{
    // What this thread is running, for a refused Engine call to name.
    [ThreadStatic] private static object? t_work;

    /// <summary>The work this thread is running, described; null outside every worker.</summary>
    internal static string? Running => t_work switch
    {
        null => null,
        IDescribedWork described => described.Describe(),
        Heightfields.IHeightfieldSampler sampler => $"the heightfield sampler {sampler.GetType().FullName}.Sample",
        VegetationTiles.IVegetationSampler sampler => $"the vegetation sampler {sampler.GetType().FullName}.Sample",
        object other => other.GetType().FullName,
    };

    /// <summary>
    /// Runs <paramref name="work"/> on the thread pool unless <paramref name="token"/> is already
    /// cancelled. <paramref name="belowNormal"/> lowers the thread on Windows for the work's length,
    /// so work that fills every core leaves the update and the window theirs (#10718); elsewhere a
    /// lowered thread may not be raised again, so it is left as it is.
    /// </summary>
    internal static void Run(object described, bool belowNormal, Action work, CancellationToken token) =>
        _ = Task.Run(() =>
        {
            if (token.IsCancellationRequested) return;
            Thread worker = Thread.CurrentThread;
            ThreadPriority priority = worker.Priority;
            bool lowered = belowNormal && OperatingSystem.IsWindows();
            if (lowered) worker.Priority = ThreadPriority.BelowNormal;
            t_work = described;
            try
            {
                work();
            }
            finally
            {
                t_work = null;
                if (lowered) worker.Priority = priority;
            }
        }, token);

    /// <summary>
    /// Throws a worker's exception on the lane. A call-lane refusal it carries is then raised, so
    /// the lane's next Engine call does not raise it a second time.
    /// </summary>
    [System.Diagnostics.CodeAnalysis.DoesNotReturn]
    internal static void Rethrow(ExceptionDispatchInfo failure)
    {
        EngineCallLane.Claim(failure.SourceException);
        failure.Throw();
    }
}

/// <summary>Work that says what it is when an Engine call from it is refused.</summary>
internal interface IDescribedWork
{
    string Describe();
}
