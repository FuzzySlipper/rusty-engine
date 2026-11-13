using System.Diagnostics.CodeAnalysis;
using System.Runtime.CompilerServices;
using System.Runtime.ExceptionServices;
using Rusty.Engine.NativeProduct;

namespace Rusty.Engine;

/// <summary>
/// The product's worker lane (<see cref="ProductCreateContext.Jobs"/>): pure preparation work, such
/// as building a level's geometry, run on Engine workers beside the update so the window keeps
/// drawing. A job reads only product data the product will not change under it and calls no
/// Engine service; its result comes back on the update through <see cref="PreparationJob{T}.TryTake"/>,
/// where the product makes the Engine calls. Restart and Shutdown cancel every job and drop its
/// result. Call <see cref="Run{T}"/> from an Engine callback.
/// </summary>
public sealed class PreparationJobs
{
    private CancellationTokenSource generation = new();

    /// <summary>
    /// Starts <paramref name="prepare"/> on an Engine worker. It is passed a token that is cancelled
    /// when the job is disposed or the product restarts or shuts down; checking it is optional, since
    /// a cancelled job's result is dropped either way. On Windows the job runs below normal priority,
    /// so the update and the window keep their cores.
    /// </summary>
    public PreparationJob<T> Run<T>(Func<CancellationToken, T> prepare,
        [CallerMemberName] string member = "", [CallerFilePath] string file = "", [CallerLineNumber] int line = 0)
    {
        ArgumentNullException.ThrowIfNull(prepare);
        PreparationJob<T> job = new(generation.Token, member, file, line);
        EngineWorker.Run(job, belowNormal: true, () => job.Prepare(prepare), CancellationToken.None);
        return job;
    }

    /// <summary>A new lifecycle generation, or none: every job so far is cancelled.</summary>
    internal void CancelAll()
    {
        CancellationTokenSource ended = generation;
        generation = new CancellationTokenSource();
        ended.Cancel();
        ended.Dispose();
    }
}

/// <summary>
/// One preparation job (<see cref="PreparationJobs.Run{T}"/>). Read it on the update; dispose it to
/// cancel it.
/// </summary>
public sealed class PreparationJob<T> : IDisposable, IDescribedWork
{
    private readonly object gate = new();
    private readonly CancellationTokenSource cancellation;
    private readonly CancellationToken token;
    // The lifecycle generation's own token: a job that finished before a restart is dropped too,
    // though its token source is disposed by then and no longer linked.
    private readonly CancellationToken generation;
    private readonly string member;
    private readonly string file;
    private readonly int line;
    private bool finished;
    private bool taken;
    private bool disposed;
    private T? prepared;
    private ExceptionDispatchInfo? failure;

    internal PreparationJob(CancellationToken generation, string member, string file, int line)
    {
        this.generation = generation;
        cancellation = CancellationTokenSource.CreateLinkedTokenSource(generation);
        token = cancellation.Token;
        this.member = member;
        this.file = file;
        this.line = line;
    }

    /// <summary>The job has finished and <see cref="TryTake"/> will hand over its result or throw its exception.</summary>
    public bool IsReady
    {
        get
        {
            lock (gate) return finished && !taken && !disposed && !generation.IsCancellationRequested;
        }
    }

    /// <summary>The job was cancelled: disposed, or ended by a restart or shutdown.</summary>
    public bool IsCancelled
    {
        get
        {
            lock (gate) return disposed || token.IsCancellationRequested || generation.IsCancellationRequested;
        }
    }

    /// <summary>
    /// Hands over the job's result once it is ready, once. Throws the job's exception here, on the
    /// update, when it threw. False while it runs, after it was cancelled and after the result was taken.
    /// </summary>
    public bool TryTake([MaybeNullWhen(false)] out T result)
    {
        T? taking;
        ExceptionDispatchInfo? thrown;
        lock (gate)
        {
            result = default;
            if (!finished || taken || disposed || generation.IsCancellationRequested) return false;
            taken = true;
            taking = prepared;
            thrown = failure;
            prepared = default;
            failure = null;
        }

        if (thrown is not null) EngineWorker.Rethrow(thrown);
        result = taking!;
        return true;
    }

    /// <summary>Cancels the job and drops its result, whether or not it has finished.</summary>
    public void Dispose()
    {
        lock (gate)
        {
            if (disposed) return;
            disposed = true;
            // A running job's worker still owns the token source, and disposes it as the job ends.
            if (!finished) cancellation.Cancel();
            prepared = default;
            failure = null;
        }
    }

    internal void Prepare(Func<CancellationToken, T> prepare)
    {
        T? made = default;
        ExceptionDispatchInfo? thrown = null;
        if (!token.IsCancellationRequested)
        {
            try
            {
                made = prepare(token);
            }
            catch (Exception exception)
            {
                thrown = ExceptionDispatchInfo.Capture(exception);
            }
        }

        lock (gate)
        {
            finished = true;
            if (!disposed && !token.IsCancellationRequested)
            {
                prepared = made;
                failure = thrown;
            }

            cancellation.Dispose();
        }
    }

    string IDescribedWork.Describe() =>
        $"the preparation job of type PreparationJob<{typeof(T).Name}> run by {member} ({Path.GetFileName(file)}:{line})";
}
