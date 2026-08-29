using System.Numerics;
using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>Exact managed revision evidence for the one call-local Character projection.</summary>
public readonly record struct CharacterEntityWorldGuard(
    EntityId Entity,
    ulong WorldRevision,
    ComponentRevision TransformRevision,
    ComponentRevision MotionRevision);

/// <summary>
/// One generated Character step and its paired canonical managed publication. The native
/// receipt always identifies its temporary native character as entity 1; <see cref="Entity"/>
/// is therefore preserved explicitly rather than hidden behind a managed mirror.
/// </summary>
public readonly record struct CharacterEntityWorldReceipt(
    EntityId Entity,
    CharacterStepReceipt Native,
    EntityBatchReceipt Managed,
    CharacterEntityWorldGuard Guard);

/// <summary>
/// Projects the canonical managed Transform and CharacterMotion pair through the generated
/// Character controller. The current native service creates its character with a call-local
/// <c>EntityId(1)</c>; optional active obstacles are likewise borrowed for one proposal. That
/// temporary identity is validated on readback and never becomes a managed binding or mirror.
/// </summary>
public sealed class CharacterEntityWorld
{
    private const ulong NativeCharacterEntityId = 1;

    private readonly EntityWorld _entities;
    private readonly ISpatialService _spatial;

    public CharacterEntityWorld(EntityWorld entities, ISpatialService spatial)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _spatial = spatial ?? throw new ArgumentNullException(nameof(spatial));
    }

    /// <summary>
    /// Runs one generated Character proposal and publishes its returned Transform and
    /// CharacterMotion together in one guarded managed batch. The adapter keeps the managed
    /// Transform rotation and scale while applying the returned translation; optional obstacle
    /// values are borrowed by the generated proposal only.
    /// </summary>
    public CharacterEntityWorldReceipt Step(
        EntityId entity,
        SpatialSession session,
        CharacterSupport support,
        CharacterControllerConfig config,
        CharacterControllerCommand command,
        CharacterEntityWorldGuard? expectedGuard = null,
        ReadOnlyMemory<CharacterObstacle> obstacles = default)
    {
        ArgumentNullException.ThrowIfNull(session);

        CharacterEntityWorldGuard guard = CaptureGuard(entity);
        if (expectedGuard is CharacterEntityWorldGuard expected)
        {
            ValidateGuard(expected, guard);
        }

        Transform transform = _entities.Get(entity, EngineComponentTypes.Transform);
        CharacterMotion motion = _entities.Get(entity, EngineComponentTypes.CharacterMotion);

        CharacterStepReceipt native = _spatial.ProposeCharacterStep(new CharacterStepRequest(
            session,
            transform.Translation,
            motion,
            support,
            obstacles,
            config,
            command));

        ValidateGuard(guard, CaptureGuard(entity));
        ValidateNativeReceipt(native, transform, command);
        Transform managedTransform = native.Transform with
        {
            Rotation = transform.Rotation,
            Scale = transform.Scale,
        };

        EntityWorldBatchCandidate managed = _entities.PrepareBatch(
            new EntityBatch()
                .Mutate(world => world.Set(
                    entity,
                    EngineComponentTypes.Transform,
                    managedTransform,
                    guard.TransformRevision))
                .Mutate(world => world.Set(
                    entity,
                    EngineComponentTypes.CharacterMotion,
                    native.Motion,
                    guard.MotionRevision)),
            guard.WorldRevision);
        managed.Publish();
        return new CharacterEntityWorldReceipt(entity, native, managed.Receipt, guard);
    }

    private CharacterEntityWorldGuard CaptureGuard(EntityId entity)
    {
        foreach (EntityComponents<Transform, CharacterMotion> row in _entities.Query(
                     EngineComponentTypes.Transform,
                     EngineComponentTypes.CharacterMotion))
        {
            if (row.Entity == entity)
            {
                return new CharacterEntityWorldGuard(
                    entity,
                    _entities.Revision,
                    _entities.GetComponentRevision(entity, EngineComponentTypes.Transform),
                    _entities.GetComponentRevision(entity, EngineComponentTypes.CharacterMotion));
            }
        }

        throw new InvalidOperationException(
            $"Character entity {entity.Value} must be active with Transform and CharacterMotion components.");
    }

    private static void ValidateGuard(CharacterEntityWorldGuard expected, CharacterEntityWorldGuard observed)
    {
        if (expected != observed)
        {
            throw new InvalidOperationException(
                $"Character managed projection is stale for entity {expected.Entity.Value}.");
        }
    }

    private static void ValidateNativeReceipt(
        CharacterStepReceipt native,
        Transform transform,
        CharacterControllerCommand command)
    {
        Transform callLocalBefore = new(transform.Translation, Quaternion.Identity, Vector3.One);
        if (native.Entity != NativeCharacterEntityId
            || native.CommandSequence != command.Sequence
            || native.TransformBefore != callLocalBefore)
        {
            throw new InvalidOperationException("Character service returned facts outside the guarded managed projection.");
        }
    }
}
