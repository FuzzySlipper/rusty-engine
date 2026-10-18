using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct JointOverride
{
    /// <summary>Replaces the joint's local rotation, mixed in by <paramref name="weight"/>.</summary>
    public static JointOverride LocalRotation(uint joint, Quaternion rotation, float weight = 1) =>
        new(joint, PoseSpace.Local, false, true, rotation, false, Vector3.Zero, weight);

    /// <summary>Turns the joint by <paramref name="rotation"/> after its evaluated local rotation, e.g. a finger curl over a clip.</summary>
    public static JointOverride AddLocalRotation(uint joint, Quaternion rotation, float weight = 1) =>
        new(joint, PoseSpace.Local, true, true, rotation, false, Vector3.Zero, weight);

    /// <summary>Places the joint in <paramref name="space"/> (<see cref="PoseSpace.Model"/> or <see cref="PoseSpace.World"/>); its descendants follow.</summary>
    public static JointOverride Place(uint joint, PoseSpace space, Vector3 translation, Quaternion rotation, float weight = 1) =>
        new(joint, space, false, true, rotation, true, translation, weight);

    /// <summary>Turns the joint to <paramref name="rotation"/> in <paramref name="space"/>, where it is.</summary>
    public static JointOverride Orient(uint joint, PoseSpace space, Quaternion rotation, float weight = 1) =>
        new(joint, space, false, true, rotation, false, Vector3.Zero, weight);
}

public readonly partial record struct AnimationPoseRequest
{
    /// <summary>Reports the instance's evaluated joints for <see cref="IAnimationService.ReadJointPose"/>, without pose controls.</summary>
    public static AnimationPoseRequest ReportOnly(AnimationInstance instance) =>
        new(instance, ReadOnlyMemory<TwoBoneIk>.Empty, ReadOnlyMemory<JointOverride>.Empty, true);
}

public static class AnimationJoints
{
    /// <summary>The index pose controls use for the rig joint named <paramref name="id"/>, from <see cref="IAnimationService.ReadJoints"/>.</summary>
    public static uint IndexOf(ReadOnlySpan<AnimationJointInfo> joints, string id)
    {
        for (var index = 0; index < joints.Length; index++)
        {
            if (joints[index].Id == id)
            {
                return (uint)index;
            }
        }

        throw new ArgumentException($"the rig has no joint '{id}'", nameof(id));
    }
}

public readonly partial record struct AnimationPoseSampleRequest
{
    /// <summary>
    /// <paramref name="instance"/>'s joints with <paramref name="clip"/> at
    /// <paramref name="time"/> (a fraction of the clip when
    /// <paramref name="normalized"/>, otherwise seconds), its pose controls
    /// applied, the instance placed at <paramref name="placement"/>: hit
    /// shapes and held items where the clip puts them at simulation time.
    /// </summary>
    public static AnimationPoseSampleRequest OfClip(
        AnimationInstance instance,
        string clip,
        float time,
        Transform placement,
        bool normalized = true,
        bool applyPoseControls = true) =>
        new(instance, clip, time, string.Empty, 0, 0, normalized, placement, applyPoseControls);

    /// <summary>
    /// <paramref name="clip"/> at <paramref name="time"/> blended toward
    /// <paramref name="blendClip"/> at <paramref name="blendTime"/> by
    /// <paramref name="blendWeight"/>, as a two-clip graph state blends them.
    /// </summary>
    public static AnimationPoseSampleRequest OfBlend(
        AnimationInstance instance,
        string clip,
        float time,
        string blendClip,
        float blendTime,
        float blendWeight,
        Transform placement,
        bool normalized = true,
        bool applyPoseControls = true) =>
        new(instance, clip, time, blendClip, blendTime, blendWeight, normalized, placement, applyPoseControls);
}

public readonly partial record struct AnimationModelPoseSampleRequest
{
    /// <summary>A mesh resource's joints with <paramref name="clip"/> at <paramref name="time"/>, without an instance or controls: an empty clip samples the rest pose.</summary>
    public static AnimationModelPoseSampleRequest OfClip(
        RenderResource resource,
        string clip,
        float time,
        Transform placement,
        bool normalized = true) =>
        new(resource, clip, time, string.Empty, 0, 0, normalized, placement);
}
