using System.Numerics;
using Rusty.Engine.Interaction;

static void Check(bool condition, string message) { if (!condition) throw new Exception(message); }
InteractionCandidate A = new(new(1, 1), "left", new(-.05f, 0, -2), 3, InteractionVisibility.Visible, InteractionAvailability.Available);
InteractionCandidate B = A with { Target = new(2, 1), Label = "right", Point = new(.05f, 0, -2) };
InteractionQuery query = new(Vector3.Zero, -Vector3.UnitZ, .1f, .2f, 8, 9);
InteractionFocus focus = new();
Check(focus.Update([B,A], query).Selected == A.Target, "Stable ID tie ignores enumeration order");
Check(focus.Update([A,B], query with { Direction = new(.04f,0,-1) }).Selected == A.Target, "Sticky focus survives a better-ranked neighbour");
Check(focus.Update([A,B], query, 1).Selected == B.Target, "Explicit cycling");
Check(focus.Observe([A], query).Selected is null, "Observation does not report stale selection");
Check(focus.Selected == B.Target, "Observation does not mutate focus");
Check(focus.Update([A,B], query with { Direction = Vector3.UnitX }).Selected is null, "Release cone clears focus");
Check(focus.Observe([], query).Reason == InteractionReason.NoCandidate, "No candidate");
foreach (var (candidate, expected) in new[] {
 (A with { ReachDistance = 1 }, InteractionReason.OutOfReach),
 (A with { Visibility = InteractionVisibility.Occluded }, InteractionReason.Occluded),
 (A with { Visibility = InteractionVisibility.Unknown }, InteractionReason.VisibilityUnknown),
 (A with { Availability = InteractionAvailability.Locked }, InteractionReason.Locked),
 (A with { Availability = InteractionAvailability.Unavailable }, InteractionReason.Unavailable) })
{
 Check(focus.Update([candidate], query).Selected is null, $"No focus on {expected}");
 Check(InteractionFocus.Revalidate(A.Target, [candidate], query) == expected, $"Use rejects {expected}");
}
Check(InteractionFocus.Revalidate(A.Target, [A with { Target = new(1,2) }], query) == InteractionReason.StaleTarget, "Reused ID invalidates old query");
Check(InteractionFocus.Revalidate(A.Target, [], query) == InteractionReason.InvalidTarget, "Removed target cannot activate");
Check(focus.Update([A],query).Candidates[0].Candidate.Route == InteractionRoute.Unknown, "Visibility does not invent navigation evidence");
Console.WriteLine("Interaction selection, hysteresis, cycling, query and use-time invalidation checks passed.");

focus.Clear();
var releaseOnly = A with { Point = new(.3f,0,-2) };
Check(focus.Update([releaseOnly],query).Reason == InteractionReason.OutsideQuery, "Unacquired release-only target is not Ready");
var offsetCursor = query with { Origin = new(-.05f,0,-1.99f), DistanceOrigin = new(0,0,8) };
Check(InteractionFocus.Revalidate(A.Target,[A],offsetCursor) == InteractionReason.OutsideQuery, "Offset cursor ray does not shorten player distance");
