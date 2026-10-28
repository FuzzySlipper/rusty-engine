using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct JointOverride
{
    /// <summary>Replaces the joint's local rotation, mixed in by <paramref name="weight"/>.</summary>
    public static JointOverride LocalRotation(uint joint, Quaternion rotation, float weight = 1) =>
        new(joint, PoseSpace.Local, false, true, rotation, false, Vector3.Zero, false, 1, weight);

    /// <summary>Turns the joint by <paramref name="rotation"/> after its evaluated local rotation, e.g. a finger curl over a clip.</summary>
    public static JointOverride AddLocalRotation(uint joint, Quaternion rotation, float weight = 1) =>
        new(joint, PoseSpace.Local, true, true, rotation, false, Vector3.Zero, false, 1, weight);

    /// <summary>Places the joint in <paramref name="space"/> (<see cref="PoseSpace.Model"/> or <see cref="PoseSpace.World"/>); its descendants follow.</summary>
    public static JointOverride Place(uint joint, PoseSpace space, Vector3 translation, Quaternion rotation, float weight = 1) =>
        new(joint, space, false, true, rotation, true, translation, false, 1, weight);

    /// <summary>Turns the joint to <paramref name="rotation"/> in <paramref name="space"/>, where it is.</summary>
    public static JointOverride Orient(uint joint, PoseSpace space, Quaternion rotation, float weight = 1) =>
        new(joint, space, false, true, rotation, false, Vector3.Zero, false, 1, weight);

    /// <summary>
    /// Sets the joint's uniform local scale, mixed in by <paramref name="weight"/>, after the clips, overrides and layers;
    /// its descendants follow. 0 collapses the joint's subtree into it, e.g. a first-person body's head and arms.
    /// </summary>
    public static JointOverride LocalScale(uint joint, float scale, float weight = 1) =>
        new(joint, PoseSpace.Local, false, false, Quaternion.Identity, false, Vector3.Zero, true, scale, weight);
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
        new(instance, clip, time, string.Empty, 0, 0, normalized, placement, applyPoseControls, false);

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
        new(instance, clip, time, blendClip, blendTime, blendWeight, normalized, placement, applyPoseControls, false);

    /// <summary>
    /// <paramref name="instance"/>'s joints as it plays now: its direct
    /// playback (under any fade) or the first controller driving it, at the
    /// time this call advances to, which the renderer draws this call at.
    /// The instance must be in the published scene.
    /// </summary>
    public static AnimationPoseSampleRequest OfCurrentPlayback(
        AnimationInstance instance,
        Transform placement,
        bool applyPoseControls = true) =>
        new(instance, string.Empty, 0, string.Empty, 0, 0, false, placement, applyPoseControls, true);
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

public static class AnimationClips
{
    /// <summary>The index clip layers use for the clip <paramref name="id"/>, from <see cref="IAnimationService.ReadClips"/> (the mesh's own clips, then its clip packs').</summary>
    public static uint IndexOf(ReadOnlySpan<AnimationClipInfo> clips, string id)
    {
        for (var index = 0; index < clips.Length; index++)
        {
            if (clips[index].Id == id)
            {
                return (uint)index;
            }
        }

        throw new ArgumentException($"the mesh plays no clip '{id}'", nameof(id));
    }
}

/// <summary>
/// One update's clip layers for <see cref="IAnimationService.SetLayers"/>,
/// laid in order over what the instance plays. Reuse it: <see cref="Clear"/>
/// keeps its storage.
/// </summary>
public sealed class AnimationLayerSet
{
    private readonly List<AnimationClipLayer> layers = [];
    private readonly List<AnimationJointWeight> jointWeights = [];

    public int Count => layers.Count;

    public void Clear()
    {
        layers.Clear();
        jointWeights.Clear();
    }

    /// <summary>
    /// <paramref name="clip"/> at <paramref name="time"/>, moving the joints it
    /// animates toward it by <paramref name="weight"/>; only the joints in
    /// <paramref name="joints"/>, each by its own weight, when given.
    /// </summary>
    public AnimationLayerSet Override(uint clip, float time, float weight, ReadOnlySpan<AnimationJointWeight> joints = default, bool normalized = true) =>
        Add(clip, time, weight, false, 0, joints, normalized);

    /// <summary>
    /// <paramref name="clip"/>'s change from its pose at
    /// <paramref name="referenceTime"/> to its pose at <paramref name="time"/>,
    /// added over the pose below by <paramref name="weight"/>: a delta clip.
    /// </summary>
    public AnimationLayerSet Additive(uint clip, float time, float weight, float referenceTime = 0, ReadOnlySpan<AnimationJointWeight> joints = default, bool normalized = true) =>
        Add(clip, time, weight, true, referenceTime, joints, normalized);

    public AnimationLayersRequest ToRequest(AnimationInstance instance) =>
        new(instance, layers.ToArray(), jointWeights.ToArray());

    private AnimationLayerSet Add(uint clip, float time, float weight, bool additive, float referenceTime, ReadOnlySpan<AnimationJointWeight> joints, bool normalized)
    {
        layers.Add(new(clip, time, normalized, weight, additive, referenceTime, (uint)jointWeights.Count, (uint)joints.Length));
        foreach (AnimationJointWeight joint in joints)
        {
            jointWeights.Add(joint);
        }

        return this;
    }
}
