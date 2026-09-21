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

// Target-ID interaction skips only fiddly reticle acquisition, then uses the same action.
var scene = new TestInteractionScene(A with { Point = new(2,0,0) }, query);
var world = new WorldInteraction(scene);
Check(world.Update().Selected is null, "Off-reticle object is not automatically focused");
Check(world.Inspect().Focus.Candidates.Count == 1, "Discovery retains off-reticle targets");
Check(world.UseTarget(A.Target).Performed && scene.Uses == 1, "Explicit target assistance invokes product use");
scene.Candidate = scene.Candidate with { Availability = InteractionAvailability.Locked };
Check(world.UseTarget(A.Target).Reason == InteractionReason.Locked && scene.Uses == 1, "Fresh lock rejects old inspected target");
scene.Candidate = scene.Candidate with { Availability = InteractionAvailability.Available, Visibility = InteractionVisibility.Occluded };
Check(world.UseTarget(A.Target).Reason == InteractionReason.Occluded && scene.Uses == 1, "Target ID does not bypass occlusion");
scene.Candidate = A with { Point = new(0,0,-5) };
Check(world.UseTarget(A.Target).Reason == InteractionReason.OutOfReach, "Target ID does not bypass reach");
scene.Candidate = A with { Target = new(1,2) };
Check(world.UseTarget(A.Target).Reason == InteractionReason.StaleTarget, "Fresh incarnation rejects stale ID/revision");
scene.Candidate = A;
world.Update();
Check(world.UseFocused().Performed && scene.Uses == 2, "Human focus uses the identical product handler");
Check(!new WorldInteraction(scene, targetedUseEnabled: false).UseTarget(A.Target).Performed, "Product can disable targeted use");
var debug = new Rusty.Engine.Debugging.InteractionDebugModule(world);
using (var json = System.Text.Json.JsonDocument.Parse(debug.Inspect().Message))
{
 Check(json.RootElement.GetProperty("candidates")[0].GetProperty("useCommand").GetString() == "interaction.use 1 1", "Inspection gives exact action command");
}
scene.Candidate = A with { Availability = InteractionAvailability.Locked };
using (var json = System.Text.Json.JsonDocument.Parse(debug.Use(1,1).Message))
 Check(!json.RootElement.GetProperty("performed").GetBoolean(), "Command receipt reports domain rejection");

var assist = new AimAssist();
var config = new AimAssistConfig(.2f, .3f, .5f, .15f, .1f);
var target = A with { Point = new(.2f,0,-2) };
var aimQuery = query with { AcquireAngleRadians = .2f, ReleaseAngleRadians = .3f };
var tracked = assist.Update([target], aimQuery, new(.02f,0), .016f, config, active:true);
Check(tracked.Focus.Selected == target.Target && tracked.SlowdownScale < 1, "Acquisition slows stick near target");
Check(tracked.CorrectionRadians.Length() <= .00801f && tracked.CorrectionRadians.X > 0, "Tracking bounded by admitted simulation time");
var away = assist.Update([target], aimQuery, new(-.02f,0), .016f, config, active:true);
Check(away.LookDeltaRadians == new Vector2(-.02f,0) && away.CorrectionRadians == Vector2.Zero, "Deliberate away input remains free");
var mixedTarget = target with { Point = new(.1f,.1f,-2) };
var mixed = assist.Update([mixedTarget], aimQuery, new(.02f,-.001f), .016f, config, active:true);
Check(mixed.LookDeltaRadians.Y == -.001f && mixed.CorrectionRadians.Y == 0 && mixed.AppliedLookScale.Y == 1,
 "Toward yaw does not permit assistance to reverse deliberate away pitch");
var centered = assist.Update([target with { Point = new(0,0,-2) }], aimQuery, new(.001f,-.002f), .016f, config, active:true);
Check(centered.LookDeltaRadians == new Vector2(.001f,-.002f), "Centered target cannot pin deliberate look input");
var orthogonal = assist.Update([target], aimQuery, new(.02f,.001f), .016f, config, active:true);
Check(orthogonal.LookDeltaRadians.Y == .001f && orthogonal.CorrectionRadians.Y == 0, "Orthogonal pitch input stays free");
var corrected = assist.CorrectShot(-Vector3.UnitZ, [target], aimQuery, config, active:true);
Check(corrected.Assisted && corrected.Direction.X > 0 && corrected.CorrectionRadians <= .1f, "Shot correction bounded toward selected target");
Check(!assist.CorrectShot(-Vector3.UnitZ, [target with { Visibility = InteractionVisibility.Occluded }], aimQuery, config, active:true).Assisted, "Fresh shot occlusion disables magnetism");
Check(!assist.CorrectShot(-Vector3.UnitZ, [target with { Target = new(1,2) }], aimQuery, config, active:true).Assisted, "Reincarnated target cannot attract shot");
Check(assist.Update([target], aimQuery, Vector2.Zero, .016f, config, active:false).Focus.Selected is null, "Disabling controller assistance clears target");
Console.WriteLine("World interaction discovery/shared use and bounded controller aim assistance checks passed.");

sealed class TestInteractionScene(InteractionCandidate candidate, InteractionQuery query) : IWorldInteractionScene
{
 public InteractionCandidate Candidate = candidate;
 public int Uses;
 public InteractionSceneSnapshot ReadInteraction() => new(query, new[]{Candidate}, "test", "open container");
 public InteractionActionResult UseInteraction(InteractionTarget target) { Uses++; return new(true,"Container opened"); }
}
