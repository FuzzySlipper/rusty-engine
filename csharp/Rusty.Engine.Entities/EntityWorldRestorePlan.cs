namespace Rusty.Engine.Entities;

/// <summary>
/// Product-owned evidence for one managed entity's durable state.
///
/// This is a transient semantic value, not an Engine serialization record. Products may copy it
/// into whatever archive schema and codec they own.
/// </summary>
public readonly record struct EntityWorldEntityState(
    EntityId Id,
    EntityLifecycle Lifecycle,
    ulong Revision);

/// <summary>Product-owned evidence for one canonical containment edge.</summary>
public readonly record struct EntityWorldContainmentState(EntityId Child, EntityId Container);

/// <summary>
/// Product-owned evidence for one typed component slot. Every admitted entity must have exactly
/// one slot in every declared component family; an absent value still carries its revision.
/// </summary>
public readonly record struct EntityWorldComponentSlot<T>(
    EntityId Entity,
    bool Present,
    T Value,
    ulong Revision)
    where T : struct;

/// <summary>
/// Transient typed input for a managed EntityWorld restore.
///
/// The product decodes and migrates its own durable format, then supplies the resulting semantic
/// evidence here. This type intentionally defines no serialization envelope, schema, codec, or
/// component-name registry.
/// </summary>
public sealed class EntityWorldRestorePlan
{
    private readonly List<EntityWorldEntityState> _entities = [];
    private readonly List<EntityWorldContainmentState> _containment = [];
    private readonly List<ComponentFamilyPlan> _componentFamilies = [];

    public EntityWorldRestorePlan(ulong savedRevision, ulong savedNextEntityValue)
    {
        SavedRevision = savedRevision;
        SavedNextEntityValue = savedNextEntityValue;
    }

    public ulong SavedRevision { get; }

    public ulong SavedNextEntityValue { get; }

    public void AddEntity(EntityWorldEntityState state) => _entities.Add(state);

    public void AddContainment(EntityWorldContainmentState state) => _containment.Add(state);

    public void AddComponentFamily<T>(
        ComponentType<T> componentType,
        IReadOnlyList<EntityWorldComponentSlot<T>> slots)
        where T : struct
    {
        ArgumentNullException.ThrowIfNull(componentType);
        ArgumentNullException.ThrowIfNull(slots);
        _componentFamilies.Add(new ComponentFamilyPlan<T>(componentType, slots.ToArray()));
    }

    internal IReadOnlyList<EntityWorldEntityState> Entities => _entities;

    internal IReadOnlyList<EntityWorldContainmentState> Containment => _containment;

    internal IReadOnlyList<ComponentFamilyPlan> ComponentFamilies => _componentFamilies;

    internal abstract class ComponentFamilyPlan
    {
        internal abstract ComponentType Descriptor { get; }

        internal abstract int SlotCount { get; }

        internal abstract void Validate(
            IReadOnlyDictionary<ComponentTypeKey, EntityWorld.ComponentTable> registrations,
            IReadOnlyDictionary<ulong, EntityWorldEntityState> entities);

        internal abstract void Import(EntityWorld.WorldState state);
    }

    private sealed class ComponentFamilyPlan<T>(
        ComponentType<T> descriptor,
        IReadOnlyList<EntityWorldComponentSlot<T>> slots) : ComponentFamilyPlan
        where T : struct
    {
        private readonly IReadOnlyList<EntityWorldComponentSlot<T>> _slots = slots;

        internal override ComponentType Descriptor => descriptor;

        internal override int SlotCount => _slots.Count;

        internal override void Validate(
            IReadOnlyDictionary<ComponentTypeKey, EntityWorld.ComponentTable> registrations,
            IReadOnlyDictionary<ulong, EntityWorldEntityState> entities)
        {
            if (!registrations.TryGetValue(descriptor.Key, out EntityWorld.ComponentTable? table)
                || !ReferenceEquals(table.Descriptor, descriptor))
            {
                throw new InvalidOperationException(
                    $"Restore component family {descriptor.Key.Value} is not the registered typed descriptor.");
            }

            if (_slots.Count != entities.Count)
            {
                throw new InvalidOperationException(
                    $"Restore component family {descriptor.Key.Value} must provide one slot per entity.");
            }

            HashSet<ulong> seen = [];
            foreach (EntityWorldComponentSlot<T> slot in _slots)
            {
                ValidateRevision(slot.Revision, $"component {descriptor.Key.Value} entity {slot.Entity.Value}");
                if (slot.Entity.Value == 0 || !entities.TryGetValue(slot.Entity.Value, out EntityWorldEntityState entity))
                {
                    throw new InvalidOperationException(
                        $"Restore component family {descriptor.Key.Value} contains an unknown or zero entity.");
                }
                if (!seen.Add(slot.Entity.Value))
                {
                    throw new InvalidOperationException(
                        $"Restore component family {descriptor.Key.Value} contains a duplicate entity slot.");
                }
                if (entity.Lifecycle == EntityLifecycle.Tombstoned && slot.Present)
                {
                    throw new InvalidOperationException(
                        $"Tombstoned entity {slot.Entity.Value} cannot restore component {descriptor.Key.Value}.");
                }
            }

            if (seen.Count != entities.Count)
            {
                throw new InvalidOperationException(
                    $"Restore component family {descriptor.Key.Value} is missing an entity slot.");
            }
        }

        internal override void Import(EntityWorld.WorldState state)
            => state.ImportComponentFamily(descriptor, _slots);
    }

    internal static void ValidateRevision(ulong revision, string subject)
    {
        if (revision == ulong.MaxValue)
        {
            throw new InvalidOperationException($"Restore {subject} revision cannot be rebased without overflow.");
        }
    }
}
