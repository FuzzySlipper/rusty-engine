namespace Rusty.Engine.Mechanics;

/// <summary>One attached stat modifier as plain values. The removal handle is live state and is not captured.</summary>
public readonly record struct StatModifier(double Amount, StatModifierKind Kind);

/// <summary>Selected values for one stat: plain data with no live references and no authored sources.</summary>
public sealed record StatCapture(
    string Id,
    double BaseValue,
    double Minimum,
    double Maximum,
    double Quantum,
    MidpointRounding Rounding,
    MidpointRounding IntegerRounding,
    IReadOnlyList<StatModifier> Modifiers);

/// <summary>
/// Selected values for one track. MaximumId names the captured stat (by its string id) whose
/// rebuilt instance becomes this track's live maximum: shared identity is preserved by rebuild,
/// not by deserializing a second maximum.
/// </summary>
public sealed record TrackCapture(
    string Id,
    string MaximumId,
    double Current,
    double Minimum,
    TrackMaximumChangePolicy MaximumChangePolicy,
    double Quantum,
    MidpointRounding Rounding,
    MidpointRounding IntegerRounding);

/// <summary>Selected values for one StatsComponent: plain data with no live references.</summary>
public sealed record StatsComponentSnapshot(
    IReadOnlyList<StatCapture> Stats,
    IReadOnlyList<TrackCapture> Tracks);

/// <summary>
/// Captures a StatsComponent's selected values and rebuilds an equivalent component set.
/// </summary>
/// <remarks>
/// <para>
/// Rebuild constructs each stat once, re-adds its modifiers, then constructs each track against
/// the rebuilt maximum instance — so a restored track's later maximum changes follow the
/// restored stat, exactly like the live original. Every track maximum must be a stat in the
/// same snapshot; an external maximum is an InvalidOperationException, not a silent duplicate.
/// </para>
/// <para>
/// Explicit product choices, not captured here: authored stat sources and provenance
/// (re-supply via SetSources), effect definitions and instance identities (re-apply via
/// EffectsComponent.Apply with fresh ids), item and slot definitions (re-supply to the
/// inventory store), and durable-to-runtime identity mapping (product-owned). Existing
/// constructors keep their validation, so hand-edited snapshots fail loudly at rebuild.
/// </para>
/// </remarks>
public static class StatsComponentCapture
{
    /// <summary>
    /// Reads the component's current selected values. The component is not modified and the
    /// snapshot shares no live state with it.
    /// </summary>
    public static StatsComponentSnapshot Capture(StatsComponent component)
    {
        ArgumentNullException.ThrowIfNull(component);
        var stats = new List<StatCapture>(component.Stats.Count);
        foreach ((StatId id, Stat stat) in component.Stats.OrderBy(entry => entry.Key.Value, StringComparer.Ordinal))
        {
            stats.Add(new StatCapture(
                id.Value,
                stat.BaseValue,
                stat.Minimum,
                stat.Maximum,
                stat.Quantum,
                stat.Rounding,
                stat.IntegerRounding,
                stat.Modifiers));
        }

        var tracks = new List<TrackCapture>(component.Tracks.Count);
        foreach ((TrackId id, Track track) in component.Tracks.OrderBy(entry => entry.Key.Value, StringComparer.Ordinal))
        {
            StatId? maximumId = component.Stats.FirstOrDefault(entry => ReferenceEquals(entry.Value, track.Maximum)).Key;
            if (maximumId is null)
            {
                throw new InvalidOperationException(
                    $"Track '{id.Value}' has a maximum stat outside this component; capture requires shared component ownership.");
            }
            tracks.Add(new TrackCapture(
                id.Value,
                maximumId.Value,
                track.Value,
                track.Minimum,
                track.MaximumChangePolicy,
                track.Quantum,
                track.Rounding,
                track.IntegerRounding));
        }

        return new StatsComponentSnapshot(stats, tracks);
    }

    /// <summary>
    /// Rebuilds an equivalent component set from captured values. Stats are constructed first;
    /// tracks share the rebuilt maximum instances. Throws on unknown maximum ids, duplicate
    /// ids, or values the existing constructors reject.
    /// </summary>
    public static StatsComponent Rebuild(StatsComponentSnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        var rebuilt = new StatsComponent();
        var stats = new Dictionary<string, Stat>(snapshot.Stats.Count, StringComparer.Ordinal);
        foreach (StatCapture captured in snapshot.Stats)
        {
            var stat = new Stat(
                captured.BaseValue,
                minimum: captured.Minimum,
                maximum: captured.Maximum,
                quantum: captured.Quantum,
                rounding: captured.Rounding,
                integerRounding: captured.IntegerRounding);
            foreach (StatModifier modifier in captured.Modifiers)
            {
                stat.AddModifier(modifier.Amount, modifier.Kind);
            }
            rebuilt.AddStat(StatId.Parse(captured.Id), stat);
            stats.Add(captured.Id, stat);
        }

        foreach (TrackCapture captured in snapshot.Tracks)
        {
            if (!stats.TryGetValue(captured.MaximumId, out Stat? maximum))
            {
                throw new InvalidOperationException(
                    $"Track '{captured.Id}' names an unknown maximum stat '{captured.MaximumId}'.");
            }
            rebuilt.AddTrack(
                TrackId.Parse(captured.Id),
                new Track(
                    maximum,
                    current: captured.Current,
                    minimum: captured.Minimum,
                    maximumChangePolicy: captured.MaximumChangePolicy,
                    quantum: captured.Quantum,
                    rounding: captured.Rounding,
                    integerRounding: captured.IntegerRounding));
        }

        return rebuilt;
    }
}
