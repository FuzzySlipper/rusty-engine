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
