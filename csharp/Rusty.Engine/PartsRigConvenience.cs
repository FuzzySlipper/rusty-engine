using System;
using System.Numerics;

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
            new Color(1, 1, 1, 1), GraphicsDefaults.SpriteMaterial with { AlphaMode = SpriteAlphaMode.Mask }, 0f,
            false)
    {
    }
}

public readonly partial record struct PartsRigViewportRequest
{
    /// <summary>
    /// Places <paramref name="appearance"/> in the viewport rectangle from <paramref name="minimum"/> (its
    /// lower-left corner) of <paramref name="size"/>, in CSS-viewport units, fitting the bounds of its first
    /// view's setup pose.
    /// </summary>
    public PartsRigViewportRequest(Appearance appearance, Vector2 minimum, Vector2 size, Vector2 alignment,
        SpriteViewportFit fit)
        : this(appearance, true, minimum, size, alignment, fit, Vector2.Zero, Vector2.Zero)
    {
    }

    /// <summary>
    /// Places <paramref name="appearance"/> in the viewport rectangle from <paramref name="minimum"/> of
    /// <paramref name="size"/>, fitting the rig-unit frame from <paramref name="frameMinimum"/> of
    /// <paramref name="frameSize"/>: a bust's fixed square, which breathing and sway move inside.
    /// </summary>
    public PartsRigViewportRequest(Appearance appearance, Vector2 minimum, Vector2 size, Vector2 alignment,
        SpriteViewportFit fit, Vector2 frameMinimum, Vector2 frameSize)
        : this(appearance, true, minimum, size, alignment, fit, frameMinimum, frameSize)
    {
    }

    /// <summary>Returns <paramref name="appearance"/> to the world, placed by its object again.</summary>
    public static PartsRigViewportRequest InWorld(Appearance appearance) =>
        new(appearance, false, Vector2.Zero, Vector2.Zero, Vector2.Zero, SpriteViewportFit.Contain, Vector2.Zero,
            Vector2.Zero);
}

public readonly partial record struct PartsRigPlayRequest
{
    private const uint BaseTrack = 0;
    private const float FullAlpha = 1f, AuthoredSpeed = 1f;

    /// <summary>Loops <paramref name="clip"/> on track 0 at its authored speed, fading in over <paramref name="crossfadeSeconds"/>.</summary>
    public PartsRigPlayRequest(ulong objectId, string clip, float crossfadeSeconds = 0)
        : this(objectId, clip, true, AuthoredSpeed, crossfadeSeconds)
    {
    }

    /// <summary>Plays <paramref name="clip"/> on track 0, fully, at <paramref name="speed"/>.</summary>
    public PartsRigPlayRequest(ulong objectId, string clip, bool looping, float speed, float crossfadeSeconds)
        : this(objectId, clip, looping, speed, crossfadeSeconds, BaseTrack, FullAlpha, false)
    {
    }

    /// <summary>
    /// Plays <paramref name="clip"/> on <paramref name="track"/> over the tracks below it, at
    /// <paramref name="alpha"/>: with <paramref name="additive"/> its keyed values add to theirs (a
    /// flinch, a glow pulse or a breathing layer over a walk), otherwise it replaces them part way. The
    /// object's speed is left as track 0's play set it.
    /// </summary>
    public static PartsRigPlayRequest Layer(ulong objectId, uint track, string clip, float alpha, bool additive,
        float crossfadeSeconds = 0, bool looping = true) =>
        new(objectId, clip, looping, AuthoredSpeed, crossfadeSeconds, track, alpha, additive);
}

public readonly partial record struct PartsRigStopRequest
{
    /// <summary>Fades track 0 back to the setup pose over <paramref name="crossfadeSeconds"/>.</summary>
    public PartsRigStopRequest(ulong objectId, float crossfadeSeconds)
        : this(objectId, crossfadeSeconds, 0)
    {
    }
}

public readonly partial record struct PartsRigTrackRequest
{
    /// <summary>Track 0, the base.</summary>
    public PartsRigTrackRequest(ulong objectId)
        : this(objectId, 0)
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
