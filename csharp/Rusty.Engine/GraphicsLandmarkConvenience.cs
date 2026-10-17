using System;
using System.Numerics;

namespace Rusty.Engine;

/// <summary>
/// A point the renderer finds as drawn, with an axis and a frame: a point in an overlay place,
/// or one derived from rig joints (<see cref="HandRig"/> names a hand's). Joints are indices in
/// <see cref="IAnimationService.ReadJoints"/> of the entity's animated mesh.
/// </summary>
public readonly partial record struct Landmark
{
    /// <summary>
    /// <paramref name="point"/> in <paramref name="place"/>. <paramref name="axis"/> (none when
    /// zero) and the frame a pair's offset is read along are the place's frame turned by
    /// <paramref name="frame"/>.
    /// </summary>
    public static Landmark At(OverlayPlace place, Vector3 point, Vector3 axis = default, Quaternion? frame = null) =>
        new(LandmarkKind.At, place.Space, place.Entity, place.Joint, 0, 0, 0, point, frame ?? Quaternion.Identity, axis, 0, 0, false);

    /// <summary><paramref name="amount"/> of the way from joint <paramref name="from"/> to joint <paramref name="to"/>, its axis along them: the web between thumb and index.</summary>
    public static Landmark Between(ulong entity, uint from, uint to, float amount) =>
        new(LandmarkKind.Between, OverlaySpace.Joint, entity, from, to, 0, 0, default, Quaternion.Identity, default, amount, 0, false);

    /// <summary>
    /// Past joint <paramref name="last"/> along its own bone, <paramref name="amount"/> of the
    /// length from joint <paramref name="previous"/> to it, its axis along that bone: a fingertip's pad.
    /// </summary>
    public static Landmark Beyond(ulong entity, uint previous, uint last, float amount) =>
        new(LandmarkKind.Beyond, OverlaySpace.Joint, entity, previous, last, 0, 0, default, Quaternion.Identity, default, amount, 0, false);

    /// <summary>
    /// A palm's centre, <paramref name="along"/> of the way from the <paramref name="wrist"/> to the
    /// <paramref name="middle"/> finger's base and <paramref name="depth"/> metres out of the palm,
    /// with the palm's normal (across the <paramref name="index"/> and <paramref name="pinky"/>
    /// bases) as its axis.
    /// </summary>
    public static Landmark Palm(ulong entity, uint wrist, uint index, uint middle, uint pinky, float along, float depth, HandSide side) =>
        new(LandmarkKind.Palm, OverlaySpace.Joint, entity, wrist, index, middle, pinky, default, Quaternion.Identity, default, along, depth, side == HandSide.Left);
}

/// <summary>
/// Two landmarks the renderer measures as drawn, published in an overlay set
/// (<see cref="OverlaySetRequest"/>): green within tolerance and red outside while inspection
/// shows the set, and answered by <c>engine.inspect.landmarks</c>.
/// </summary>
public readonly partial record struct LandmarkPair
{
    /// <summary>How far apart the landmarks are, within <paramref name="tolerance"/> metres.</summary>
    public static LandmarkPair Distance(string name, Landmark from, Landmark to, float tolerance) =>
        new(name, from, to, tolerance, LandmarkAim.None, 0);

    /// <summary>
    /// How far <paramref name="from"/>'s axis turns from what it <paramref name="aim"/>s at, within
    /// <paramref name="angleTolerance"/> degrees, and its distance within
    /// <paramref name="tolerance"/> metres when that is above 0.
    /// </summary>
    public static LandmarkPair Facing(string name, Landmark from, Landmark to, LandmarkAim aim, float angleTolerance, float tolerance = 0) =>
        new(name, from, to, tolerance, aim, angleTolerance);
}

public enum HandSide
{
    Right,
    Left,
}

public enum Digit
{
    Thumb,
    Index,
    Middle,
    Ring,
    Pinky,
}

/// <summary>
/// A hand's rig joints, named once, and the standard hand landmarks found from them as drawn: the
/// palm's centre and normal, the web between thumb and index, a digit's pad and its joints. Each
/// chain runs from the digit's base, fixed to the hand, to its last joint.
/// </summary>
public sealed record HandRig(ulong Entity, HandSide Side, uint Wrist, uint[] Thumb, uint[] Index, uint[] Middle, uint[] Ring, uint[] Pinky)
{
    /// <summary>The palm's centre: this much of the way from the wrist to the middle finger's base, and this far (metres) out of the palm.</summary>
    public const float PalmAlong = 0.55f, PalmDepth = 0.012f;
    /// <summary>The web: this much of the way from the thumb's base to the index finger's.</summary>
    public const float WebAmount = 0.5f;
    /// <summary>A pad: past the last joint by this share of the bone before it.</summary>
    public const float PadReach = 0.8f;

    /// <summary>The hand of <paramref name="entity"/>'s rig with joints named as <see cref="IAnimationService.ReadJoints"/> names them.</summary>
    public static HandRig FromJoints(
        ReadOnlySpan<AnimationJointInfo> joints,
        ulong entity,
        HandSide side,
        string wrist,
        string[] thumb,
        string[] index,
        string[] middle,
        string[] ring,
        string[] pinky)
    {
        static uint[] Chain(ReadOnlySpan<AnimationJointInfo> rig, string[] ids)
        {
            var chain = new uint[ids.Length];
            for (var at = 0; at < ids.Length; at++)
            {
                chain[at] = AnimationJoints.IndexOf(rig, ids[at]);
            }
            return chain;
        }
        return new(entity, side, AnimationJoints.IndexOf(joints, wrist), Chain(joints, thumb), Chain(joints, index), Chain(joints, middle), Chain(joints, ring), Chain(joints, pinky));
    }

    public uint[] Chain(Digit digit) => digit switch
    {
        Digit.Thumb => Thumb,
        Digit.Index => Index,
        Digit.Middle => Middle,
        Digit.Ring => Ring,
        _ => Pinky,
    };

    /// <summary>The palm's centre, with its normal (out of the palm) as the axis.</summary>
    public Landmark Palm(float along = PalmAlong, float depth = PalmDepth) =>
        Landmark.Palm(Entity, Wrist, Index[0], Middle[0], Pinky[0], along, depth, Side);

    /// <summary>The web between the thumb's base and the index finger's.</summary>
    public Landmark Web(float amount = WebAmount) => Landmark.Between(Entity, Thumb[0], Index[0], amount);

    /// <summary>A digit's pad, past its last joint.</summary>
    public Landmark Pad(Digit digit, float reach = PadReach)
    {
        uint[] chain = Chain(digit);
        return Landmark.Beyond(Entity, chain[^2], chain[^1], reach);
    }

    /// <summary>A digit's joint (0 its base), or a point in that joint's frame.</summary>
    public Landmark Joint(Digit digit, int joint, Vector3 offset = default) =>
        Landmark.At(OverlayPlace.AtJoint(Entity, Chain(digit)[joint]), offset);

    /// <summary>The wrist, or a point in its frame.</summary>
    public Landmark AtWrist(Vector3 offset = default) => Landmark.At(OverlayPlace.AtJoint(Entity, Wrist), offset);
}
