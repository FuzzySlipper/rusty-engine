using Rusty.Engine.Entities;
using Rusty.Engine.Debugging;

internal static class ClassComponentExercise
{
    public static void Run()
    {
        ExerciseAutomaticKeyPromotion();
        using var store = new EntityStore();
        EntityId actor = store.Create();
        EntityId other = store.Create();
        var state = new ActorState { Health = 10 };
        store.Add(actor, state);
        Require(ReferenceEquals(state, store.Get<ActorState>(actor)), "attachment copied a class");
        Require(store.TryGet(actor, out ActorState? found) && ReferenceEquals(state, found), "TryGet copied a class");
        Throws(() => store.Add(actor, new ActorState()), "duplicate attachment was accepted");
        Throws(() => store.Add<ActorState>(other, null!), "null component was accepted");
        Throws(() => store.Replace(other, new ActorState()), "replacement created an absent slot");
        Require(!store.Has<ActorState>(other) && !store.Remove<ActorState>(other), "absent slot semantics changed");

        ulong beforeMutation = store.Revision;
        state.Health = 8;
        Require(store.Get<ActorState>(actor).Health == 8 && store.Revision == beforeMutation,
            "in-place mutation must be live without pretending to advance a structural version");
        store.Replace(actor, state);
        Require(store.Revision == beforeMutation, "same-instance replacement acted as a dirty notification");

        IReadOnlyList<EntityComponent<ActorState>> membership = store.Query<ActorState>();
        store.Add(other, new ActorState { Health = 20 });
        state.Health = 7;
        Require(membership.Count == 1 && membership[0].Value.Health == 7,
            "query membership must be stable while class state remains live");
        var replacement = new ActorState { Health = 30 };
        store.Replace(actor, replacement);
        state.Health = 6;
        Require(ReferenceEquals(store.Get<ActorState>(actor), replacement) && membership[0].Value.Health == 6,
            "replacement invalidated an existing C# reference");
        Require(store.Remove<ActorState>(actor) && !store.Remove<ActorState>(actor), "removal result was incorrect");
        Require(!replacement.Disposed, "removal disposed a product component");
        replacement.Health = 31;
        Require(replacement.Health == 31 && !store.Has<ActorState>(actor), "removed references must remain ordinary objects");

        // Sharing is ordinary explicit aliasing, not a global ownership violation.
        store.Add(actor, state);
        store.Replace(other, state);
        state.Health = 5;
        Require(store.Get<ActorState>(other).Health == 5, "deliberate aliasing was intercepted");
        store.SetLifecycle(other, EntityLifecycle.Disabled);
        Require(store.Query<ActorState>().Count == 1 && store.Query<ActorState>(includeDisabled: true).Count == 2,
            "disabled filtering changed");
        Require(store.Query<ActorState>(true).Select(row => row.Entity).SequenceEqual([actor, other]), "query order changed");

        var facts = ComponentType<Position>.Create(ProductComponentKeys.Create(1));
        store.Register(facts);
        store.Set(actor, new Position(3));
        Require(store.Get(actor, facts).X == 3 && store.Get<Position>(actor).X == 3,
            "descriptor and generic access do not share one canonical family");
        Position copy = store.Get<Position>(actor);
        copy = copy with { X = 4 };
        Require(store.Get<Position>(actor).X == 3, "value read was not a copy");
        store.Replace(actor, copy);
        Require(store.Query<ActorState, Position>().Single().Second.X == 4, "mixed class/value join failed");
        Throws(() => store.Register(ComponentType<Position>.Create(ProductComponentKeys.Create(2))),
            "multiple descriptors created competing storage for one type");

        var explicitState = ComponentType<ActorState>.Create(ProductComponentKeys.Create(3));
        store.Register(explicitState); // Attach first, then opt into a named legacy/debug descriptor.
        Require(ReferenceEquals(store.Get(actor, explicitState), state), "descriptor binding replaced generic attachment");
        var debug = new EntityStoreDebugModule();
        debug.RegisterStore("actors", store);
        debug.RegisterProjection(explicitState, static (in ActorState value) => $"health={value.Health}");
        Require(debug.GetComponent("actors", actor.Value, explicitState.Key.Value).Message.Contains("health=5"),
            "class debug projection did not read the attached instance");
        ComponentRevision slot = store.GetComponentRevision(actor, explicitState);
        state.Health = 4;
        Require(debug.GetComponent("actors", actor.Value, explicitState.Key.Value).Message.Contains("health=4"),
            "class debug projection cached by structural revision");
        store.Replace(actor, state);
        Require(store.GetComponentRevision(actor, explicitState) == slot, "slot revision tracked object internals");
        // Typed preparation replaces selected value slots only. Class identity/state remains live.
        EntityEdit edit = store.PrepareBatch(new EntityBatch().Set(actor, facts, new Position(9)), store.Revision);
        state.Health = 2;
        edit.Publish();
        Require(ReferenceEquals(store.Get<ActorState>(actor), state) && state.Health == 2
            && store.Get<Position>(actor).X == 9, "typed value edit overwrote a live class");
        Throws(() => store.PrepareBatch(new EntityBatch().Set(actor, facts, new Position(12))
            .Set(new EntityId(999), facts, new Position(13))), "invalid value edit was accepted");
        Require(store.Get<Position>(actor).X == 9, "failed preparation changed a live value");

        EntityId child = store.Create();
        store.SetContainment(child, actor);
        store.SetContainment(actor, other);
        store.Destroy(actor);
        Require(!store.IsAlive(actor) && !store.Has<ActorState>(actor) && !store.TryGet(actor, out ActorState? _),
            "destroy retained component membership");
        Require(store.IsAlive(child) && !store.TryGetContainedIn(child, out _) && store.ContainedEntities(other).Count == 0,
            "destroy cascaded into children or retained relations");
        Require(!state.Disposed && state.Health == 2 && store.Get<ActorState>(other).Health == 2,
            "destroy disposed or invalidated shared references");
        Require(store.Diagnostics().TombstonedCount == 0 && store.Diagnostics().EntityCount == 2,
            "destroy retained a tombstone record");
        Throws(() => store.Get<ActorState>(actor), "destroyed entity lookup succeeded");
        EntityId next = store.Create();
        Require(next.Value > child.Value, "destroy allowed ID reuse");

        store.Add<IActor>(next, new ActorState());
        Require(store.Has<IActor>(next) && !store.Has<ActorState>(next), "runtime type replaced explicit generic family");
        store.Dispose();
        Require(!state.Disposed, "store disposal took ownership of component disposal");
        Throws(() => store.Query<ActorState>(), "disposed store allowed access");
    }

    private static void ExerciseAutomaticKeyPromotion()
    {
        const uint HighestProductLocalId = uint.MaxValue - ProductComponentKeys.FirstProductValue + 1;
        using var store = new EntityStore();
        EntityId entity = store.Create();
        var state = new ActorState();
        store.Add(entity, state);
        var facts = ComponentType<Position>.Create(ProductComponentKeys.Create(HighestProductLocalId));
        store.Register(facts); // The automatic class family must yield this caller-selected key.
        store.Set(entity, facts, new Position(1));
        var explicitState = ComponentType<ActorState>.Create(ProductComponentKeys.Create(HighestProductLocalId - 1));
        store.Register(explicitState); // Promotion also works at the family's own automatic key.
        Require(ReferenceEquals(store.Get(entity, explicitState), state) && store.Get<Position>(entity).X == 1,
            "automatic diagnostic keys blocked explicit descriptor registration");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private static void Throws(Action action, string message)
    {
        try { action(); }
        catch (Exception error) when (error is InvalidOperationException or ArgumentException or ObjectDisposedException) { return; }
        throw new InvalidOperationException(message);
    }

    private interface IActor { }
    private sealed class ActorState : IActor, IDisposable
    {
        public int Health { get; set; }
        public bool Disposed { get; private set; }
        public void Dispose() => Disposed = true;
    }
    private readonly record struct Position(int X);
}
