using System.Numerics;

namespace Rusty.Engine;

/// <summary>Product-held first-person orientation in radians.</summary>
public readonly record struct LookState(float YawRadians, float PitchRadians);

/// <summary>Explicit product tuning for one call-local look integration.</summary>
public readonly record struct LookConfig(
    float HorizontalRadiansPerUnit,
    float VerticalRadiansPerUnit,
    float MinimumPitchRadians,
    float MaximumPitchRadians,
    float MaximumDeltaRadians,
    bool InvertHorizontal,
    bool InvertVertical,
    bool WrapYaw);

/// <summary>One pointer or stick delta applied to product-held look state.</summary>
public readonly record struct LookRequest(LookState State, Vector2 Delta, LookConfig Config);

/// <summary>One explicit replacement of product-held look state.</summary>
public readonly record struct LookRebaseRequest(LookState State, LookState Target, LookConfig Config);

/// <summary>One reset of product-held look state.</summary>
public readonly record struct LookResetRequest(LookState State);

/// <summary>Copied orientation and basis facts for one look operation.</summary>
public readonly record struct LookReceipt(
    LookState Before,
    LookState After,
    Quaternion Orientation,
    Vector3 Forward,
    Vector3 Right,
    Vector3 Up);

/// <summary>A stable classification for a rejected look request.</summary>
public enum LookDiagnostic : uint
{
    Accepted = 0,
    InvalidConfig = 1,
    InvalidState = 2,
    InvalidCommand = 3,
    DeltaLimitExceeded = 4,
}

/// <summary>
/// Call-local first-person look math. It owns no Engine state and does not
/// cross the native boundary; products retain their own <see cref="LookState"/>.
/// </summary>
public static class Look
{
    private const float HalfPi = MathF.PI / 2f;
    private const float Tau = MathF.PI * 2f;

    /// <summary>Applies one delta while preserving the established radian units and limits.</summary>
    public static LookReceipt Integrate(LookRequest request)
    {
        LookDiagnostic diagnostic = Diagnose(request);
        if (diagnostic != LookDiagnostic.Accepted)
        {
            throw new ArgumentException($"Look request was rejected: {diagnostic}.", nameof(request));
        }

        float yawDelta = request.Delta.X * request.Config.HorizontalRadiansPerUnit
            * (request.Config.InvertHorizontal ? -1f : 1f);
        float pitchDelta = request.Delta.Y * request.Config.VerticalRadiansPerUnit
            * (request.Config.InvertVertical ? -1f : 1f);
        return Receipt(
            request.State,
            Normalize(
                request.Config,
                new LookState(
                    request.State.YawRadians + yawDelta,
                    request.State.PitchRadians + pitchDelta)));
    }

    /// <summary>Returns neutral look state even when the previous accumulator was corrupt.</summary>
    public static LookReceipt Reset(LookResetRequest request) => Receipt(request.State, default);

    /// <summary>Replaces state while applying the established yaw and pitch normalization.</summary>
    public static LookReceipt Rebase(LookRebaseRequest request)
    {
        if (!IsValidConfig(request.Config))
        {
            throw new ArgumentException("Look rebase has an invalid configuration.", nameof(request));
        }
        if (!IsValidState(request.State) || !IsValidState(request.Target))
        {
            throw new ArgumentException("Look rebase has an invalid state.", nameof(request));
        }
        return Receipt(request.State, Normalize(request.Config, request.Target));
    }

    /// <summary>Classifies a request without changing product-held state.</summary>
    public static LookDiagnostic Diagnose(LookRequest request)
    {
        if (!IsValidConfig(request.Config)) return LookDiagnostic.InvalidConfig;
        if (!IsValidState(request.State)) return LookDiagnostic.InvalidState;
        if (!float.IsFinite(request.Delta.X) || !float.IsFinite(request.Delta.Y))
        {
            return LookDiagnostic.InvalidCommand;
        }

        float yawDelta = request.Delta.X * request.Config.HorizontalRadiansPerUnit;
        float pitchDelta = request.Delta.Y * request.Config.VerticalRadiansPerUnit;
        return MathF.Abs(yawDelta) > request.Config.MaximumDeltaRadians
                || MathF.Abs(pitchDelta) > request.Config.MaximumDeltaRadians
            ? LookDiagnostic.DeltaLimitExceeded
            : LookDiagnostic.Accepted;
    }

    private static LookReceipt Receipt(LookState before, LookState after)
    {
        (float sinYaw, float cosYaw) = MathF.SinCos(after.YawRadians);
        (float sinPitch, float cosPitch) = MathF.SinCos(after.PitchRadians);
        Vector3 forward = new(sinYaw * cosPitch, sinPitch, -cosYaw * cosPitch);
        Vector3 right = new(cosYaw, 0f, sinYaw);
        Vector3 up = Vector3.Cross(right, forward);
        (float sinHalfYaw, float cosHalfYaw) = MathF.SinCos(after.YawRadians * 0.5f);
        (float sinHalfPitch, float cosHalfPitch) = MathF.SinCos(after.PitchRadians * 0.5f);
        return new LookReceipt(
            before,
            after,
            new Quaternion(
                -sinHalfPitch * sinHalfYaw,
                sinHalfPitch * cosHalfYaw,
                cosHalfPitch * sinHalfYaw,
                cosHalfPitch * cosHalfYaw),
            forward,
            right,
            up);
    }

    private static LookState Normalize(LookConfig config, LookState state) => new(
        config.WrapYaw ? WrapRadians(state.YawRadians) : state.YawRadians,
        Math.Clamp(state.PitchRadians, config.MinimumPitchRadians, config.MaximumPitchRadians));

    private static float WrapRadians(float value) => value + MathF.PI
        - Tau * MathF.Floor((value + MathF.PI) / Tau) - MathF.PI;

    private static bool IsValidState(LookState state)
        => float.IsFinite(state.YawRadians) && float.IsFinite(state.PitchRadians);

    private static bool IsValidConfig(LookConfig config)
        => float.IsFinite(config.HorizontalRadiansPerUnit)
            && float.IsFinite(config.VerticalRadiansPerUnit)
            && float.IsFinite(config.MinimumPitchRadians)
            && float.IsFinite(config.MaximumPitchRadians)
            && float.IsFinite(config.MaximumDeltaRadians)
            && config.HorizontalRadiansPerUnit >= 0f
            && config.VerticalRadiansPerUnit >= 0f
            && config.MinimumPitchRadians < config.MaximumPitchRadians
            && config.MinimumPitchRadians >= -HalfPi
            && config.MaximumPitchRadians <= HalfPi
            && config.MaximumDeltaRadians > 0f;
}
