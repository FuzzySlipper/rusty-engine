using System.Numerics;
using Rusty.Engine;

namespace CsharpCloakComparison;

/// <summary>What one phase of the run measured on the inspected character.</summary>
internal sealed class PhaseFacts
{
    public int Steps;
    /// <summary>Seconds of the phase measured.</summary>
    public float Seconds;
    public double StepMicroseconds, StepMicrosecondsMost;
    /// <summary>The cloak's top from where the drawn pose holds it (m).</summary>
    public float CollarGapMost;
    /// <summary>The deepest a cloak point went into the body's collision shapes (m).</summary>
    public float PenetrationMost;
    /// <summary>Soft: the largest edge stretch. Bones: the widest gap a link opened (m).</summary>
    public float StretchMost;
    /// <summary>The hem's acceleration (second differences over a call, m/s²) summed squared while the character stands.</summary>
    public double JitterSquares;
    public int JitterSamples;
    public float HemSpeedMost;
    /// <summary>The hem's speed over about the phase's last quarter second (m/s): how far it is from rest when the phase ends.</summary>
    public float HemSpeedEnd;
    public uint ContactsMost;

    public double MeanStepMicroseconds => Steps == 0 ? 0 : StepMicroseconds / Steps;
    /// <summary>RMS of the hem's acceleration while standing (m/s²): shaking with nothing moving it.</summary>
    public double Jitter => JitterSamples == 0 ? 0 : Math.Sqrt(JitterSquares / JitterSamples);
}

/// <summary>
/// Measures a character's cloak against its own pose and collision shapes,
/// the same way in both modes: the soft cloak by its particles, the bone
/// cloak by points on its cape bones' plates.
/// </summary>
internal sealed class Probe
{
    /// <summary>Points across and down each cape bone's plate (fractions of its half width and length).</summary>
    private static readonly (float Across, float Down)[] PlatePoints =
        [(-1, 0), (0, 0), (1, 0), (-1, .5f), (0, .5f), (1, .5f), (-1, 1), (0, 1), (1, 1)];
    /// <summary>The time constant of the hem's end speed (s).</summary>
    private const float EndSeconds = .25f;

    private readonly IEngineContext engine;
    private readonly Character character;
    private readonly CloakAsset cloak;
    private readonly DynamicsRagdollBone[] bones;
    /// <summary>Each ragdoll bone's far end in its joint's frame, from the rest pose.</summary>
    private readonly Vector3[] tips;
    /// <summary>Each collar point (soft particle, or bone column's first joint) at rest in its joint's frame, and that joint.</summary>
    private readonly (uint Joint, Vector3 Local)[] collar;
    private Vector3[] hemBefore = [], hemLast = [];
    /// <summary>The cloak's top after the last call's steps, and the facts it belongs to, until that call's pose is evaluated.</summary>
    private (Vector3[] Top, PhaseFacts Facts)? pendingTop;

    public Probe(IEngineContext engine, Character character, CloakAsset cloak, DynamicsRagdollBone[] bones, ReadOnlySpan<JointPose> rest)
    {
        this.engine = engine;
        this.character = character;
        this.cloak = cloak;
        this.bones = bones;
        JointPose[] at = rest.ToArray();
        tips = [.. bones.Select(bone => bone.EndJoint == DynamicsRagdollBone.NoEndJoint
            ? new Vector3(0, bone.Length, 0)
            : Local(at[bone.Joint].World, at[bone.EndJoint].World.Translation))];
        collar = character.Mode == CloakMode.Soft
            ? [.. cloak.Collar.Select(point =>
            {
                uint joint = character.Joint(point.Joint);
                return (joint, Local(at[joint].World, character.Placed(cloak.ProxyPositions[point.Particle])));
            })]
            : [.. cloak.Columns.Select(column =>
            {
                uint chest = character.Joint("mixamorig:Spine2"), root = character.Joint(column.Bones[0]);
                return (chest, Local(at[chest].World, at[root].World.Translation));
            })];
    }

    private static Vector3 Local(Transform frame, Vector3 point) =>
        Vector3.Transform(point - frame.Translation, Quaternion.Inverse(frame.Rotation));

    private static Vector3 World(Transform frame, Vector3 local) =>
        frame.Translation + Vector3.Transform(local, frame.Rotation);

    /// <summary>After a call's steps (<paramref name="dt"/> seconds): measure into <paramref name="facts"/>; <paramref name="standing"/> when the character is still.</summary>
    public void Measure(PhaseFacts facts, float dt, bool standing)
    {
        facts.Seconds += dt;
        Transform[] placed = engine.Dynamics.ReadRagdoll(character.Ragdoll!).Bones.ToArray();
        Vector3[] points;
        Vector3[] hem;
        Vector3[] top;
        if (character.Cloak is { } soft)
        {
            DynamicsSoftBodyParticle[] particles = engine.Dynamics.ReadSoftBodyParticles(soft).ToArray();
            points = [.. particles.Select(particle => particle.Position)];
            hem = [.. cloak.Hem.Select(index => particles[index].Position)];
            top = [.. cloak.Collar.Select(point => particles[point.Particle].Position)];
            DynamicsSoftBodyReadout readout = engine.Dynamics.ReadSoftBody(soft);
            facts.StretchMost = MathF.Max(facts.StretchMost, readout.MaxStretch);
            facts.ContactsMost = Math.Max(facts.ContactsMost, readout.ContactCount);
        }
        else
        {
            var list = new List<Vector3>();
            foreach (int bone in character.CapeBones.SelectMany(column => column))
            {
                foreach ((float across, float down) in PlatePoints)
                {
                    list.Add(World(placed[bone], new Vector3(across * bones[bone].Radius, 0, 0) + tips[bone] * down));
                }
            }
            points = [.. list];
            hem = [.. character.CapeBones.Select(column => World(placed[column[^1]], tips[column[^1]]))];
            top = [.. character.CapeBones.Select(column => placed[column[0]].Translation)];
            float widest = 0;
            foreach (int[] column in character.CapeBones)
            {
                for (int row = 1; row < column.Length; row++)
                {
                    Vector3 end = World(placed[column[row - 1]], tips[column[row - 1]]);
                    widest = MathF.Max(widest, Vector3.Distance(end, placed[column[row]].Translation));
                }
            }
            facts.StretchMost = MathF.Max(facts.StretchMost, widest);
        }
        pendingTop = (top, facts);
        float deepest = 0;
        for (int bone = 0; bone < bones.Length; bone++)
        {
            if (!bones[bone].Animated) continue;
            foreach (Vector3 point in points)
            {
                deepest = MathF.Max(deepest, -Distance(bone, placed[bone], point));
            }
        }
        facts.PenetrationMost = MathF.Max(facts.PenetrationMost, deepest);
        if (hemLast.Length == hem.Length)
        {
            float speed = hem.Zip(hemLast, (now, last) => Vector3.Distance(now, last)).Average() / dt;
            facts.HemSpeedMost = MathF.Max(facts.HemSpeedMost, speed);
            if (standing && hemBefore.Length == hem.Length)
            {
                for (int index = 0; index < hem.Length; index++)
                {
                    facts.JitterSquares += ((hem[index] - 2 * hemLast[index] + hemBefore[index]) / (dt * dt)).LengthSquared();
                    facts.JitterSamples++;
                }
            }
            float blend = 1 - MathF.Exp(-dt / EndSeconds);
            facts.HemSpeedEnd = facts.Seconds <= dt ? speed : facts.HemSpeedEnd + (speed - facts.HemSpeedEnd) * blend;
        }
        hemBefore = hemLast;
        hemLast = hem;
    }

    /// <summary>
    /// Before the next call's steps: the pose the last call drew is evaluated
    /// now, so measure how far the cloak's top was from where that pose holds it.
    /// </summary>
    public void MeasureCollar()
    {
        if (pendingTop is not (var top, var facts)) return;
        pendingTop = null;
        ReadOnlySpan<JointPose> drawn = engine.Animation.ReadJointPose(character.Instance).Joints.Span;
        for (int index = 0; index < top.Length; index++)
        {
            Vector3 held = World(drawn[(int)collar[index].Joint].World, collar[index].Local);
            facts.CollarGapMost = MathF.Max(facts.CollarGapMost, Vector3.Distance(top[index], held));
        }
    }

    /// <summary>A point's signed distance from a body bone's shape (negative inside).</summary>
    private float Distance(int bone, Transform joint, Vector3 point)
    {
        Vector3 local = Local(joint, point);
        Vector3 tip = tips[bone];
        DynamicsRagdollBone shape = bones[bone];
        if (shape.Shape == DynamicsRagdollShape.Capsule)
        {
            float along = Math.Clamp(Vector3.Dot(local, tip) / tip.LengthSquared(), 0, 1);
            return Vector3.Distance(local, tip * along) - shape.Radius;
        }
        // The box runs along the bone, centred halfway.
        Quaternion along2 = Rotation(Vector3.UnitY, Vector3.Normalize(tip));
        Vector3 inBox = Vector3.Transform(local - tip / 2, Quaternion.Inverse(along2));
        Vector3 half = new(shape.Radius, tip.Length() / 2, shape.HalfDepth);
        Vector3 outside = Vector3.Max(Vector3.Abs(inBox) - half, Vector3.Zero);
        float inside = MathF.Min(MathF.Max(MathF.Abs(inBox.X) - half.X, MathF.Max(MathF.Abs(inBox.Y) - half.Y, MathF.Abs(inBox.Z) - half.Z)), 0);
        return outside.Length() + inside;
    }

    private static Quaternion Rotation(Vector3 from, Vector3 to)
    {
        float dot = Vector3.Dot(from, to);
        if (dot > .99999f) return Quaternion.Identity;
        if (dot < -.99999f) return Quaternion.CreateFromAxisAngle(Vector3.UnitX, MathF.PI);
        Vector3 axis = Vector3.Cross(from, to);
        return Quaternion.Normalize(new Quaternion(axis, 1 + dot));
    }
}
