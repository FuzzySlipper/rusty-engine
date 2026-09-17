using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

internal static class StatsComponentExercise
{
    public static void Run()
    {
        var maximumId = StatId.Parse("health");
        var healthId = TrackId.Parse("health");
        var maximum = new Stat(100, minimum: 0);
        var health = new Track(maximum, current: 70);
        var stats = new StatsComponent();
        var view = stats.Stats;
        stats.AddStat(maximumId, maximum);
        stats.AddTrack(healthId, health);
        using var entities = new EntityStore();
        var entity = entities.Create();
        entities.Add(entity, stats);
        var attached = entities.Get<StatsComponent>(entity);
        Check(ReferenceEquals(attached, stats) && ReferenceEquals(attached.GetStat(maximumId), health.Maximum), "attachment retains the supplied graph");
        attached.GetStat(maximumId).BaseValue = 60;
        Check(health.Current == 60 && view.Single().Value.Value == 60, "ordinary reads and enumeration see live changes");
        attached.GetTrack(healthId).Spend(10);
        Check(stats.Tracks.Single().Value.Current == 50, "track enumeration reads canonical values");
        try { stats.AddStat(StatId.Parse("health"), new Stat(5)); throw new InvalidOperationException("duplicate stat admitted"); }
        catch (ArgumentException) { }
        try { stats.AddTrack(TrackId.Parse("health"), new Track(5)); throw new InvalidOperationException("duplicate track admitted"); }
        catch (ArgumentException) { }
        Check(ReferenceEquals(stats.GetStat(maximumId), maximum) && ReferenceEquals(stats.GetTrack(healthId), health), "duplicate rejection preserves original entries");
        stats.RemoveStat(maximumId);
        maximum.BaseValue = 40;
        Check(!stats.TryGetStat(maximumId, out _) && health.Current == 40, "removing membership does not sever maximum reference");
        stats.RemoveTrack(healthId);
        Check(!stats.TryGetTrack(healthId, out _), "track removal");
        Console.WriteLine("passed: live standalone and attached stats component");
    }

    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
