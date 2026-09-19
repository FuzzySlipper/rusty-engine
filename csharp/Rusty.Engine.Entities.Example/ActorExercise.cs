using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

/// <summary>
/// Engine-owned proof for the optional Actor facade: wrap one entity, compose a game actor with
/// named properties over the same live instances, and exercise the ordinary failure semantics.
/// Downstream Kit/ruleset actors follow the GameActor pattern here — a small wrapper holding an
/// Actor — rather than inheriting from Actor or registering with it.
/// </summary>
internal static class ActorExercise
{
    public static void Run()
    {
        using var store = new EntityStore();
        EntityId entity = store.Create(new EntityTypeId("code:spawn/goblin-scout"));
        var stats = new StatsComponent();
        StatId mightId = StatId.Parse("might");
        TrackId healthId = TrackId.Parse("health");
        stats.AddStat(mightId, new Stat(10, minimum: 0, maximum: 100));
        stats.AddTrack(healthId, new Track(100, current: 80));
        store.Add(entity, stats);
        store.Add(entity, new Callsign { Name = "Scout" });

        var actor = new Actor(store, entity);
        var game = new GameActor(actor);
        Require(ReferenceEquals(game.Stats, stats) && ReferenceEquals(game.Stats, store.Get<StatsComponent>(entity)),
            "named actor properties did not return the attached instances");
        Require(game.Entity == entity && game.TypeId == new EntityTypeId("code:spawn/goblin-scout"),
            "composed actor did not expose entity identity and metadata");
        Require(actor.IsAlive && actor.Lifecycle == EntityLifecycle.Active
            && actor.Has<StatsComponent>() && actor.Has<Callsign>()
            && actor.TryGet(out StatsComponent? liveStats) && ReferenceEquals(liveStats, stats)
            && actor.TryGet(out Callsign? liveSign) && ReferenceEquals(liveSign, store.Get<Callsign>(entity)),
            "attached membership was not visible through the facade");

        // Mutation in either direction is visible through both facades: one authoritative store.
        game.Sign.Name = "Vanguard";
        Require(store.Get<Callsign>(entity).Name == "Vanguard", "actor-side mutation was not visible to the store");
        store.Get<Callsign>(entity).Name = "Rearguard";
        Require(game.Sign.Name == "Rearguard", "store-side mutation was not visible to the actor");
        double spent = game.Stats.GetTrack(healthId).Spend(15);
        Require(spent == 15 && store.Get<StatsComponent>(entity).GetTrack(healthId).Value == 65,
            "mechanics mutation through the actor did not apply to the attached component");
        var statsReplacement = new StatsComponent();
        actor.Replace(statsReplacement);
        Require(ReferenceEquals(game.Stats, statsReplacement)
            && ReferenceEquals(store.Get<StatsComponent>(entity), statsReplacement),
            "stats replacement was not visible through both facades");

        // Wrapping is not construction: a bare entity wraps cleanly and gains nothing.
        EntityId bare = store.Create();
        var bareActor = new Actor(store, bare);
        Require(!bareActor.Has<StatsComponent>() && !bareActor.TryGet(out StatsComponent? _),
            "wrapping created or implied a component");
        ThrowsInvalidOperation(() => bareActor.Get<StatsComponent>(), "missing component did not throw");

        // Replacement is observed live; removal detaches without invalidating held references.
        var replacement = new Callsign { Name = "Replacement" };
        actor.Replace(replacement);
        Require(ReferenceEquals(game.Sign, replacement) && ReferenceEquals(store.Get<Callsign>(entity), replacement),
            "replacement was not visible through both facades");
        Require(actor.Remove<Callsign>() && !actor.Has<Callsign>(), "removal did not detach the family");
        replacement.Name = "Kept";
        Require(!store.Has<Callsign>(entity) && !store.TryGet(entity, out Callsign? _)
            && replacement.Name == "Kept",
            "removal retained the slot or invalidated a held reference");

        // Facade-side composition and value insertion land in the same authoritative store.
        actor.Add(new Callsign { Name = "Readded" });
        Require(actor.Has<Callsign>() && game.Sign.Name == "Readded",
            "facade composition did not attach");
        actor.Set(new Ping(3));
        Require(store.Get<Ping>(entity) == new Ping(3),
            "facade value insertion was not visible to the store");

        // Entity removal ends facade access with ordinary store errors; the facade itself owns nothing.
        store.Destroy(entity);
        Require(!actor.IsAlive, "destroyed entity still reported alive through the facade");
        ThrowsInvalidOperation(() => actor.Get<StatsComponent>(), "destroyed entity component access succeeded");
        ThrowsInvalidOperation(() => { _ = actor.TypeId; }, "destroyed entity metadata access succeeded");
        ThrowsInvalidOperation(() => { _ = actor.Lifecycle; }, "destroyed entity lifecycle access succeeded");
        ThrowsInvalidOperation(() => new Actor(store, new EntityId(999)), "wrapping an unknown entity succeeded");

        // Releasing a facade cannot destroy its entity: there is no disposal cascade to trigger.
        Require(!typeof(Actor).IsAssignableTo(typeof(IDisposable)), "Actor must not own entity lifetime");
        EntityId survivor = store.Create();
        store.Add(survivor, new Callsign { Name = "Survivor" });
        var released = new Actor(store, survivor);
        released = null;
        GC.Collect(GC.MaxGeneration, GCCollectionMode.Forced, blocking: true);
        GC.WaitForPendingFinalizers();
        Require(store.IsAlive(survivor) && store.Get<Callsign>(survivor).Name == "Survivor",
            "releasing a facade affected its entity");
    }

    /// <summary>
    /// The downstream pattern: a small product-owned wrapper holding an Actor and exposing named
    /// properties over the same attached instances. No inheritance, no registration, no template.
    /// </summary>
    private sealed class GameActor(Actor actor)
    {
        public EntityId Entity => actor.Entity;
        public EntityTypeId TypeId => actor.TypeId;
        public StatsComponent Stats => actor.Get<StatsComponent>();
        public Callsign Sign => actor.Get<Callsign>();
    }

    private sealed class Callsign
    {
        public string Name { get; set; } = string.Empty;
    }

    private readonly record struct Ping(int Count);

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private static void ThrowsInvalidOperation(Action action, string message)
    {
        try { action(); }
        catch (Exception error) when (error.GetType() == typeof(InvalidOperationException)) { return; }
        throw new InvalidOperationException(message);
    }
}
