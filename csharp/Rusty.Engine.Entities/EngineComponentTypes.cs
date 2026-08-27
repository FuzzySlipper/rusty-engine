using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// SDK-owned descriptors for generated Engine values that may participate in a managed entity
/// world. These are value facts only; they do not make this world a mirror of Rust entity-state.
/// </summary>
public static class EngineComponentTypes
{
    private const float MaxAbsTranslation = 1_000_000f;
    private const float MaxAbsVelocity = 10_000f;
    private const float MaxCharacterTimerSeconds = 60f;
    private const float QuaternionNormalizationTolerance = 0.001f;

    // Values below 1024 are reserved for Engine-maintained descriptors.
    public static ComponentType<Transform> Transform { get; } = ComponentType<Transform>.CreateEngine(
        EngineComponentKeys.Create(1),
        validator: ValidateTransform);

    public static ComponentType<CharacterMotion> CharacterMotion { get; } = ComponentType<CharacterMotion>.CreateEngine(
        EngineComponentKeys.Create(2),
        validator: ValidateCharacterMotion);

    private static void ValidateTransform(in Transform value)
    {
        if (!IsFinite(value.Translation) || !IsFinite(value.Scale)
            || MathF.Abs(value.Translation.X) > MaxAbsTranslation
            || MathF.Abs(value.Translation.Y) > MaxAbsTranslation
            || MathF.Abs(value.Translation.Z) > MaxAbsTranslation
            || value.Scale.X <= 0 || value.Scale.Y <= 0 || value.Scale.Z <= 0
            || !IsFinite(value.Rotation)
            || MathF.Abs(value.Rotation.LengthSquared() - 1f) > QuaternionNormalizationTolerance)
        {
            throw new ArgumentException("Transform must have finite bounded translation, positive scale, and normalized rotation.");
        }
    }

    private static void ValidateCharacterMotion(in CharacterMotion value)
    {
        if (!IsBounded(value.ControlledVelocity, MaxAbsVelocity)
            || !IsBounded(value.ExternalVelocity, MaxAbsVelocity)
            || !IsTimer(value.JumpBufferRemaining) || !IsTimer(value.CoyoteRemaining)
            || !IsTimer(value.LandingLockoutRemaining)
            || !IsBounded(value.SupportLocalAnchor, MaxAbsTranslation)
            || !IsBounded(value.SupportPreviousTranslation, MaxAbsTranslation)
            || !IsNormalized(value.SupportPreviousRotation)
            || !IsBounded(value.SupportPointVelocity, MaxAbsVelocity)
            || !IsBounded(value.FallOriginY, MaxAbsTranslation)
            || !IsBounded(value.PeakY, MaxAbsTranslation))
        {
            throw new ArgumentException("Character motion does not satisfy its intrinsic Engine value bounds.");
        }
    }

    private static bool IsFinite(System.Numerics.Vector3 value)
        => float.IsFinite(value.X) && float.IsFinite(value.Y) && float.IsFinite(value.Z);

    private static bool IsFinite(System.Numerics.Quaternion value)
        => float.IsFinite(value.X) && float.IsFinite(value.Y) && float.IsFinite(value.Z) && float.IsFinite(value.W);

    private static bool IsBounded(System.Numerics.Vector3 value, float maximum)
        => IsFinite(value) && MathF.Abs(value.X) <= maximum && MathF.Abs(value.Y) <= maximum && MathF.Abs(value.Z) <= maximum;

    private static bool IsBounded(float value, float maximum) => float.IsFinite(value) && MathF.Abs(value) <= maximum;

    private static bool IsTimer(float value) => float.IsFinite(value) && value is >= 0 and <= MaxCharacterTimerSeconds;

    private static bool IsNormalized(System.Numerics.Quaternion value)
        => IsFinite(value) && MathF.Abs(value.LengthSquared() - 1f) <= QuaternionNormalizationTolerance;
}
