using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.Numerics;
using System.Runtime.ExceptionServices;
using System.Threading;
using System.Threading.Tasks;

namespace Rusty.Engine.Heightfields;

/// <summary>
/// Fills one heightfield tile's samples. Called on worker threads, several tiles at once, so it
/// reads only product-owned data the product will not change under it (its recipe, its world
/// map) and calls no Engine service.
/// </summary>
public interface IHeightfieldSampler
{
    void Sample(HeightfieldTileRequest tile, HeightfieldTileSamples samples);
}

/// <summary>
/// One tile's samples as an <see cref="IHeightfieldSampler"/> fills them: a height at every
/// vertex and one more each side (NaN where there is no surface), and optionally at every vertex
/// a linear colour or terrain layer weights, and a material index.
/// </summary>
public sealed class HeightfieldTileSamples
{
    private readonly int heightSide;
    private readonly int side;

    internal HeightfieldTileSamples(HeightfieldTileRequest tile)
    {
        Tile = tile;
        heightSide = tile.HeightSide;
        side = tile.Side;
        Heights = new float[heightSide * heightSide];
    }

    public HeightfieldTileRequest Tile { get; }

    /// <summary>Heights, <c>HeightSide</c>² from vertex (−1, −1), x fastest.</summary>
    public float[] Heights { get; }

    /// <summary>Linear RGBA colours, <c>Side</c>² from vertex (0, 0), x fastest; null for none.</summary>
    public Color[]? Colors { get; private set; }

    /// <summary>A terrain layer material's first four layers' weights; null for none.</summary>
    public Vector4[]? LayerWeights { get; private set; }

    /// <summary>Material indices; null for index 0 everywhere.</summary>
    public uint[]? Materials { get; private set; }

    /// <summary>The height at vertex (i, j), −1 to <c>TileCells + 1</c>.</summary>
    public void SetHeight(int i, int j, float height) => Heights[((j + 1) * heightSide) + i + 1] = height;

    public float Height(int i, int j) => Heights[((j + 1) * heightSide) + i + 1];

    /// <summary>The linear colour at vertex (i, j), 0 to <c>TileCells</c>.</summary>
    public void SetColor(int i, int j, Color color) => (Colors ??= new Color[side * side])[(j * side) + i] = color;

    /// <summary>The layer weights at vertex (i, j), 0 to <c>TileCells</c>.</summary>
    public void SetLayerWeights(int i, int j, Vector4 weights) =>
        (LayerWeights ??= new Vector4[side * side])[(j * side) + i] = weights;

    /// <summary>The material index at vertex (i, j), 0 to <c>TileCells</c>.</summary>
    public void SetMaterial(int i, int j, uint material) => (Materials ??= new uint[side * side])[(j * side) + i] = material;
}

/// <summary>
/// Streams a heightfield's tiles: each <see cref="Update"/> reads the tiles the Engine asks for,
/// samples new ones with the product's <see cref="IHeightfieldSampler"/> on the thread pool, and
/// commits at most <c>commitsPerUpdate</c> finished ones. Call <see cref="Update"/> from the
/// product's update; every Engine call happens there. Samples the Engine no longer wants (the
/// tile was invalidated or left the rings) are ignored when committed.
/// </summary>
public sealed class HeightfieldStreamer : IDisposable
{
    private readonly IEngineContext engine;
    private readonly Heightfield heightfield;
    private readonly IHeightfieldSampler sampler;
    private readonly int commitsPerUpdate;
    private readonly int maxInFlight;
    private readonly HashSet<(uint Ring, long X, long Z, uint Revision)> inFlight = [];
    private readonly ConcurrentQueue<HeightfieldTileSamples> finished = new();
    private readonly ConcurrentQueue<(HeightfieldTileRequest Tile, ExceptionDispatchInfo Failure)> failed = new();
    private readonly CancellationTokenSource stopping = new();
    private long sampleTicks;

    /// <param name="commitsPerUpdate">The most tiles committed an update: each costs its copy and,
    /// as the call finishes, its share of the Engine's parallel meshing.</param>
    /// <param name="maxInFlight">The most tiles sampled at once; 0 for twice the processors.</param>
    public HeightfieldStreamer(IEngineContext engine, Heightfield heightfield, IHeightfieldSampler sampler,
        int commitsPerUpdate = 8, int maxInFlight = 0)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(commitsPerUpdate);
        ArgumentOutOfRangeException.ThrowIfNegative(maxInFlight);
        this.engine = engine ?? throw new ArgumentNullException(nameof(engine));
        this.heightfield = heightfield ?? throw new ArgumentNullException(nameof(heightfield));
        this.sampler = sampler ?? throw new ArgumentNullException(nameof(sampler));
        this.commitsPerUpdate = commitsPerUpdate;
        this.maxInFlight = maxInFlight == 0 ? Environment.ProcessorCount * 2 : maxInFlight;
    }

    /// <summary>Tiles being sampled now.</summary>
    public int InFlight => inFlight.Count;

    /// <summary>Tiles the Engine took, and committed tiles it no longer wanted.</summary>
    public long Committed { get; private set; }

    public long Ignored { get; private set; }

    /// <summary>Time the sampler has taken, summed over tiles (they sample in parallel).</summary>
    public double SampleMilliseconds => Stopwatch.GetElapsedTime(0, Interlocked.Read(ref sampleTicks)).TotalMilliseconds;

    /// <summary>
    /// Commit finished tiles and start sampling new ones. A sampler that threw on a worker throws
    /// here, and its tile is asked for again.
    /// </summary>
    public void Update()
    {
        if (failed.TryDequeue(out var failure))
        {
            HeightfieldTileRequest tile = failure.Tile;
            inFlight.Remove((tile.Ring, tile.X, tile.Z, tile.Revision));
            failure.Failure.Throw();
        }

        for (int committed = 0; committed < commitsPerUpdate && finished.TryDequeue(out HeightfieldTileSamples? samples); committed++)
        {
            HeightfieldTileRequest tile = samples.Tile;
            inFlight.Remove((tile.Ring, tile.X, tile.Z, tile.Revision));
            HeightfieldCommitReceipt receipt = engine.Heightfield.CommitTile(new HeightfieldTileCommit(heightfield, tile.Ring, tile.X,
                tile.Z, tile.Revision, samples.Heights, samples.Colors ?? ReadOnlyMemory<Color>.Empty,
                samples.LayerWeights ?? ReadOnlyMemory<Vector4>.Empty, samples.Materials ?? ReadOnlyMemory<uint>.Empty));
            if (receipt.Accepted) Committed++;
            else Ignored++;
        }

        int free = maxInFlight - inFlight.Count;
        if (free <= 0) return;
        ReadOnlyMemory<HeightfieldTileRequest> requests =
            engine.Heightfield.ReadRequests(new HeightfieldRequestsRequest(heightfield, (uint)(maxInFlight * 2)));
        foreach (HeightfieldTileRequest tile in requests.Span)
        {
            if (free == 0) break;
            if (!inFlight.Add((tile.Ring, tile.X, tile.Z, tile.Revision))) continue;
            free--;
            CancellationToken token = stopping.Token;
            _ = Task.Run(() =>
            {
                if (token.IsCancellationRequested) return;
                long started = Stopwatch.GetTimestamp();
                HeightfieldTileSamples samples = new(tile);
                try
                {
                    sampler.Sample(tile, samples);
                }
                catch (Exception exception)
                {
                    failed.Enqueue((tile, ExceptionDispatchInfo.Capture(exception)));
                    return;
                }

                Interlocked.Add(ref sampleTicks, Stopwatch.GetTimestamp() - started);
                if (!token.IsCancellationRequested) finished.Enqueue(samples);
            }, token);
        }
    }

    /// <summary>Stops sampling: tiles being sampled finish on their workers and are dropped.</summary>
    public void Dispose()
    {
        stopping.Cancel();
        finished.Clear();
        inFlight.Clear();
    }
}
