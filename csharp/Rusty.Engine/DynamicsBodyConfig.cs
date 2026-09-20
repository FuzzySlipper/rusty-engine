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
            true, false, false))
    {
    }
}
