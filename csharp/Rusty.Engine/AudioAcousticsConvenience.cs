using System.Numerics;

namespace Rusty.Engine;

/// <summary>
/// Sound through the world's geometry. Once a product names the spatial
/// session the listener is in, solid between a spatial source and the
/// listener muffles it, open passages carry it round, and the listener's
/// space adds reverb by how enclosed it is. <see cref="QueryArrival"/>
/// answers how a sound arrives at other points by the same model.
/// </summary>
public static class AudioAcousticsConvenience
{
    /// <summary>The most points one <see cref="QueryArrival"/> answers.</summary>
    public const int MaxArrivalPoints = 64;

    /// <summary>
    /// Plays every spatial source through <paramref name="session"/>'s
    /// collision until another space is named or it is cleared;
    /// <paramref name="reverbLevel"/> (0 to 1) scales the reverb the
    /// listener's space adds.
    /// </summary>
    public static void SetAcousticSpace(this IAudioService audio, SpatialSession session, float reverbLevel = 1f)
    {
        ArgumentNullException.ThrowIfNull(audio);
        audio.SetAcousticSpace(new AudioAcousticSpaceRequest(session, reverbLevel));
    }

    /// <summary>Plays every source as if nothing stood in the way, with no reverb.</summary>
    public static void ClearAcousticSpace(this IAudioService audio)
    {
        ArgumentNullException.ThrowIfNull(audio);
        audio.ClearAcousticSpace(default(AudioClearAcousticSpaceRequest));
    }

    /// <summary>
    /// How a sound made at <paramref name="source"/>, audible to
    /// <paramref name="range"/> with <paramref name="rolloff"/>, arrives at each
    /// of <paramref name="points"/> through <paramref name="session"/>'s
    /// collision: the level playback would play it at, its cutoff and the way
    /// it came. At most <see cref="MaxArrivalPoints"/> points.
    /// </summary>
    public static AudioArrivalResult QueryArrival(this IAudioService audio, SpatialSession session, Vector3 source, float range,
        ReadOnlyMemory<Vector3> points, AudioRolloff rolloff = AudioRolloff.LinearDecibels)
    {
        ArgumentNullException.ThrowIfNull(audio);
        return audio.QueryArrival(new AudioArrivalRequest(session, source, range, rolloff, points));
    }
}

public readonly partial record struct AudioSourceDescriptor
{
    /// <summary>A source the world's geometry acts on once an acoustic space is named.</summary>
    public AudioSourceDescriptor(
        AudioClip clip,
        AudioBus bus,
        float volume,
        float pitch,
        bool looping,
        float spatialBlend,
        float maxDistance,
        AudioRolloff rolloff,
        float pan,
        AudioEmitterKind emitterKind,
        Vector3 position,
        ulong entity,
        Vector3 offset)
        : this(clip, bus, volume, pitch, looping, spatialBlend, maxDistance, rolloff, pan, emitterKind, position, entity, offset,
            AudioAcoustics.World)
    {
    }
}

public readonly partial record struct AudioArrival
{
    /// <summary>The sound reaches the point at all.</summary>
    public bool Audible => Gain > 0f;

    /// <summary>The level it arrives at in decibels; negative infinity when silent.</summary>
    public float Decibels => Gain > 0f ? 20f * MathF.Log10(Gain) : float.NegativeInfinity;
}
