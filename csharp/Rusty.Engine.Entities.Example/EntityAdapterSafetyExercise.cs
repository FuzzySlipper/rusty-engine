using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Entities;

// Focused adapter-contract checks kept apart from the broad example program.
// They run with the proof harness and exercise the outcome when a native
// Dynamics step has already happened but managed publication becomes stale.
internal static class EntityAdapterSafetyExercise
{
    internal static void Run()
    {
        DynamicsDoesNotPretendToRollbackAfterManagedPublicationBecomesStale();
    }

    private static void DynamicsDoesNotPretendToRollbackAfterManagedPublicationBecomesStale()
    {
        using var entities = new EntityStore([
            EngineComponentTypes.Transform,
            EngineComponentTypes.DynamicsMotion]);
        EntityId entity = entities.Create();
        Transform initialTransform = new(Vector3.Zero, Quaternion.Identity, new Vector3(2.0f, 3.0f, 4.0f));
        entities.Set(entity, EngineComponentTypes.Transform, initialTransform);
        entities.Set(entity, EngineComponentTypes.DynamicsMotion, new DynamicsMotion(Vector3.Zero, Vector3.Zero, false));

        var service = new AdvancingDynamicsService(() => entities.Set(
            entity,
            EngineComponentTypes.DynamicsMotion,
            new DynamicsMotion(Vector3.UnitY, Vector3.Zero, false)));
        using var world = new DynamicsWorld(new DynamicsWorldHandle(71), static () => { });
        using var body = new DynamicsBody(new DynamicsBodyHandle(72), static () => { });
        var adapter = new EntityDynamicsAdapter(entities, service, world);

        Throws(() => adapter.Step(
            stepSeconds: 1.0f / 60.0f,
            steps: 1,
            bindings: new[] { new DynamicsEntityBinding(entity, body) },
            actions: Array.Empty<DynamicsEntityAction>(),
            maximumBodies: 1,
            maximumActions: 0),
            "a post-step managed mutation must reject Dynamics publication");

        Require(service.StepAndReadCalls == 1,
            "the dedicated fake did not model an already-completed native Dynamics step");
        Require(entities.Get(entity, EngineComponentTypes.Transform) == initialTransform
            && entities.Get(entity, EngineComponentTypes.DynamicsMotion).LinearVelocity == Vector3.UnitY,
            "a stale managed publication overwrote product state or implied native rollback");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }

    private static void Throws(Action action, string message)
    {
        try
        {
            action();
        }
        catch (InvalidOperationException)
        {
            return;
        }
        throw new InvalidOperationException(message);
    }

    private sealed class AdvancingDynamicsService(Action afterStep) : IDynamicsService
    {
        public int StepAndReadCalls { get; private set; }

        public DynamicsWorld CreateWorld(DynamicsWorldConfig request) => throw new NotSupportedException();
        public DynamicsBody CreateBody(DynamicsCreateBodyRequest request) => throw new NotSupportedException();
        public DynamicsBody CreateSphereBody(DynamicsCreateSphereBodyRequest request) => throw new NotSupportedException();
        public DynamicsBody CreateCuboidBody(DynamicsCreateCuboidBodyRequest request) => throw new NotSupportedException();
        public void ConfigureRopes(DynamicsRopeSolverRequest request) => throw new NotSupportedException();
        public void SetChainLength(DynamicsChainLengthRequest request) => throw new NotSupportedException();
        public void CreateFixedChain(DynamicsFixedChainRequest request) => throw new NotSupportedException();
        public void CreateBodyChain(DynamicsBodyChainRequest request) => throw new NotSupportedException();
        public DynamicsChainReadout ReadChain(DynamicsChainRequest request) => throw new NotSupportedException();
        public DynamicsChainPointReadout ReadChainPoint(DynamicsChainPointRequest request) => throw new NotSupportedException();
        public DynamicsChainReleaseReceipt RemoveChain(DynamicsChainRequest request) => throw new NotSupportedException();
        public void SetFixedTether(DynamicsFixedTetherRequest request) => throw new NotSupportedException();
        public void SetBodyTether(DynamicsBodyTetherRequest request) => throw new NotSupportedException();
        public DynamicsTetherReleaseReceipt RemoveTether(DynamicsTetherRequest request) => throw new NotSupportedException();
        public DynamicsTetherReadout ReadTether(DynamicsTetherRequest request) => throw new NotSupportedException();
        public DynamicsBody CreateSphereBodyWithProperties(DynamicsCreateSphereBodyPropertiesRequest request) => throw new NotSupportedException();
        public DynamicsBody CreateCapsuleBody(DynamicsCreateCapsuleBodyRequest request) => throw new NotSupportedException();
        public void BindWorldCollision(DynamicsWorldCollisionBindingRequest request) => throw new NotSupportedException();
        public DynamicsRebaseWorldOriginReceipt RebaseWorldOrigin(DynamicsRebaseWorldOriginRequest request) => throw new NotSupportedException();
        public DynamicsStepReceipt Step(DynamicsStepRequest request) => throw new NotSupportedException();
        public DynamicsReadout Read(DynamicsReadRequest request) => throw new NotSupportedException();
        public void Reset(DynamicsResetRequest request) => throw new NotSupportedException();
        public void UpdateBody(DynamicsUpdateBodyRequest request) => throw new NotSupportedException();
        public DynamicsWorldReadout ReadWorld(DynamicsWorldReadRequest request) => throw new NotSupportedException();
        public DynamicsBodyAtReceipt ReadBodyAt(DynamicsBodyAtRequest request) => throw new NotSupportedException();
        public DynamicsContactAtReceipt ReadContactAt(DynamicsContactAtRequest request) => throw new NotSupportedException();
        public DynamicsBody ReplaceBody(DynamicsReplaceBodyRequest request) => throw new NotSupportedException();
        public DynamicsBody ReplaceCuboidBody(DynamicsReplaceCuboidBodyRequest request) => throw new NotSupportedException();
        public DynamicsBody ReplaceSphereBody(DynamicsReplaceSphereBodyRequest request) => throw new NotSupportedException();
        public DynamicsBody ReplaceCapsuleBody(DynamicsReplaceCapsuleBodyRequest request) => throw new NotSupportedException();

        public DynamicsStepAndReadLeaseReceipt StepAndRead(DynamicsStepAndReadRequest request)
        {
            StepAndReadCalls++;
            afterStep();
            DynamicsBody body = request.Bodies.Span[0];
            return new DynamicsStepAndReadLeaseReceipt(
                new[]
                {
                    new DynamicsStepAndReadBody(
                        new DynamicsBodyReference(body.Handle.Value),
                        new DynamicsReadout(
                            new Transform(Vector3.UnitX, Quaternion.Identity, Vector3.One),
                            Vector3.UnitX,
                            Vector3.Zero,
                            false,
                            default,
                            0,
                            default)),
                },
                1,
                0,
                0);
        }
    }
}
