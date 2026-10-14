using System.Numerics;

namespace Rusty.Engine.Input;

/// <summary>Normalized radial-stick dead-zone parameters. Radii are normalized stick magnitudes and exponent shapes the remapped response.
/// Inner must be below outer in [0, 1] and the exponent positive; <see cref="FpsInput"/> checks its dead zones once when constructed.</summary>
public readonly record struct RadialDeadzone(float InnerRadius, float OuterRadius, float Exponent)
{
    public static RadialDeadzone Standard => new(0.15f, 1f, 1f);
}

/// <summary>Normalized scalar trigger curve parameters. Thresholds are in the inclusive [0, 1] trigger range,
/// inner below outer, and the exponent is positive.</summary>
public readonly record struct TriggerCurve(float InnerThreshold, float OuterThreshold, float Exponent)
{
    public static TriggerCurve Linear => new(0f, 1f, 1f);
}

/// <summary>Small pure functions for normalized controller input and keyboard/controller movement composition.</summary>
public static class AnalogInput
{
    /// <summary>
    /// Applies a radial dead zone and response curve to a normalized stick vector.
    /// Input outside the unit circle is capped before remapping.
    /// </summary>
    public static Vector2 ApplyRadialDeadzone(Vector2 value, RadialDeadzone deadzone)
    {
        float magnitude = value.Length();
        if (magnitude <= deadzone.InnerRadius)
        {
            return Vector2.Zero;
        }

        float cappedMagnitude = MathF.Min(magnitude, deadzone.OuterRadius);
        float normalized = (cappedMagnitude - deadzone.InnerRadius) / (deadzone.OuterRadius - deadzone.InnerRadius);
        return value / magnitude * MathF.Pow(normalized, deadzone.Exponent);
    }

    /// <summary>Applies a normalized [0, 1] trigger threshold and response curve.</summary>
    public static float ApplyTriggerCurve(float value, TriggerCurve curve)
    {
        float cappedValue = Math.Clamp(value, 0f, curve.OuterThreshold);
        if (cappedValue <= curve.InnerThreshold)
        {
            return 0f;
        }

        return MathF.Pow(
            (cappedValue - curve.InnerThreshold) / (curve.OuterThreshold - curve.InnerThreshold),
            curve.Exponent);
    }

    /// <summary>
    /// Adds digital and analog planar movement and caps the result to unit magnitude.
    /// This keeps partial stick deflection partial when no digital direction is held.
    /// </summary>
    public static Vector2 CombineMovement(Vector2 digital, Vector2 analog)
    {
        Vector2 combined = Vector2.Clamp(digital + analog, -Vector2.One, Vector2.One);
        return ClampMagnitude(combined, 1f);
    }

    private static Vector2 ClampMagnitude(Vector2 value, float maximum)
    {
        float lengthSquared = value.LengthSquared();
        return lengthSquared > maximum * maximum ? Vector2.Normalize(value) * maximum : value;
    }
}
