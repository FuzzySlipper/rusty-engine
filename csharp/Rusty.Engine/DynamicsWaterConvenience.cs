using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct DynamicsWaterPlane
{
    /// <summary>Fresh water's density in kilograms per cubic metre.</summary>
    public const float FreshWaterDensity = 1000;

    /// <summary>A drag rate, per second at full immersion, that settles a light raft within a few seconds.</summary>
    public const float SettlingDrag = 3;

    /// <summary>
    /// Still fresh water at <paramref name="surface"/> over the rectangle from <paramref name="min"/> to
    /// <paramref name="max"/> (x and z), with the settling drag. Use <c>with</c> for a current, another
    /// density or other drags.
    /// </summary>
    public static DynamicsWaterPlane Still(Vector2 min, Vector2 max, float surface) =>
        new(min.X, min.Y, max.X, max.Y, surface, 0, 0, FreshWaterDensity, SettlingDrag, SettlingDrag);

    /// <summary>This plane's water flowing at <paramref name="current"/> (x and z).</summary>
    public DynamicsWaterPlane Flowing(Vector2 current) => this with { CurrentX = current.X, CurrentZ = current.Y };
}

public readonly partial record struct DynamicsWaterFieldEntry
{
    /// <summary>A water field as fresh water with the settling drag.</summary>
    public DynamicsWaterFieldEntry(WaterField Field)
        : this(Field, DynamicsWaterPlane.FreshWaterDensity, DynamicsWaterPlane.SettlingDrag, DynamicsWaterPlane.SettlingDrag)
    {
    }
}

public readonly partial record struct DynamicsWaterRequest
{
    /// <summary>Flat planes of water acting on <paramref name="Bodies"/>.</summary>
    public DynamicsWaterRequest(DynamicsWorld World, ReadOnlyMemory<DynamicsWaterPlane> Planes, ReadOnlyMemory<DynamicsBody> Bodies)
        : this(World, Planes, ReadOnlyMemory<DynamicsWaterFieldEntry>.Empty, Bodies)
    {
    }
}
