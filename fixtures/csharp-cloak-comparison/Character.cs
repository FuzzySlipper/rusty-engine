using System.Numerics;
using Rusty.Engine;

namespace CsharpCloakComparison;

internal enum CloakMode
{
    /// <summary>Cape bones simulated as a ragdoll's bodies and skinned as the GLB weights them.</summary>
    Bones,
    /// <summary>A soft sheet simulated under the cloak, the cloak riding it.</summary>
    Soft,
}

/// <summary>What every character shares: its meshes, the cloak, its material and the collision groups.</summary>
internal sealed record Cast(
    RenderResource BodyOnly,
    RenderResource BodyAndCape,
    CloakAsset Cloak,
    Material CloakMaterial)
{
    /// <summary>The cloak's slot in the cape GLB's materials.</summary>
    public const uint CloakSlot = 1;
    public const uint BodyGroup = 1 << 1, CloakGroup = 1 << 2;
    /// <summary>A wool cloak of about this mass (kg), in both modes.</summary>
    public const float CloakMass = 1.2f;
    public const float LinearDamping = .1f;
    public const float Friction = .3f;
}

/// <summary>
/// One mage on the run with a cloak in one mode. Both modes share the same
/// character collision: body bones of a ragdoll that follow the pose. The
/// bone cloak hangs simulated cape bones from the chest's; the soft cloak
/// drives its collar row to the joints the GLB's collar follows.
/// </summary>
internal sealed class Character : IDisposable
{
    private sealed record BodyBone(string Joint, string End, DynamicsRagdollShape Shape, float Radius, float HalfDepth);

    /// <summary>The shapes the cloak meets: the torso as boxes inside the body where the cloak lies on it, the limbs as capsules.</summary>
    private static readonly BodyBone[] BodyBones =
    [
        new("mixamorig:Hips", "mixamorig:Spine", DynamicsRagdollShape.Box, .14f, .05f),
        new("mixamorig:Spine", "mixamorig:Spine1", DynamicsRagdollShape.Box, .12f, .05f),
        new("mixamorig:Spine1", "mixamorig:Spine2", DynamicsRagdollShape.Box, .12f, .05f),
        new("mixamorig:Spine2", "mixamorig:Neck", DynamicsRagdollShape.Box, .12f, .05f),
        new("mixamorig:LeftUpLeg", "mixamorig:LeftLeg", DynamicsRagdollShape.Capsule, .075f, 0),
        new("mixamorig:LeftLeg", "mixamorig:LeftFoot", DynamicsRagdollShape.Capsule, .055f, 0),
        new("mixamorig:RightUpLeg", "mixamorig:RightLeg", DynamicsRagdollShape.Capsule, .075f, 0),
        new("mixamorig:RightLeg", "mixamorig:RightFoot", DynamicsRagdollShape.Capsule, .055f, 0),
        new("mixamorig:LeftArm", "mixamorig:LeftForeArm", DynamicsRagdollShape.Capsule, .05f, 0),
        new("mixamorig:LeftForeArm", "mixamorig:LeftHand", DynamicsRagdollShape.Capsule, .04f, 0),
        new("mixamorig:RightArm", "mixamorig:RightForeArm", DynamicsRagdollShape.Capsule, .05f, 0),
        new("mixamorig:RightForeArm", "mixamorig:RightHand", DynamicsRagdollShape.Capsule, .04f, 0),
    ];
    /// <summary>The body bone the cape hangs from.</summary>
    private const int Chest = 3;
    private const float BodyBoneMass = 5;
    /// <summary>A cape bone's plate is this half thick (m).</summary>
    private const float PlateHalfDepth = .012f;

    private readonly IEngineContext engine;
    private readonly Cast cast;
    private readonly CloakTuning tuning;
    private readonly DynamicsWorld world;
    public CloakMode Mode { get; }
    public ulong Id { get; }
    public ulong CloakId => Id + 1;
    public Appearance Body { get; }
    public AnimationInstance Instance { get; }
    public Runner Runner;
    /// <summary>The clip it plays and when it started, and the one before and when that started, for the cross-fade.</summary>
    public (string Clip, float Started, string Before, float BeforeStarted) Clips = ("", 0, "", 0);
    private readonly ReadOnlyMemory<AnimationJointInfo> rig;
    public DynamicsRagdoll? Ragdoll { get; private set; }
    public DynamicsSoftBody? Cloak { get; private set; }
    public Appearance? CloakLook { get; private set; }
    /// <summary>The ragdoll's cape bones (after its body bones), by column, top down.</summary>
    public int[][] CapeBones { get; private set; } = [];
    public DynamicsRagdollBone[] Bones { get; private set; } = [];
    public bool Attached => Ragdoll is not null;
    /// <summary>Where the rest-pose cloak was placed when it was attached.</summary>
    private Matrix4x4 attachedAt = Matrix4x4.Identity;

    /// <summary>Where the character stands now.</summary>
    public Transform Placement => new(Runner.Root, Runner.Facing, Vector3.One);

    /// <summary>A rest-pose point of the cloak where it was when attached.</summary>
    public Vector3 Placed(Vector3 rest) => Vector3.Transform(rest, attachedAt);

    public Character(IEngineContext engine, Cast cast, CloakTuning tuning, DynamicsWorld world, CloakMode mode, ulong id, Vector3 start)
    {
        this.engine = engine;
        this.cast = cast;
        this.tuning = tuning;
        this.world = world;
        Mode = mode;
        Id = id;
        RenderResource mesh = mode == CloakMode.Bones ? cast.BodyAndCape : cast.BodyOnly;
        Body = engine.Animation.CreateAnimatedMeshAppearance(new(mesh));
        if (mode == CloakMode.Bones)
        {
            engine.Animation.UpdateAnimatedMeshMaterials(new(Body, new[] { new MeshMaterialBinding(Cast.CloakSlot, cast.CloakMaterial) }));
        }
        rig = engine.Animation.ReadJoints(mesh);
        Instance = engine.Animation.CreateInstance(new(Body, id));
        Runner = new Runner(start);
    }

    public uint Joint(string id) => AnimationJoints.IndexOf(rig.Span, id);

    /// <summary>
    /// Once the instance is posed (at rest, as the cloak was derived): spawn the
    /// body bones and the cloak. The soft cloak's collar captures its offsets
    /// in the rest pose.
    /// </summary>
    public void Attach()
    {
        var bones = BodyBones
            .Select(bone => new DynamicsRagdollBone(Joint(bone.Joint), Joint(bone.End), 0, bone.Shape, bone.Radius, bone.HalfDepth, BodyBoneMass, true))
            .ToList();
        var links = new List<DynamicsRagdollLink>();
        if (Mode == CloakMode.Bones)
        {
            float boneMass = Cast.CloakMass / cast.Cloak.Columns.Sum(column => column.Bones.Length);
            var columns = new List<int[]>();
            foreach (CloakAsset.CapeColumn column in cast.Cloak.Columns)
            {
                var indices = new List<int>();
                for (int row = 0; row < column.Bones.Length; row++)
                {
                    bool last = row == column.Bones.Length - 1;
                    uint end = last ? DynamicsRagdollBone.NoEndJoint : Joint(column.Bones[row + 1]);
                    bones.Add(new DynamicsRagdollBone(Joint(column.Bones[row]), end, last ? column.LastLength : 0,
                        DynamicsRagdollShape.Box, tuning.PlateShare * column.Width, PlateHalfDepth, boneMass, false));
                    int parent = row == 0 ? Chest : indices[^1];
                    links.Add(new((uint)parent, (uint)(bones.Count - 1), Vector3.UnitY,
                        DynamicsJointLimits.Cone(tuning.CapeSwing, -tuning.CapeTwist, tuning.CapeTwist, tuning.CapeTwistDamping)));
                    indices.Add(bones.Count - 1);
                }
                columns.Add([.. indices]);
            }
            CapeBones = [.. columns];
        }
        Bones = [.. bones];
        attachedAt = Matrix4x4.CreateFromQuaternion(Runner.Facing) * Matrix4x4.CreateTranslation(Runner.Root);
        Ragdoll = engine.Dynamics.CreateRagdoll(new(world, Instance, Bones, links.ToArray(),
            Cast.BodyGroup, uint.MaxValue, Cast.Friction, 0, Cast.LinearDamping, tuning.CapeAngularDamping, 1));
        if (Mode == CloakMode.Soft)
        {
            Vector3[] Place(Vector3[] points) => [.. points.Select(Placed)];
            Cloak = engine.Dynamics.CreateSoftBody(DynamicsSoftBodyRequest
                .Surface(world, Place(cast.Cloak.ProxyPositions), ReadOnlyMemory<Vector2>.Empty, cast.Cloak.ProxyTriangles, Cast.CloakMass)
                .WithSkin(Place(cast.Cloak.SkinPositions), cast.Cloak.SkinUvs, cast.Cloak.SkinTriangles) with
            {
                ParticleRadius = tuning.SoftThickness,
                ExtraSubsteps = tuning.SoftExtraSubsteps,
                Material = tuning.SoftMaterial,
                LinearDamping = Cast.LinearDamping,
                CollisionGroups = Cast.CloakGroup,
                Friction = Cast.Friction,
            });
            engine.Dynamics.SetSoftBodyJointAttachments(new(Cloak, Instance,
                cast.Cloak.Collar.Select(collar => DynamicsSoftBodyJointAttachment.Here(collar.Particle, Joint(collar.Joint))).ToArray()));
            CloakLook = engine.Dynamics.CreateSoftBodyAppearance(new(Cloak, cast.CloakMaterial, false));
        }
    }

    /// <summary>Air moving at <paramref name="wind"/> past the cloak, with <paramref name="drag"/> (moving through still air drags it too).</summary>
    public void Blow(Vector3 wind)
    {
        if (Cloak is not null) engine.Dynamics.SetSoftBodyForces(DynamicsSoftBodyForcesRequest.Blowing(Cloak, wind, tuning.Drag));
        if (Ragdoll is not null && Mode == CloakMode.Bones) engine.Dynamics.SetRagdollWind(new(Ragdoll, wind, tuning.Drag));
    }

    /// <summary>Release everything; publish a snapshot without the character first.</summary>
    public void Dispose()
    {
        Ragdoll?.Dispose();
        Cloak?.Dispose();
        CloakLook?.Dispose();
        Instance.Dispose();
        Body.Dispose();
    }
}
