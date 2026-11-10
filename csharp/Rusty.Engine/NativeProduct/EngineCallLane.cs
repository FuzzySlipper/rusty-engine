using System.ComponentModel;

namespace Rusty.Engine.NativeProduct;

/// <summary>
/// The Engine's call lane: the callbacks the Engine invokes on a product (construction, lifecycle,
/// update, debug commands, disposal). Engine services may be called only inside one, on its thread
/// (docs/world-streaming-contract.md, "Engine call affinity"). A Debug product build checks every
/// Engine call against it and refuses one from anywhere else before it reaches native code, where it
/// would race the update's own calls and corrupt memory. Release and NativeAOT builds do not check.
/// </summary>
internal static class EngineCallLane
{
    // Set once, at bind, from the product's own build configuration.
    private static bool s_checking;
    [ThreadStatic] private static int t_depth;
    private static int s_laneThread;
    private static string? s_laneThreadName;
    // The first refusal not yet raised on the lane, so a worker that swallows its exception
    // cannot hide it.
    private static string? s_pending;

    internal static void Enable() => s_checking = true;

    /// <summary>Marks the calling thread as inside an Engine callback until the returned scope ends.</summary>
    internal static Scope Enter()
    {
        if (!s_checking) return default;
        if (t_depth++ == 0)
        {
            s_laneThread = Environment.CurrentManagedThreadId;
            s_laneThreadName = Thread.CurrentThread.Name;
        }
        return new Scope(true);
    }

    /// <summary>
    /// Refuses an Engine call made outside every Engine callback. On the lane, raises a refusal made
    /// earlier on another thread, once.
    /// </summary>
    internal static void Check(string service, string operation)
    {
        if (!s_checking) return;
        if (t_depth == 0) Refuse(service, operation);
        else if (Volatile.Read(ref s_pending) is not null) RaisePending(service, operation);
    }

    private static void Refuse(string service, string operation)
    {
        Thread caller = Thread.CurrentThread;
        int lane = s_laneThread;
        string callerName = caller.Name ?? (caller.IsThreadPoolThread ? "a .NET thread-pool worker" : "an unnamed thread");
        string laneText = lane == 0
            ? "No Engine callback has run on any thread yet."
            : $"The lane's last Engine callback ran on thread '{s_laneThreadName ?? "unnamed"}' (managed thread {lane}).";
        string message =
            $"Rusty Engine {service}.{operation} was called from thread '{callerName}' (managed thread " +
            $"{Environment.CurrentManagedThreadId}{(caller.IsThreadPoolThread ? ", thread pool" : string.Empty)}), " +
            "outside the product's Engine call lane, and was refused before it reached the Engine. " +
            "Engine services, including content reads, ContentReference and handle Dispose, may only be called " +
            "synchronously inside a callback the Engine invoked (product construction, Start, Update, Pause, Resume, " +
            "Restart, Shutdown, a debug command or the product's Dispose), on that callback's own thread. " +
            laneText + " " +
            "The Engine's native state is not shared between threads: a call from another thread races the update's own " +
            "calls and corrupts the native heap, which shows up much later as an unrelated crash. " +
            "The usual cause is a sampler (IHeightfieldSampler, IVegetationSampler), Task.Run, a timer, an async " +
            "continuation or a thread-pool worker that reaches an Engine service, often indirectly through a lazy or " +
            "cached read of a ProductContentBundle. Fix: read or create what the worker needs on the update, and hand " +
            "the worker plain product-owned values (bytes, arrays, records). See docs/world-streaming-contract.md, " +
            "\"Engine call affinity\". This check runs in Debug product builds only.";
        string stack = Environment.StackTrace;
        Console.Error.WriteLine(message);
        Console.Error.WriteLine(stack);
        Interlocked.CompareExchange(ref s_pending, $"{message}{Environment.NewLine}The refused call's stack on thread '{callerName}':{Environment.NewLine}{stack}", null);
        throw new InvalidOperationException(message);
    }

    private static void RaisePending(string service, string operation)
    {
        string? pending = Interlocked.Exchange(ref s_pending, null);
        if (pending is null) return;
        throw new InvalidOperationException(
            $"Rusty Engine {service}.{operation} was stopped on the Engine's call lane because an Engine call was refused " +
            "earlier on another thread, and that thread's exception may have been caught or lost. This call did not run. " +
            $"The earlier refusal:{Environment.NewLine}{pending}");
    }

    [EditorBrowsable(EditorBrowsableState.Never)]
    internal readonly struct Scope(bool entered) : IDisposable
    {
        public void Dispose()
        {
            if (entered) t_depth--;
        }
    }
}
