using System;
using System.Numerics;
using Rusty.Engine;

// Borrowed Dynamics results (#8744): the generated binding copies Engine-owned
// result memory before returning, so a copy must stay intact after later calls
// reuse and grow the same native buffer.
internal static class DynamicsResultChecks
{
    private const float StepSeconds = 1.0f / 60.0f;
    private const int GrownSelection = 64;

    internal static void Run(IEngineContext engine)
    {
        using DynamicsWorld world = engine.Dynamics.CreateWorld(new(new Vector3(0, -9.81f, 0)));
        using DynamicsBody first = CreateBody(engine, world, new Vector3(0, 0, 0));
        using DynamicsBody second = CreateBody(engine, world, new Vector3(2, 0, 0));

        DynamicsAction push = new(first, new Vector3(3, 0, 0), default, default, default, true);
        DynamicsStepAndReadResult ordinary = engine.Dynamics.StepAndRead(new(world, StepSeconds, 1, new[] { push }, new[] { second, first }));
        Require(ordinary.Bodies.Length == 2
            && ordinary.Bodies.Span[0].Body.Value == second.Handle.Value
            && ordinary.Bodies.Span[1].Body.Value == first.Handle.Value
            && ordinary.Bodies.Span[1].Readout.LinearVelocity.X > 0,
            "ordinary step/read lost request order or readout");
        DynamicsBodyFact[] ordinaryCopy = ordinary.Bodies.ToArray();

        DynamicsStepAndReadResult empty = engine.Dynamics.StepAndRead(new(world, StepSeconds, 1, ReadOnlyMemory<DynamicsAction>.Empty, ReadOnlyMemory<DynamicsBody>.Empty));
        Require(empty.Bodies.IsEmpty && empty.Generation == ordinary.Generation + 1, "empty step/read did not step exactly once");

        // One call grows the bridge buffer; the step is never repeated to fit it.
        DynamicsBody[] many = new DynamicsBody[GrownSelection];
        for (int index = 0; index < many.Length; index++) many[index] = index % 2 == 0 ? first : second;
        DynamicsStepAndReadResult grown = engine.Dynamics.StepAndRead(new(world, StepSeconds, 1, ReadOnlyMemory<DynamicsAction>.Empty, many));
        Require(grown.Generation == empty.Generation + 1 && grown.Bodies.Length == many.Length, "grown step/read was repeated or truncated");
        for (int index = 0; index < many.Length; index++)
            Require(grown.Bodies.Span[index].Body.Value == many[index].Handle.Value, "grown step/read lost request order");

        DynamicsWorldResult state = engine.Dynamics.ReadWorld(new(world));
        Require(state.Generation == grown.Generation && state.Bodies.Length == 2, "world result did not report both retained bodies");

        // Earlier results are managed copies, untouched by the later calls.
        Require(ordinary.Bodies.Span.SequenceEqual(ordinaryCopy) && grown.Bodies.Length == GrownSelection,
            "an earlier result changed when the native buffer was reused");
        Console.WriteLine("DYNAMICS_RESULT_CHECKS_PASSED");
    }

    private static DynamicsBody CreateBody(IEngineContext engine, DynamicsWorld world, Vector3 position) =>
        engine.Dynamics.CreateBody(new(world, new DynamicsBodyConfig(
            new Transform(position, Quaternion.Identity, Vector3.One), new Vector3(0.5f), 1.0f,
            new DynamicsMassPolicy(DynamicsMassPolicyKind.DeriveFromShapeAndMass, default), default, 0.0f)));

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException($"Dynamics result check failed: {message}");
    }
}
