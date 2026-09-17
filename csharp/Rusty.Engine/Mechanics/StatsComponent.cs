using System.Collections.ObjectModel;
using System.Diagnostics.CodeAnalysis;

namespace Rusty.Engine.Mechanics;

/// <summary>Product-named stats and tracks, usable standalone or attached to an entity.</summary>
/// <remarks>Entries retain the supplied objects. Collection membership is explicit; values remain live.
/// Adding a track does not implicitly register its maximum under a stat ID.</remarks>
public sealed class StatsComponent
{
    private readonly Dictionary<StatId, Stat> _stats = [];
    private readonly Dictionary<TrackId, Track> _tracks = [];

    public StatsComponent()
    {
        Stats = new ReadOnlyDictionary<StatId, Stat>(_stats);
        Tracks = new ReadOnlyDictionary<TrackId, Track>(_tracks);
    }

    public IReadOnlyDictionary<StatId, Stat> Stats { get; }
    public IReadOnlyDictionary<TrackId, Track> Tracks { get; }

    /// <summary>Adds the supplied stat. Duplicate IDs are rejected rather than replaced.</summary>
    public void AddStat(StatId id, Stat stat)
    {
        ArgumentNullException.ThrowIfNull(id);
        ArgumentNullException.ThrowIfNull(stat);
        _stats.Add(id, stat);
    }

    /// <summary>Adds the supplied track. Duplicate IDs are rejected rather than replaced.</summary>
    public void AddTrack(TrackId id, Track track)
    {
        ArgumentNullException.ThrowIfNull(id);
        ArgumentNullException.ThrowIfNull(track);
        _tracks.Add(id, track);
    }

    public Stat GetStat(StatId id) => _stats[id];
    public Track GetTrack(TrackId id) => _tracks[id];
    public bool TryGetStat(StatId id, [NotNullWhen(true)] out Stat? stat) => _stats.TryGetValue(id, out stat);
    public bool TryGetTrack(TrackId id, [NotNullWhen(true)] out Track? track) => _tracks.TryGetValue(id, out track);

    // Removal changes membership only; it does not detach a track from its maximum stat.
    public bool RemoveStat(StatId id) => _stats.Remove(id);
    public bool RemoveTrack(TrackId id) => _tracks.Remove(id);
}
