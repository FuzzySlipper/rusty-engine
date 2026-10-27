using System;
using System.Numerics;
using Rusty.Engine;

// Exercises two generated borrowed-result families through the packaged host.
// The returned ReadOnlyMemory must remain valid across the next call on that
// service, when its native bridge reuses the borrowed-result storage.
internal static class GeneratedResultCopyChecks
{
    public static void Run(IEngineContext engine, SpatialSession spatial)
    {
        PerceptionObserver[] observers =
        [
            new(1, Vector3.Zero, Vector3.UnitX, 10, 0.5, 0.75),
        ];
        PerceptionTarget[] targets =
        [
            new(2, new Vector3(1, 0, 0)),
            new(3, new Vector3(2, 0, 0)),
        ];
        PerceptionReadoutResult perception = engine.Perception.QueryVisibility(
            new(spatial, observers, targets, Array.Empty<SpatialEntityCollider>(), 0, 8));
        Require(perception.Pairs.Length == 2 && perception.Aggregates.Length == 2,
            "perception did not return both copied result collections");

        _ = engine.Perception.QueryVisibility(new(spatial, observers,
            new[] { new PerceptionTarget(4, new Vector3(3, 0, 0)) },
            Array.Empty<SpatialEntityCollider>(), 0, 8));
        Require(perception.Pairs.Length == 2
            && perception.Pairs.Span[0].Target == 2
            && perception.Pairs.Span[1].Target == 3
            && perception.Aggregates.Span[0].Target == 2
            && perception.Aggregates.Span[1].Target == 3,
            "perception results changed after a later query reused native storage");

        using WaterField field = engine.WaterField.Create(
            new WaterFieldCreateRequest(2, 1, 1, new float[] { 0, 0 }, Vector3.Zero)
            {
                Depth = new float[] { 1, 2 },
            });
        WaterFieldSampleRequest points = new(field, new WaterFieldPoint[]
        {
            new(0.5f, 0.5f),
            new(1.5f, 0.5f),
        });
        ReadOnlyMemory<WaterFieldSample> samples = engine.WaterField.Sample(points);
        ReadOnlyMemory<WaterFieldCell> cells = engine.WaterField.ReadCells(field);
        _ = engine.WaterField.Sample(new(field, new[] { new WaterFieldPoint(-1, -1) }));
        Require(samples.Length == 2 && samples.Span[0].Wet && samples.Span[0].Depth == 1
            && samples.Span[1].Wet && samples.Span[1].Depth == 2,
            "water samples changed after a later call reused native storage");
        Require(cells.Length == 2 && cells.Span[0].Depth == 1 && cells.Span[0].Surface == 1
            && cells.Span[1].Depth == 2 && cells.Span[1].Surface == 2,
            "water cell readout changed after a later call reused native storage");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition)
            throw new InvalidOperationException(message);
    }
}
