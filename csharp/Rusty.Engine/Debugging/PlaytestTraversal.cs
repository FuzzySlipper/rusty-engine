using System.Numerics;
using System.Text.Json;

namespace Rusty.Engine.Debugging;

public sealed record PlaytestJumpPlan(bool Available, string? Reason, double YawDeltaDegrees,
    double MoveMs, double SettleMs, PlaytestAction? Action,
    string Meaning = "bounded ordinary jump toward target feet; estimate excludes collision and acceleration; inspect actual landing, never teleport or assume arrival");

/// <summary>Small input guidance over the current character tuning. The normal
/// Engine controller remains the only collision and movement solver.</summary>
public static class PlaytestTraversal
{
    public static PlaytestJumpPlan JumpToward(Vector3 feet, Vector3 facing, Vector3 targetFeet,
        CharacterControllerConfig config, bool grounded, string jumpKey, string forwardKey)
    {
        Vector3 d = targetFeet - feet;
        double gravity = config.Vertical.Gravity, speed = config.Ground.ForwardSpeed, jump = config.Vertical.JumpSpeed;
        if (!float.IsFinite(d.X) || !float.IsFinite(d.Y) || !float.IsFinite(d.Z) || gravity <= 0 || speed <= 0 || jump <= 0)
            return new(false, "invalid-target-or-jump-tuning", 0, 0, 0, null);
        if (!grounded) return new(false, "requires-grounded-player", 0, 0, 0, null);
        double discriminant = jump * jump - 2 * gravity * d.Y;
        if (discriminant < 0) return new(false, "target-above-estimated-jump-height", 0, 0, 0, null);
        double flight = (jump + Math.Sqrt(discriminant)) / gravity;
        double move = Math.Sqrt(d.X * d.X + d.Z * d.Z) / speed;
        if (flight > 2 || move > flight || move > 2)
            return new(false, "target-beyond-bounded-jump-window", 0, 0, 0, null);
        double yaw = Math.Atan2(d.X, -d.Z) - Math.Atan2(facing.X, -facing.Z);
        yaw = Math.Atan2(Math.Sin(yaw), Math.Cos(yaw)) * 180 / Math.PI;
        double duration = Math.Max(1000d / 60d, move * 1000);
        return new(true, null, d.X == 0 && d.Z == 0 ? 0 : yaw, duration,
            Math.Clamp(flight * 1000 - duration + 100, 0, 2000),
            new PlaytestAction("jump-toward", jumpKey, duration, false, HeldKeys: move > 0 ? new[] { forwardKey } : Array.Empty<string>()));
    }

    public static string Probe(ISpatialService spatial, SpatialSession session, Vector3 feet,
        float height, float stepHeight, float distance, ReadOnlyMemory<SpatialEntityCollider> entities)
    {
        if (!float.IsFinite(distance) || distance <= 0 || distance > 8)
            throw new ArgumentOutOfRangeException(nameof(distance), "Probe distance must be in (0,8].");
        var samples = new List<object>();
        void Cast(string label, Vector3 origin, Vector3 direction, double range)
        {
            var hit = spatial.CastRay(new SpatialRaycastRequest(session, origin, direction, range,
                new SpatialQueryFilter(1, uint.MaxValue), entities, ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<SpatialEntityCollider>.Empty));
            samples.Add(new { label, origin = V(origin), direction = V(direction), range, hit = hit.Present,
                distance = hit.Present ? hit.Distance : (double?)null, kind = hit.Kind.ToString(),
                entity = hit.Entity.ToString(), instance = hit.Instance.ToString(),
                point = hit.Present ? V(hit.Point) : null, normal = hit.Present ? V(hit.Normal) : null });
        }
        for (int i = 0; i < 8; i++)
        {
            float angle = i * MathF.PI / 4;
            var direction = new Vector3(MathF.Sin(angle), 0, -MathF.Cos(angle));
            Cast($"heading-{i * 45}-ankle", feet + Vector3.UnitY * .1f, direction, distance);
            Cast($"heading-{i * 45}-above-step", feet + Vector3.UnitY * (stepHeight + .05f), direction, distance);
            Cast($"heading-{i * 45}-head", feet + Vector3.UnitY * (height - .1f), direction, distance);
            Cast($"heading-{i * 45}-floor", feet + direction * distance + Vector3.UnitY * (stepHeight + .1f), -Vector3.UnitY, height + stepHeight);
        }
        return JsonSerializer.Serialize(new { meaning = "sampled rays only, not body clearance or guaranteed route; headings 0=-Z,90=+X", samples });
    }
    private static float[] V(Vector3 p) => new[] { p.X, p.Y, p.Z };
}

/// <summary>Product supplies the current session, pose and controller tuning.</summary>
public sealed class SpatialInspectionDebugModule(
    Func<int, int, double, DebugCommandResult> grid,
    Func<double, DebugCommandResult> probe,
    Func<double, double, double, DebugCommandResult> jump) : IDebugCommandModule
{
    [DebugCommand("spatial.grid", Description = "Read-only local XYZ collision grid centered on player feet. radius/verticalRadius are cells; cellSize is world units. Explicit trigger only.")]
    public DebugCommandResult Grid(int radius, int verticalRadius, double cellSize) => grid(radius, verticalRadius, cellSize);
    [DebugCommand("spatial.probe", Description = "Read-only nearby ankle/step/head/floor rays and current character movement diagnostics.")]
    public DebugCommandResult Probe(double distance) => probe(distance);
    [DebugCommand("playtest.jump-plan", Description = "Read-only bounded jump-toward input plan from live character tuning. XYZ is target feet; no movement or guaranteed landing.")]
    public DebugCommandResult Jump(double x, double y, double z) => jump(x, y, z);
}
