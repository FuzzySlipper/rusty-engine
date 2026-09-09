using System.Numerics;

namespace Rusty.Engine.Interaction;

/// <summary>Product identity plus incarnation; reuse of an identity must change its revision.</summary>
public readonly record struct InteractionTarget(ulong Id, ulong Revision);
public enum InteractionVisibility { Unknown, Visible, Occluded }
public enum InteractionAvailability { Available, Unavailable, Locked, Invalid }
public enum InteractionRoute { Unknown, Reachable, Unreachable }
public enum InteractionReason { Ready, NoCandidate, OutsideQuery, OutOfReach, VisibilityUnknown, Occluded, Unavailable, Locked, InvalidTarget, StaleTarget }

/// <summary>Call-local product facts. ReachDistance is interaction distance in world units;
/// Route is separate evidence about walking, never inferred from line of sight.</summary>
public readonly record struct InteractionCandidate(
    InteractionTarget Target, string Label, Vector3 Point, float ReachDistance,
    InteractionVisibility Visibility, InteractionAvailability Availability,
    float Priority = 0, InteractionRoute Route = InteractionRoute.Unknown);

/// <summary>Reticle or free-cursor ray. Cone angles are radians and distances are world units.
/// Release bounds must contain acquisition bounds. Higher priority wins, then weighted
/// angular/distance score, then stable ID/revision. DistanceOrigin optionally separates
/// player reach from an offset free-cursor ray origin. Weights are nonnegative.</summary>
public readonly record struct InteractionQuery(
    Vector3 Origin, Vector3 Direction, float AcquireAngleRadians, float ReleaseAngleRadians,
    float MaximumDistance, float ReleaseDistance, float AngularWeight = 1, float DistanceWeight = 0, Vector3? DistanceOrigin = null);

public readonly record struct InteractionObservation(
    InteractionCandidate Candidate, float Distance, float AngleRadians, bool WithinAcquisition,
    bool WithinRelease, InteractionReason Reason, bool Selected);

/// <summary>Copied query facts. Selection never implies activation or a walking route.</summary>
public sealed record InteractionReadout(InteractionTarget? Selected, InteractionReason Reason,
    IReadOnlyList<InteractionObservation> Candidates);

/// <summary>Optional product-held sticky focus. No registry, spatial state, input loop or effects.</summary>
public sealed class InteractionFocus
{
    public InteractionTarget? Selected { get; private set; }
    public void Clear() => Selected = null;

    /// <summary>Updates ordinary human focus. Cycle direction is -1, 0 or +1; cycling is explicit.</summary>
    public InteractionReadout Update(ReadOnlySpan<InteractionCandidate> candidates, InteractionQuery query, int cycleDirection = 0)
    {
        if (cycleDirection is < -1 or > 1) throw new ArgumentOutOfRangeException(nameof(cycleDirection));
        List<InteractionObservation> rows = Evaluate(candidates, query);
        int retainedIndex = rows.FindIndex(x => x.Candidate.Target == Selected && x.WithinRelease && x.Reason == InteractionReason.Ready);
        InteractionObservation? retained = retainedIndex >= 0 ? rows[retainedIndex] : null;
        List<InteractionObservation> eligible = rows.FindAll(x => x.WithinAcquisition && x.Reason == InteractionReason.Ready);
        InteractionTarget? next = retained?.Candidate.Target;
        if (cycleDirection != 0 && eligible.Count > 0)
        {
            int current = eligible.FindIndex(x => x.Candidate.Target == Selected);
            int chosen = current < 0 ? (cycleDirection > 0 ? 0 : eligible.Count - 1)
                : (current + cycleDirection + eligible.Count) % eligible.Count;
            next = eligible[chosen].Candidate.Target;
        }
        else if (next is null && eligible.Count > 0) next = eligible[0].Candidate.Target;
        Selected = next;
        return Readout(rows, next);
    }

    /// <summary>Read-only agent or UI query; does not acquire, switch, aim or use a target.</summary>
    public InteractionReadout Observe(ReadOnlySpan<InteractionCandidate> candidates, InteractionQuery query)
        => Readout(Evaluate(candidates, query), Selected);

    /// <summary>Rechecks freshly supplied facts at use time. The product then invokes its ordinary
    /// action path only on Ready. Never use a previously returned observation as authorization.</summary>
    public static InteractionReason Revalidate(InteractionTarget target, ReadOnlySpan<InteractionCandidate> current, InteractionQuery query)
    {
        Validate(query);
        bool identityPresent = false;
        foreach (InteractionCandidate candidate in current)
        {
            if (candidate.Target.Id != target.Id) continue;
            identityPresent = true;
            if (candidate.Target != target) continue;
            InteractionObservation row = EvaluateOne(candidate, query);
            return row.WithinRelease ? row.Reason : InteractionReason.OutsideQuery;
        }
        return identityPresent ? InteractionReason.StaleTarget : InteractionReason.InvalidTarget;
    }

    private static InteractionReadout Readout(List<InteractionObservation> rows, InteractionTarget? selected)
    {
        InteractionTarget? live = null;
        for (int i = 0; i < rows.Count; i++)
        {
            bool isSelected = rows[i].Candidate.Target == selected && rows[i].WithinRelease && rows[i].Reason == InteractionReason.Ready;
            rows[i] = rows[i] with { Selected = isSelected };
            if (isSelected) live = selected;
        }
        InteractionReason reason = InteractionReason.NoCandidate;
        if (live.HasValue) reason = InteractionReason.Ready;
        else if (rows.Count > 0)
        {
            int acquired = rows.FindIndex(x => x.WithinAcquisition);
            reason = acquired >= 0 ? rows[acquired].Reason : InteractionReason.OutsideQuery;
        }
        return new(live, reason, rows.AsReadOnly());
    }

    private static List<InteractionObservation> Evaluate(ReadOnlySpan<InteractionCandidate> candidates, InteractionQuery query)
    {
        Validate(query);
        List<InteractionObservation> rows = new();
        HashSet<ulong> identities = new();
        foreach (InteractionCandidate candidate in candidates)
        {
            if (!identities.Add(candidate.Target.Id)) throw new ArgumentException("Candidate IDs must be unique within a query.", nameof(candidates));
            InteractionObservation row = EvaluateOne(candidate, query);
            if (row.WithinRelease) rows.Add(row);
        }
        rows.Sort((a, b) =>
        {
            int priority = b.Candidate.Priority.CompareTo(a.Candidate.Priority);
            if (priority != 0) return priority;
            double Score(InteractionObservation x) => query.AngularWeight * (double)x.AngleRadians + query.DistanceWeight * (double)x.Distance;
            int score = Score(a).CompareTo(Score(b));
            return score != 0 ? score : a.Candidate.Target.Id.CompareTo(b.Candidate.Target.Id);
        });
        return rows;
    }

    private static InteractionObservation EvaluateOne(InteractionCandidate candidate, InteractionQuery query)
    {
        if (!Finite(candidate.Point) || !float.IsFinite(candidate.ReachDistance) || candidate.ReachDistance < 0 || !float.IsFinite(candidate.Priority))
            throw new ArgumentException("Candidate point, reach and priority must be finite; reach must be nonnegative.", nameof(candidate));
        Vector3 offset = candidate.Point - query.Origin;
        float distance = Vector3.Distance(candidate.Point, query.DistanceOrigin ?? query.Origin);
        float angle = offset.LengthSquared() == 0 ? 0 : MathF.Acos(Math.Clamp(Vector3.Dot(Vector3.Normalize(offset), Vector3.Normalize(query.Direction)), -1, 1));
        bool acquire = distance <= query.MaximumDistance && angle <= query.AcquireAngleRadians;
        bool release = distance <= query.ReleaseDistance && angle <= query.ReleaseAngleRadians;
        InteractionReason reason = candidate.Availability switch
        {
            InteractionAvailability.Invalid => InteractionReason.InvalidTarget,
            InteractionAvailability.Locked => InteractionReason.Locked,
            InteractionAvailability.Unavailable => InteractionReason.Unavailable,
            _ => candidate.Visibility switch
            {
                InteractionVisibility.Occluded => InteractionReason.Occluded,
                InteractionVisibility.Unknown => InteractionReason.VisibilityUnknown,
                _ => distance > candidate.ReachDistance ? InteractionReason.OutOfReach : InteractionReason.Ready,
            },
        };
        if (!release) reason = InteractionReason.OutsideQuery;
        return new(candidate, distance, angle, acquire, release, reason, false);
    }

    private static bool Finite(Vector3 x) => float.IsFinite(x.X) && float.IsFinite(x.Y) && float.IsFinite(x.Z);
    private static void Validate(InteractionQuery q)
    {
        if (!Finite(q.Origin) || (q.DistanceOrigin is {} distanceOrigin && !Finite(distanceOrigin)) || !Finite(q.Direction) || q.Direction.LengthSquared() <= 0 || !float.IsFinite(q.Direction.LengthSquared())
            || !float.IsFinite(q.AcquireAngleRadians) || !float.IsFinite(q.ReleaseAngleRadians)
            || q.AcquireAngleRadians < 0 || q.ReleaseAngleRadians < q.AcquireAngleRadians || q.ReleaseAngleRadians > MathF.PI
            || !float.IsFinite(q.MaximumDistance) || !float.IsFinite(q.ReleaseDistance) || q.MaximumDistance < 0 || q.ReleaseDistance < q.MaximumDistance
            || !float.IsFinite(q.AngularWeight) || q.AngularWeight < 0 || !float.IsFinite(q.DistanceWeight) || q.DistanceWeight < 0)
            throw new ArgumentException("Invalid interaction ray, cone, distance or ranking configuration.", nameof(q));
    }
}
