using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct DynamicsBodyConfig
{
    /// <summary>Creates a body configuration with the standard dynamic-body defaults.</summary>
    public DynamicsBodyConfig(
        Transform Transform,
        Vector3 HalfExtents,
        float Mass,
        DynamicsMassPolicy MassPolicy,
        AxisLocks AxisLocks,
        float GravityScale)
        : this(Transform, HalfExtents, new DynamicsBodyProperties(
            Mass, MassPolicy, Vector3.Zero, Vector3.Zero, AxisLocks,
            0, 0, GravityScale, 0.5f, 0, uint.MaxValue, uint.MaxValue,
            true, false, false, DynamicsSleepThresholds.Default))
    {
    }
}

public readonly partial record struct DynamicsBodyProperties
{
    /// <summary>Creates body properties that sleep by the solver's defaults.</summary>
    public DynamicsBodyProperties(
        float Mass,
        DynamicsMassPolicy MassPolicy,
        Vector3 LinearVelocity,
        Vector3 AngularVelocity,
        AxisLocks AxisLocks,
        float LinearDamping,
        float AngularDamping,
        float GravityScale,
        float Friction,
        float Restitution,
        uint CollisionGroups,
        uint CollisionMask,
        bool Enabled,
        bool Sleeping,
        bool ContinuousCollision)
        : this(Mass, MassPolicy, LinearVelocity, AngularVelocity, AxisLocks, LinearDamping, AngularDamping,
            GravityScale, Friction, Restitution, CollisionGroups, CollisionMask, Enabled, Sleeping,
            ContinuousCollision, DynamicsSleepThresholds.Default)
    {
    }
}

public readonly partial record struct DynamicsSleepThresholds
{
    /// <summary>The solver's defaults: below 0.1 m/s for 0.5 s.</summary>
    public static DynamicsSleepThresholds Default => default;
}
