using System.Numerics;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// First-person arms apart from any one arms model: where the right shoulder sits in camera space and the point its
/// elbow bends toward (the left's are mirrored), how a hand holds a look by default (<see cref="Hold"/>, degrees, a turn
/// in the hand's canonical frame from the look's, so it suits every rig; see <see cref="HandFrame"/>), and the hands'
/// closure and holding pose. A product may extend it with its own fields (which rig draws the arms, say).
/// </summary>
public record HeldArms(Vector3 Shoulder, Vector3 Pole, Vector3 Hold, ArmDigits Hands)
{
    public Vector3 Shoulder { get; set; } = Shoulder;
    public Vector3 Pole { get; set; } = Pole;
    public Vector3 Hold { get; set; } = Hold;
}

/// <summary>
/// One arms model and how to drive it: the GLB (a content path) and the clip played under the IK; how the model is
/// turned (<see cref="Turn"/>, degrees) and scaled (<see cref="Scale"/>) so it faces -Z in metres (it is placed so its
/// right upper arm joint sits at the arms' shoulder); each arm's joints; how its digits turn; the palm's offset from the
/// hand joint in the hand's canonical frame (right hand; the left mirrors it); and each digit joint's turn added to the
/// clip's hands to bring them to the arms' canonical holding pose (<see cref="Grip"/>, per hand where the clip's hands
/// differ; measure it with <see cref="FirstPersonArms.Knuckles"/> and <see cref="HandKnuckles.Difference"/>). A rig's
/// hand frames and digit axes are measured from the drawn model, so holds, shapes and closure suit every rig.
/// </summary>
public sealed record ArmRig(string Model, string Clip, Vector3 Turn, ArmJoints Main, ArmJoints Off, DigitFrames Digits, Vector3 Palm, HeldShape Grip)
{
    public Vector3 Turn { get; set; } = Turn;
    public Vector3 Palm { get; set; } = Palm;
    /// <summary>The model's uniform scale into metres (1 for a model already in metres).</summary>
    public float Scale { get; set; } = 1;

    /// <summary>What is wrong with this profile, if anything (joints the model lacks are found when it is drawn).</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        if (!Model.EndsWith(".glb", StringComparison.Ordinal)) yield return new("model", "must be a GLB content path.");
        if (Scale <= 0) yield return new("scale", "must be positive.");
        foreach (HeldProblem problem in Grip.Problems(offAllowed: true)) yield return problem.Under("grip");
        foreach (var (name, joints) in new[] { ("main", Main), ("off", Off) })
        {
            if (joints.Fingers.Length != 4 || joints.Fingers.Any(f => f.Length != 3))
                yield return new($"{name}.fingers", "names the index, middle, ring and little fingers' joints, three each from the knuckle out.");
            if (joints.Thumb.Length != 3) yield return new($"{name}.thumb", "names the thumb's three joints from its base out.");
        }
    }

    /// <summary>Takes another profile's tuning in place: its turn, scale, palm and grip.</summary>
    public void Take(ArmRig fresh)
    {
        (Turn, Scale, Palm) = (fresh.Turn, fresh.Scale, fresh.Palm);
        Grip.Take(fresh.Grip);
    }

    /// <summary>Whether another profile draws the same: the same model, clip, joints and digit frames (what tuning in place cannot change).</summary>
    public bool SameDrawing(ArmRig fresh) => Model == fresh.Model && Clip == fresh.Clip && Digits == fresh.Digits
        && Main.Same(fresh.Main) && Off.Same(fresh.Off);
}

/// <summary>
/// One arm's joints by name: the IK chain from the upper arm to the hand (<see cref="Lower"/> descends from
/// <see cref="Upper"/> and the hand from it, with any twist joints between), each digit's three joints from its base
/// out: the index, middle, ring and little fingers, then the thumb, and the forearm's twist joints (<see cref="Twist"/>).
/// </summary>
public sealed record ArmJoints(string Upper, string Lower, string Hand, string[][] Fingers, string[] Thumb)
{
    /// <summary>
    /// Forearm joints that share the hand's roll about the forearm (from the lower arm joint to the hand joint), so a
    /// rolled hold turns the forearm with the hand rather than twisting the whole roll into the wrist: a twist joint
    /// beside the hand (UE's <c>lowerarm_twist_01</c>) or a forearm segment between the lower arm and the hand (a CAT
    /// rig's second forearm bone). Each descends from <see cref="Upper"/> and not from the hand; none by default.
    /// </summary>
    public ArmTwist[] Twist { get; set; } = [];

    /// <summary>Whether another arm names the same joints.</summary>
    public bool Same(ArmJoints other) => Upper == other.Upper && Lower == other.Lower && Hand == other.Hand
        && Fingers.Length == other.Fingers.Length && Fingers.Zip(other.Fingers).All(p => p.First.SequenceEqual(p.Second))
        && Thumb.SequenceEqual(other.Thumb) && Twist.SequenceEqual(other.Twist);
}

/// <summary>A forearm twist joint by name and the share of the hand's roll it turns by (0 none, 1 all of it; 0.5 is typical).</summary>
public sealed record ArmTwist(string Joint, float Share);

/// <summary>
/// What a hand reaches for (camera space): its look's grip and turn, the look's own hold when it has one (degrees, the
/// hand's canonical frame from the look's; else the arms' <see cref="HeldArms.Hold"/>), how closed the hand is (-1 open,
/// 0 its grip, 1 squeezed), the point its elbow bends toward, whether a left hand's hold is the right hand's mirrored
/// (its own mirrored look) or given as is (the off grip of a look held in both hands), and how the look shapes the
/// hand's digits.
/// </summary>
public readonly record struct HandAim(Vector3 Grip, Quaternion Look, Vector3? Hold, float Closure, Vector3 Pole, bool HoldMirrored = true,
    HandShape? Shape = null);
