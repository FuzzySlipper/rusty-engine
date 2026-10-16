using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.Numerics;
using System.Runtime.ExceptionServices;
using System.Threading;
using System.Threading.Tasks;

namespace Rusty.Engine.VegetationTiles;

/// <summary>
/// Fills one vegetation tile's samples. Called on worker threads, several tiles at once, so it
/// reads only product-owned data the product will not change under it (its recipe, its world map,
/// precomputed placement tiles) and calls no Engine service.
/// </summary>
public interface IVegetationSampler
{
    void Sample(VegetationTileRequest tile, VegetationTileSamples samples);
}

/// <summary>
/// One tile's samples as an <see cref="IVegetationSampler"/> fills them: a height at every vertex
/// and one more each side (NaN where there is no ground), and optionally at every vertex a ground
/// class (0 to 63) and a cover weight (0 to 1), and the things the product places itself.
/// </summary>
public sealed class VegetationTileSamples
{
    private readonly int heightSide;
    private readonly int side;

    internal VegetationTileSamples(VegetationTileRequest tile)
    {
        Tile = tile;
        heightSide = tile.HeightSide;
        side = tile.Side;
        Heights = new float[heightSide * heightSide];
    }

    public VegetationTileRequest Tile { get; }

    /// <summary>Heights, <c>HeightSide</c>² from vertex (−1, −1), x fastest.</summary>
    public float[] Heights { get; }

    /// <summary>Ground classes, <c>Side</c>² from vertex (0, 0), x fastest; null for class 0 everywhere.</summary>
    public uint[]? Grounds { get; private set; }

    /// <summary>Cover weights, <c>Side</c>²; null for 1 everywhere.</summary>
    public float[]? Cover { get; private set; }

    /// <summary>Things placed as given, x and z from the tile's origin and y the world height.</summary>
    public List<VegetationRecord> Records { get; } = [];

    /// <summary>The height at vertex (i, j), −1 to <c>TileCells + 1</c>.</summary>
    public void SetHeight(int i, int j, float height) => Heights[((j + 1) * heightSide) + i + 1] = height;

    public float Height(int i, int j) => Heights[((j + 1) * heightSide) + i + 1];

    /// <summary>The ground class at vertex (i, j), 0 to <c>TileCells</c>.</summary>
    public void SetGround(int i, int j, uint ground) => (Grounds ??= new uint[side * side])[(j * side) + i] = ground;

    /// <summary>The cover weight at vertex (i, j), 0 to <c>TileCells</c>; the share of every layer's spots kept there.</summary>
    public void SetCover(int i, int j, float cover)
    {
        if (Cover is null)
        {
            Cover = new float[side * side];
            Array.Fill(Cover, 1f);
        }

        Cover[(j * side) + i] = cover;
    }
}

/// <summary>
/// Streams a vegetation field's tiles: each <see cref="Update"/> reads the tiles the Engine asks
/// for, samples new ones with the product's <see cref="IVegetationSampler"/> on the thread pool,
/// and commits at most <c>commitsPerUpdate</c> finished ones, each first passed to
/// <c>beforeCommit</c> on the update's thread (where the product may read state its workers may
/// not, such as edits). Call <see cref="Update"/> from the product's update; every Engine call
/// happens there. Samples the Engine no longer wants are ignored when committed.
/// </summary>
public sealed class VegetationStreamer : IDisposable
{
    private readonly IEngineContext engine;
    private readonly VegetationField field;
    private readonly IVegetationSampler sampler;
    private readonly Action<VegetationTileSamples>? beforeCommit;
    private readonly int commitsPerUpdate;
    private readonly int maxInFlight;
    private readonly HashSet<(long X, long Z, uint Revision)> inFlight = [];
    private readonly ConcurrentQueue<VegetationTileSamples> finished = new();
    private readonly ConcurrentQueue<(VegetationTileRequest Tile, ExceptionDispatchInfo Failure)> failed = new();
    private readonly CancellationTokenSource stopping = new();
    private long sampleTicks;

    /// <param name="commitsPerUpdate">The most tiles committed an update.</param>
    /// <param name="maxInFlight">The most tiles sampled at once; 0 for twice the processors.</param>
    public VegetationStreamer(IEngineContext engine, VegetationField field, IVegetationSampler sampler,
        Action<VegetationTileSamples>? beforeCommit = null, int commitsPerUpdate = 16, int maxInFlight = 0)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(commitsPerUpdate);
        ArgumentOutOfRangeException.ThrowIfNegative(maxInFlight);
        this.engine = engine ?? throw new ArgumentNullException(nameof(engine));
        this.field = field ?? throw new ArgumentNullException(nameof(field));
        this.sampler = sampler ?? throw new ArgumentNullException(nameof(sampler));
        this.beforeCommit = beforeCommit;
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
            VegetationTileRequest tile = failure.Tile;
            inFlight.Remove((tile.X, tile.Z, tile.Revision));
            failure.Failure.Throw();
        }

        for (int committed = 0; committed < commitsPerUpdate && finished.TryDequeue(out VegetationTileSamples? samples); committed++)
        {
            VegetationTileRequest tile = samples.Tile;
            inFlight.Remove((tile.X, tile.Z, tile.Revision));
            beforeCommit?.Invoke(samples);
            VegetationCommitReceipt receipt = engine.Vegetation.CommitTile(new VegetationTileCommit(field, tile.X, tile.Z, tile.Revision,
                samples.Heights, samples.Grounds ?? ReadOnlyMemory<uint>.Empty, samples.Cover ?? ReadOnlyMemory<float>.Empty,
                samples.Records.ToArray()));
            if (receipt.Accepted) Committed++;
            else Ignored++;
        }

        int free = maxInFlight - inFlight.Count;
        if (free <= 0) return;
        ReadOnlyMemory<VegetationTileRequest> requests =
            engine.Vegetation.ReadRequests(new VegetationRequestsRequest(field, (uint)(maxInFlight * 2)));
        foreach (VegetationTileRequest tile in requests.Span)
        {
            if (free == 0) break;
            if (!inFlight.Add((tile.X, tile.Z, tile.Revision))) continue;
            free--;
            CancellationToken token = stopping.Token;
            _ = Task.Run(() =>
            {
                if (token.IsCancellationRequested) return;
                long started = Stopwatch.GetTimestamp();
                VegetationTileSamples samples = new(tile);
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
