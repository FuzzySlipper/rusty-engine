using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct DynamicsSoftness
{
    /// <summary>A spring at <paramref name="frequency"/> Hz with <paramref name="dampingRatio"/> (1 is critical).</summary>
    public static DynamicsSoftness Of(float frequency, float dampingRatio = 1) => new(frequency, dampingRatio);
}

public readonly partial record struct DynamicsSoftBodyMaterial
{
    /// <summary>Rapier's defaults: edges at 30 Hz, bending and shape matching at 10 Hz, volume at 30 Hz, all critically damped; elastic cells of 10 kPa with Poisson's ratio 0.3; no plasticity or tearing.</summary>
    public static DynamicsSoftBodyMaterial Default => new(
        DynamicsSoftness.Of(30), DynamicsSoftness.Of(10), DynamicsSoftness.Of(30), DynamicsSoftness.Of(10),
        1e4f, .3f, 1, 0,
        0, 1, 1,
        0, 1, .5f,
        0, 0, 0, 1, 0, 0);

    /// <summary>Cloth that hangs and folds: stiff edges, soft bending.</summary>
    public static DynamicsSoftBodyMaterial Cloth => Default with { Edge = DynamicsSoftness.Of(60), Bend = DynamicsSoftness.Of(2) };

    /// <summary>An elastic solid of <paramref name="youngModulus"/> Pa (a few kPa wobbles, tens of kPa holds its shape) for corotational or Neo-Hookean cells.</summary>
    public static DynamicsSoftBodyMaterial Elastic(float youngModulus, float poissonRatio = .4f) =>
        Default with { YoungModulus = youngModulus, PoissonRatio = poissonRatio };

    /// <summary>This material tearing where it stretches past <paramref name="strain"/> (a fraction of rest length).</summary>
    public DynamicsSoftBodyMaterial Tearing(float strain) => this with { TearStrain = strain };
}

public readonly partial record struct DynamicsSoftBodyRequest
{
    /// <summary>Contact friction of the builders' bodies.</summary>
    public const float DefaultFriction = .5f;

    /// <summary>Linear damping of the builders' bodies: air enough to settle a swing.</summary>
    public const float DefaultLinearDamping = .1f;

    /// <summary>Rapier's default extra solver iterations per substep.</summary>
    public const uint DefaultExtraIterations = 3;

    /// <summary>
    /// A sheet of <paramref name="columns"/> × <paramref name="rows"/> particles at
    /// <c>origin + column * across + row * down</c>, weighing <paramref name="mass"/> kg. Particle
    /// <see cref="ClothParticle"/>(column, row). Pin particles with <c>with { Pinned = ... }</c>.
    /// </summary>
    public static DynamicsSoftBodyRequest Cloth(DynamicsWorld world, Vector3 origin, Vector3 across, Vector3 down, uint columns, uint rows, float mass) =>
        Base(world, DynamicsSoftBodyShape.Cloth, mass, DynamicsSoftBodyMaterial.Cloth) with
        {
            Origin = origin, Across = across, Down = down, ResolutionX = columns, ResolutionY = rows,
        };

    /// <summary>A solid box about <paramref name="center"/> of <paramref name="halfExtents"/>, on <paramref name="particles"/> particles per axis, as a corotational elastic solid of <paramref name="youngModulus"/> Pa keeping its volume.</summary>
    public static DynamicsSoftBodyRequest Box(DynamicsWorld world, Vector3 center, Vector3 halfExtents, uint particles, float mass, float youngModulus) =>
        Base(world, DynamicsSoftBodyShape.Box, mass, DynamicsSoftBodyMaterial.Elastic(youngModulus)) with
        {
            Origin = center, Across = halfExtents, ResolutionX = particles, ResolutionY = particles, ResolutionZ = particles,
            CellModel = DynamicsSoftBodyCellModel.Corotational, VolumePreservation = true,
        };

    /// <summary>A hollow ball about <paramref name="center"/> of <paramref name="radius"/>, refined <paramref name="subdivisions"/> times (0 to 6), holding its volume: a balloon or a blob.</summary>
    public static DynamicsSoftBodyRequest Shell(DynamicsWorld world, Vector3 center, float radius, uint subdivisions, float mass) =>
        Base(world, DynamicsSoftBodyShape.Shell, mass, DynamicsSoftBodyMaterial.Default) with
        {
            Origin = center, Radius = radius, ResolutionX = subdivisions, VolumePreservation = true,
        };

    /// <summary>A triangle mesh whose vertices are the particles (three <paramref name="triangles"/> indices each): a banner, a cloak. <paramref name="uvs"/> is one per position, or empty.</summary>
    public static DynamicsSoftBodyRequest Surface(DynamicsWorld world, ReadOnlyMemory<Vector3> positions, ReadOnlyMemory<Vector2> uvs, ReadOnlyMemory<uint> triangles, float mass) =>
        Base(world, DynamicsSoftBodyShape.Surface, mass, DynamicsSoftBodyMaterial.Cloth) with
        {
            Positions = positions, Uvs = uvs, Indices = triangles,
        };

    /// <summary>Tetrahedra (four indices each) over <paramref name="positions"/>, as a corotational elastic solid of <paramref name="youngModulus"/> Pa.</summary>
    public static DynamicsSoftBodyRequest Solid(DynamicsWorld world, ReadOnlyMemory<Vector3> positions, ReadOnlyMemory<Vector2> uvs, ReadOnlyMemory<uint> tetrahedra, float mass, float youngModulus) =>
        Base(world, DynamicsSoftBodyShape.Solid, mass, DynamicsSoftBodyMaterial.Elastic(youngModulus)) with
        {
            Positions = positions, Uvs = uvs, Indices = tetrahedra, CellModel = DynamicsSoftBodyCellModel.Corotational,
        };

    /// <summary>A closed, outward-wound mesh filled with tetrahedra of about <paramref name="cellSize"/>, drawn as the mesh, as a corotational elastic solid of <paramref name="youngModulus"/> Pa: a creature's body.</summary>
    public static DynamicsSoftBodyRequest Filled(DynamicsWorld world, ReadOnlyMemory<Vector3> positions, ReadOnlyMemory<Vector2> uvs, ReadOnlyMemory<uint> triangles, float cellSize, float mass, float youngModulus) =>
        Base(world, DynamicsSoftBodyShape.Filled, mass, DynamicsSoftBodyMaterial.Elastic(youngModulus)) with
        {
            Positions = positions, Uvs = uvs, Indices = triangles, CellSize = cellSize,
            CellModel = DynamicsSoftBodyCellModel.Corotational,
        };

    /// <summary>This surface drawing <paramref name="positions"/> (with <paramref name="uvs"/>, one per position or none, and three <paramref name="triangles"/> indices each) instead of its own triangles: a thick or detailed garment riding a coarse simulated sheet.</summary>
    public DynamicsSoftBodyRequest WithSkin(ReadOnlyMemory<Vector3> positions, ReadOnlyMemory<Vector2> uvs, ReadOnlyMemory<uint> triangles) =>
        this with { SkinPositions = positions, SkinUvs = uvs, SkinIndices = triangles };

    /// <summary>The index of a cloth's particle at <paramref name="column"/> and <paramref name="row"/>.</summary>
    public uint ClothParticle(uint column, uint row) => column * ResolutionY + row;

    private static DynamicsSoftBodyRequest Base(DynamicsWorld world, DynamicsSoftBodyShape shape, float mass, DynamicsSoftBodyMaterial material) => new(
        world, shape, Vector3.Zero, Vector3.Zero, Vector3.Zero, 0, 0, 0, 0, 0,
        ReadOnlyMemory<Vector3>.Empty, ReadOnlyMemory<Vector2>.Empty, ReadOnlyMemory<uint>.Empty, ReadOnlyMemory<uint>.Empty,
        ReadOnlyMemory<Vector3>.Empty, ReadOnlyMemory<Vector2>.Empty, ReadOnlyMemory<uint>.Empty,
        mass, 0, DefaultLinearDamping, 1,
        DynamicsSoftBodyCellModel.Volume, DynamicsSoftBodySolver.Constraints, material,
        false, 1, false, false, false,
        uint.MaxValue, uint.MaxValue, DefaultFriction, 0,
        0, DefaultExtraIterations, true);
}

public readonly partial record struct DynamicsSoftBodySettingsRequest
{
    /// <summary>Rapier's defaults: one substep per step, up to four more for a body hit fast, soft contacts four times as stiff as rigid ones, constraints past 0.75 strain solved again.</summary>
    public DynamicsSoftBodySettingsRequest(DynamicsWorld World)
        : this(World, 1, 4, 4, .75f)
    {
    }
}

public readonly partial record struct DynamicsSoftBodyForcesRequest
{
    /// <summary>A flag's drag: half the air's density times a flat plate's drag coefficient.</summary>
    public const float FlagDrag = .6f;

    /// <summary>Air moving at <paramref name="wind"/> m/s past <paramref name="softBody"/>'s surface, with <paramref name="drag"/>.</summary>
    public static DynamicsSoftBodyForcesRequest Blowing(DynamicsSoftBody softBody, Vector3 wind, float drag = FlagDrag) =>
        new(softBody, Vector3.Zero, wind, drag);

    /// <summary>No forces but gravity.</summary>
    public static DynamicsSoftBodyForcesRequest None(DynamicsSoftBody softBody) => new(softBody, Vector3.Zero, Vector3.Zero, 0);
}

public readonly partial record struct DynamicsSoftBodyJointAttachmentsRequest
{
    /// <summary>Each particle held where it is now in its joint's frame (the offsets are captured; <paramref name="Instance"/> must have a pose).</summary>
    public DynamicsSoftBodyJointAttachmentsRequest(DynamicsSoftBody SoftBody, AnimationInstance Instance, ReadOnlyMemory<DynamicsSoftBodyJointAttachment> Attachments)
        : this(SoftBody, Instance, Attachments, true)
    {
    }
}

public readonly partial record struct DynamicsSoftBodyJointAttachment
{
    /// <summary><paramref name="particle"/> held to <paramref name="joint"/> where it is when attached.</summary>
    public static DynamicsSoftBodyJointAttachment Here(uint particle, uint joint) => new(particle, joint, Vector3.Zero);
}
