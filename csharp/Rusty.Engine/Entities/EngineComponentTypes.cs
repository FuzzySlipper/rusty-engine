using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// SDK-owned descriptors for generated Engine values that may participate in a managed entity
/// world. These are value facts only; they do not make this world a mirror of Rust entity-state.
/// They carry no validator: the Engine services that produce and consume these values own
/// their meaning.
/// </summary>
public static class EngineComponentTypes
{
    // Values below 1024 are reserved for Engine-maintained descriptors.
    public static ComponentType<Transform> Transform { get; } = ComponentType<Transform>.CreateEngine(
        EngineComponentKeys.Create(1));

    public static ComponentType<CharacterMotion> CharacterMotion { get; } = ComponentType<CharacterMotion>.CreateEngine(
        EngineComponentKeys.Create(2));

    /// <summary>
    /// A local-space collider shape projected with the canonical <see cref="Transform"/> into
    /// the generated Spatial trigger batch. Entity identity remains owned by <see cref="EntityStore"/>.
    /// </summary>
    public static ComponentType<SpatialCollider> SpatialCollider { get; } = ComponentType<SpatialCollider>.CreateEngine(
        EngineComponentKeys.Create(3));

    /// <summary>
    /// Caller-owned velocity and bounds projected into the call-local generated
    /// Kinematic motion phase. This is distinct from the generated detached
    /// <c>KinematicBody</c> value used by direct Physics integration.
    /// </summary>
    public static ComponentType<Kinematic> Kinematic { get; } = ComponentType<Kinematic>.CreateEngine(
        EngineComponentKeys.Create(4));

    /// <summary>
    /// The minimal copied motion facts returned by the retained Dynamics
    /// owner. Product-side body bindings retain handle ownership; this value
    /// carries no shape, mass, contact, or solver representation.
    /// </summary>
    public static ComponentType<DynamicsMotion> DynamicsMotion { get; } = ComponentType<DynamicsMotion>.CreateEngine(
        EngineComponentKeys.Create(5));
}
