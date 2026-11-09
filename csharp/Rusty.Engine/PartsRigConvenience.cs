using System;

namespace Rusty.Engine;

public readonly partial record struct PartsRigAppearanceRequest
{
    /// <summary>
    /// A world-sized rig that stands upright and turns toward the eye
    /// (<see cref="BillboardMode.Cylindrical"/>): unlit, untinted, solid where its
    /// pieces' alpha passes one half, and drawn smooth.
    /// </summary>
    public PartsRigAppearanceRequest(ReadOnlyMemory<PartsRigView> views, float metresPerUnit)
        : this(views, metresPerUnit, SpriteSizeMode.World, BillboardMode.Cylindrical, 0, SpriteDepthPolicy.Default,
            new Color(1, 1, 1, 1), GraphicsDefaults.SpriteMaterial with { AlphaMode = SpriteAlphaMode.Mask }, 0f)
    {
    }
}

public readonly partial record struct PartsRigPlayRequest
{
    /// <summary>Loops <paramref name="clip"/> at its authored speed, fading in over <paramref name="crossfadeSeconds"/>.</summary>
    public PartsRigPlayRequest(ulong objectId, string clip, float crossfadeSeconds = 0)
        : this(objectId, clip, true, 1f, crossfadeSeconds)
    {
    }
}

public readonly partial record struct PartsRigDriveRequest
{
    /// <summary>Holds <paramref name="bone"/>'s <paramref name="channel"/> at <paramref name="value"/> over what the clips pose.</summary>
    public static PartsRigDriveRequest Hold(ulong objectId, string bone, PartsRigBoneChannel channel, float value) =>
        new(objectId, bone, channel, true, value);

    /// <summary>Lets the clips pose <paramref name="bone"/>'s <paramref name="channel"/> again.</summary>
    public static PartsRigDriveRequest Release(ulong objectId, string bone, PartsRigBoneChannel channel) =>
        new(objectId, bone, channel, false, 0);
}
