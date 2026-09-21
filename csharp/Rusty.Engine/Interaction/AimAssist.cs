using System.Numerics;

namespace Rusty.Engine.Interaction;

/// <summary>Product-selected assistance. Angles are radians; tracking is radians per simulation second.
/// Selection cones and range belong to InteractionQuery. Zero tracking/magnetism disables that contribution.</summary>
public sealed record AimAssistConfig(float SlowdownAngleRadians, float MinimumLookScale,
    float TrackingRadiansPerSecond, float ShotConeRadians, float MaximumShotCorrectionRadians);

public sealed record AimAssistReadout(InteractionReadout Focus, Vector2 LookDeltaRadians,
    Vector2 CorrectionRadians, float SlowdownScale, bool Active)
{
    /// <summary>Per-axis applied scale; deliberate away axes remain 1 even when the other axis slows.</summary>
    public Vector2 AppliedLookScale { get; init; } = Vector2.One;
}
public readonly record struct AimShotReadout(Vector3 Direction, InteractionTarget? Target,
    float CorrectionRadians, bool Assisted);

/// <summary>Shared stick assistance over fresh product candidates. Product owns activation, eligibility,
/// tuning and application of returned look/shot values. No input loop, weapon effect or collision bypass.</summary>
public sealed class AimAssist
{
    private const float AngularEpsilon = 0.000001f;
    public InteractionFocus Focus { get; } = new();

    /// <summary>Input is already time-integrated yaw-right/pitch-up radians. Query direction is the
    /// look before those deltas. Deliberate input away from the target receives no slowdown/tracking.</summary>
    public AimAssistReadout Update(ReadOnlySpan<InteractionCandidate> candidates, InteractionQuery query,
        Vector2 lookDeltaRadians, float simulationSeconds, AimAssistConfig config, bool active)
    {
        Validate(config);
        if (!float.IsFinite(simulationSeconds) || simulationSeconds < 0 || !Finite(lookDeltaRadians))
            throw new ArgumentOutOfRangeException(nameof(simulationSeconds));
        if (!active) Focus.Clear();
        InteractionReadout focus = active ? Focus.Update(candidates, query) : Focus.Observe(candidates, query);
        if (!active || focus.Selected is null)
            return new(focus, lookDeltaRadians, Vector2.Zero, 1, active);
        InteractionObservation selected = focus.Candidates.First(row => row.Selected);
        Vector3 offset = selected.Candidate.Point - query.Origin;
        if (offset.LengthSquared() <= float.Epsilon)
            return new(focus, lookDeltaRadians, Vector2.Zero, 1, active);
        Vector2 error = AngularError(query.Direction, offset);
        bool awayYaw = lookDeltaRadians.X != 0 && (MathF.Abs(error.X) <= AngularEpsilon || error.X * lookDeltaRadians.X < 0);
        bool awayPitch = lookDeltaRadians.Y != 0 && (MathF.Abs(error.Y) <= AngularEpsilon || error.Y * lookDeltaRadians.Y < 0);
        if (awayYaw && awayPitch)
            return new(focus, lookDeltaRadians, Vector2.Zero, 1, active);
        float proximity = config.SlowdownAngleRadians > 0
            ? 1 - Math.Clamp(selected.AngleRadians / config.SlowdownAngleRadians, 0, 1) : 0;
        float scale = 1 - proximity * (1 - config.MinimumLookScale);
        Vector2 appliedScale = new(awayYaw ? 1 : scale, awayPitch ? 1 : scale);
        Vector2 input = lookDeltaRadians * appliedScale;
        Vector2 correction = new(awayYaw ? 0 : error.X - input.X, awayPitch ? 0 : error.Y - input.Y);
        float maximum = config.TrackingRadiansPerSecond * simulationSeconds;
        float length = correction.Length();
        if (length > maximum && length > 0) correction *= maximum / length;
        return new(focus, input + correction, correction, scale, active) { AppliedLookScale = appliedScale };
    }

    /// <summary>Corrects only toward the retained, freshly revalidated target inside the shot cone.
    /// Cast the returned direction through ordinary world collision; this does not assert a hit.</summary>
    public AimShotReadout CorrectShot(Vector3 direction, ReadOnlySpan<InteractionCandidate> candidates,
        InteractionQuery query, AimAssistConfig config, bool active)
    {
        Validate(config);
        if (!Finite(direction) || direction.LengthSquared() <= 0 || !float.IsFinite(direction.LengthSquared()))
            throw new ArgumentException("Shot direction must be finite and nonzero.", nameof(direction));
        direction = Vector3.Normalize(direction);
        AimShotReadout unchanged = new(direction, null, 0, false);
        if (!active || Focus.Selected is not {} target || config.MaximumShotCorrectionRadians == 0)
            return unchanged;
        query = query with { Direction = direction };
        if (InteractionFocus.Revalidate(target, candidates, query) != InteractionReason.Ready)
            return unchanged;
        InteractionCandidate candidate = default;
        foreach (InteractionCandidate current in candidates)
            if (current.Target == target) { candidate = current; break; }
        Vector3 offset = candidate.Point - query.Origin;
        if (offset.LengthSquared() <= float.Epsilon) return unchanged;
        Vector3 desired = Vector3.Normalize(offset);
        float angle = MathF.Acos(Math.Clamp(Vector3.Dot(direction, desired), -1, 1));
        if (angle > config.ShotConeRadians) return unchanged;
        float correction = MathF.Min(angle, config.MaximumShotCorrectionRadians);
        if (angle <= AngularEpsilon) return new(direction, target, 0, false);
        float fraction = correction / angle;
        Vector3 corrected = Vector3.Normalize((MathF.Sin((1 - fraction) * angle) * direction
            + MathF.Sin(fraction * angle) * desired) / MathF.Sin(angle));
        return new(corrected, target, correction, correction > 0);
    }

    private static Vector2 AngularError(Vector3 direction, Vector3 target)
    {
        float yaw = MathF.Atan2(target.X, -target.Z) - MathF.Atan2(direction.X, -direction.Z);
        yaw = MathF.Atan2(MathF.Sin(yaw), MathF.Cos(yaw));
        float pitch = MathF.Atan2(target.Y, MathF.Sqrt(target.X * target.X + target.Z * target.Z))
            - MathF.Atan2(direction.Y, MathF.Sqrt(direction.X * direction.X + direction.Z * direction.Z));
        return new(yaw, pitch);
    }
    private static bool Finite(Vector2 x) => float.IsFinite(x.X) && float.IsFinite(x.Y);
    private static bool Finite(Vector3 x) => float.IsFinite(x.X) && float.IsFinite(x.Y) && float.IsFinite(x.Z);
    private static void Validate(AimAssistConfig c)
    {
        if (!float.IsFinite(c.SlowdownAngleRadians) || c.SlowdownAngleRadians < 0 || c.SlowdownAngleRadians > MathF.PI
            || !float.IsFinite(c.MinimumLookScale) || c.MinimumLookScale < 0 || c.MinimumLookScale > 1
            || !float.IsFinite(c.TrackingRadiansPerSecond) || c.TrackingRadiansPerSecond < 0
            || !float.IsFinite(c.ShotConeRadians) || c.ShotConeRadians < 0 || c.ShotConeRadians > MathF.PI / 2
            || !float.IsFinite(c.MaximumShotCorrectionRadians) || c.MaximumShotCorrectionRadians < 0
            || c.MaximumShotCorrectionRadians > c.ShotConeRadians)
            throw new ArgumentException("Invalid aim assistance configuration.", nameof(c));
    }
}
