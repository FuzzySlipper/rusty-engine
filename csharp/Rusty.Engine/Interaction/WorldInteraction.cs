namespace Rusty.Engine.Interaction;

/// <summary>Current product facts sampled on its ordinary serialized update/debug boundary.
/// Candidates must remain valid for the synchronous call; no second world or registry is retained.</summary>
public sealed record InteractionSceneSnapshot(InteractionQuery Query, ReadOnlyMemory<InteractionCandidate> Candidates,
    string Stamp, string Action = "use");
public readonly record struct InteractionActionResult(bool Performed, string Message);

/// <summary>The product supplies current facts and its ordinary action handler (open, talk, use, etc.).
/// Action-specific rules and presentation remain in that handler.</summary>
public interface IWorldInteractionScene
{
    InteractionSceneSnapshot ReadInteraction();
    InteractionActionResult UseInteraction(InteractionTarget target);
}

public sealed record WorldInteractionReadout(InteractionSceneSnapshot Scene, InteractionReadout Focus,
    bool TargetedUseEnabled);
public sealed record InteractionUseReceipt(InteractionTarget? Target, InteractionReason Reason,
    bool Performed, string Message, string Assistance, string Stamp);

/// <summary>Shared human/agent world-object interaction. Targeted use is explicit pixel-precision
/// assistance; it never implies navigation, camera movement or completion of the product action.</summary>
public sealed class WorldInteraction(IWorldInteractionScene scene, bool targetedUseEnabled = true)
{
    private readonly IWorldInteractionScene _scene = scene ?? throw new ArgumentNullException(nameof(scene));
    public InteractionFocus Focus { get; } = new();
    public bool TargetedUseEnabled { get; } = targetedUseEnabled;

    public InteractionReadout Update(int cycleDirection = 0)
    {
        InteractionSceneSnapshot snapshot = _scene.ReadInteraction();
        return Focus.Update(snapshot.Candidates.Span, snapshot.Query, cycleDirection);
    }
    public WorldInteractionReadout Inspect()
    {
        InteractionSceneSnapshot snapshot = _scene.ReadInteraction();
        return new(snapshot, Focus.Inspect(snapshot.Candidates.Span, snapshot.Query), TargetedUseEnabled);
    }
    public InteractionUseReceipt UseFocused() => Use(Focus.Selected, targeted: false);
    public InteractionUseReceipt UseTarget(InteractionTarget target) => Use(target, targeted: true);

    private InteractionUseReceipt Use(InteractionTarget? target, bool targeted)
    {
        InteractionSceneSnapshot snapshot = _scene.ReadInteraction();
        string assistance = targeted ? "target-id" : "focus";
        if (targeted && !TargetedUseEnabled)
            return new(target, InteractionReason.Unavailable, false, "Target-ID use is disabled by this product.", assistance, snapshot.Stamp);
        if (target is not {} identity)
            return new(null, InteractionReason.NoCandidate, false, "No focused target; inspect candidates or aim at an available object.", assistance, snapshot.Stamp);
        InteractionReason reason = targeted
            ? InteractionFocus.RevalidateTarget(identity, snapshot.Candidates.Span, snapshot.Query)
            : InteractionFocus.Revalidate(identity, snapshot.Candidates.Span, snapshot.Query);
        if (reason != InteractionReason.Ready)
            return new(target, reason, false, $"Interaction rejected: {reason}.", assistance, snapshot.Stamp);
        InteractionActionResult action = _scene.UseInteraction(identity);
        return new(target, action.Performed ? InteractionReason.Ready : InteractionReason.Unavailable,
            action.Performed, action.Message, assistance, snapshot.Stamp);
    }
}
