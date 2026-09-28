using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// One generated Character step and the managed writes that applied it. The native
/// receipt always identifies its temporary native character as entity 1; <see cref="Entity"/>
/// is therefore preserved explicitly rather than hidden behind a managed mirror.
/// </summary>
public readonly record struct EntityCharacterControllerReceipt(
    EntityId Entity,
    CharacterStepReceipt Native,
    EntityBatchReceipt Managed);

/// <summary>
/// Projects the canonical managed Transform and CharacterMotion pair through the generated
/// Character controller. Optional active obstacles are borrowed for one proposal. Retained
/// collision-resident mesh instances are selected by the product's durable instance and
/// entity identities; their current pose is resolved by Spatial for the proposal.
/// </summary>
public sealed class EntityCharacterController
{
    private readonly EntityStore _entities;
    private readonly ISpatialService _spatial;

    public EntityCharacterController(EntityStore entities, ISpatialService spatial)
    {
        _entities = entities ?? throw new ArgumentNullException(nameof(entities));
        _spatial = spatial ?? throw new ArgumentNullException(nameof(spatial));
    }

    /// <summary>
    /// Runs one generated Character proposal and writes its returned Transform and
    /// CharacterMotion. The adapter keeps the managed Transform rotation and scale while
    /// applying the returned translation; optional obstacle values are borrowed by the
    /// generated proposal only.
    /// </summary>
    public EntityCharacterControllerReceipt Step(
        EntityId entity,
        SpatialSession session,
        CharacterSupport support,
        CharacterControllerConfig config,
        CharacterControllerCommand command,
        ReadOnlyMemory<CharacterObstacle> obstacles = default,
        ReadOnlyMemory<CharacterMeshInstance> meshInstances = default)
    {
        ArgumentNullException.ThrowIfNull(session);
        if (_entities.GetLifecycle(entity) != EntityLifecycle.Active)
        {
            throw new InvalidOperationException($"Character entity {entity.Value} must be active.");
        }

        Transform transform = _entities.Get(entity, EngineComponentTypes.Transform);
        CharacterMotion motion = _entities.Get(entity, EngineComponentTypes.CharacterMotion);

        CharacterStepReceipt native = _spatial.ProposeCharacterStep(new CharacterStepRequest(
            session,
            transform.Translation,
            motion,
            support,
            obstacles,
            meshInstances,
            config,
            command));

        EntityBatchReceipt managed = _entities.Commit(new EntityBatch()
            .Set(entity, EngineComponentTypes.Transform, native.Transform with
            {
                Rotation = transform.Rotation,
                Scale = transform.Scale,
            })
            .Set(entity, EngineComponentTypes.CharacterMotion, native.Motion));
        return new EntityCharacterControllerReceipt(entity, native, managed);
    }
}
