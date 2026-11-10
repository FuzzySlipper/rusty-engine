using System;
using System.Threading;
using Rusty.Engine;

namespace SdkPackageConsumer;

/// <summary>
/// A Debug product build refuses an Engine call made off the Engine's call lane, before it reaches
/// native code, with a message that names the call, the thread and the fix. The lane's next Engine
/// call raises that refusal once, with the worker's stack; after it, calls on the lane work.
/// </summary>
internal static class CallLaneChecks
{
    private const string Texture = "spatial-particle.png";
    private const string WorkerName = "call-lane-check worker";

    private static readonly string[] Explains =
    [
        "Rusty Engine Graphics.OpenResource was called from thread 'call-lane-check worker'",
        "outside the product's Engine call lane",
        "The lane's last Engine callback ran on thread",
        "Task.Run",
        "hand the worker plain product-owned values",
        "docs/world-streaming-contract.md",
        "\"Engine call affinity\"",
    ];

    internal static void Run(IEngineContext engine)
    {
        Exception? refused = null;
        Thread worker = new(() =>
        {
            try
            {
                engine.Graphics.OpenResource(new RenderResourceRequest(Texture));
            }
            catch (Exception error)
            {
                refused = error;
            }
        })
        { Name = WorkerName };
        worker.Start();
        worker.Join();

        if (refused is not InvalidOperationException refusal)
        {
            throw new InvalidOperationException($"An Engine call from another thread was not refused: {refused?.ToString() ?? "it succeeded"}");
        }

        foreach (string expected in Explains)
        {
            if (!refusal.Message.Contains(expected, StringComparison.Ordinal))
            {
                throw new InvalidOperationException($"The off-lane refusal does not say '{expected}': {refusal.Message}");
            }
        }

        // The next call on the lane raises the earlier refusal once, with the worker's stack, so a
        // worker that swallowed its exception cannot hide it.
        try
        {
            engine.Graphics.OpenResource(new RenderResourceRequest(Texture));
            throw new InvalidOperationException("The lane's next Engine call did not raise the earlier off-lane refusal.");
        }
        catch (InvalidOperationException raised) when (raised.Message.Contains("refused earlier on another thread", StringComparison.Ordinal))
        {
            foreach (string expected in new[] { "This call did not run", Explains[0], "The refused call's stack on thread 'call-lane-check worker'", nameof(CallLaneChecks) })
            {
                if (!raised.Message.Contains(expected, StringComparison.Ordinal))
                {
                    throw new InvalidOperationException($"The raised earlier refusal does not say '{expected}': {raised.Message}");
                }
            }
        }

        RenderResourceInfo onLane = engine.Graphics.OpenResource(new RenderResourceRequest(Texture));
        if (onLane.Handle.Equals(default(RenderResource)))
        {
            throw new InvalidOperationException("The same call on the Engine's call lane returned no handle.");
        }

        Console.WriteLine("CALL_LANE_CHECKS_PASSED");
    }
}
