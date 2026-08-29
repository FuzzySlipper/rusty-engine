using System;

namespace Rusty.Engine.Resolution;

/// <summary>
/// Session-wide structural ceilings. These bound bookkeeping without prescribing how a product
/// evaluates an encounter, action, rule, or other domain concept.
/// </summary>
public readonly record struct ResolutionLimits(
    int MaxEvidence,
    int MaxWork,
    int MaxEffects,
    int MaxEvents,
    int MaxChildResolutions,
    int MaxChildDepth)
{
    public const int DefaultMaxEvidence = 256;
    public const int DefaultMaxWork = 4_096;
    public const int DefaultMaxEffects = 4_096;
    public const int DefaultMaxEvents = 4_096;
    public const int DefaultMaxChildResolutions = 1_024;
    public const int DefaultMaxChildDepth = 32;

    public static ResolutionLimits Default => new(
        DefaultMaxEvidence,
        DefaultMaxWork,
        DefaultMaxEffects,
        DefaultMaxEvents,
        DefaultMaxChildResolutions,
        DefaultMaxChildDepth);

    internal void Validate()
    {
        ValidatePositive(MaxEvidence, nameof(MaxEvidence));
        ValidatePositive(MaxWork, nameof(MaxWork));
        ValidatePositive(MaxEffects, nameof(MaxEffects));
        ValidatePositive(MaxEvents, nameof(MaxEvents));
        ValidatePositive(MaxChildResolutions, nameof(MaxChildResolutions));
        ValidatePositive(MaxChildDepth, nameof(MaxChildDepth));
    }

    private static void ValidatePositive(int value, string name)
    {
        if (value <= 0)
        {
            throw new ArgumentOutOfRangeException(name, value, "A resolution limit must be positive.");
        }
    }
}

/// <summary>
/// Per-attempt ceilings. Products can choose a smaller budget for a child without exposing a
/// generic program or operation model.
/// </summary>
public readonly record struct ResolutionBudget(
    int MaxEvidence,
    int MaxWork,
    int MaxEffects,
    int MaxEvents,
    int MaxChildren)
{
    public static ResolutionBudget From(ResolutionLimits limits) => new(
        limits.MaxEvidence,
        limits.MaxWork,
        limits.MaxEffects,
        limits.MaxEvents,
        limits.MaxChildResolutions);

    internal void Validate(ResolutionLimits limits)
    {
        limits.Validate();
        ValidateBounded(MaxEvidence, limits.MaxEvidence, nameof(MaxEvidence));
        ValidateBounded(MaxWork, limits.MaxWork, nameof(MaxWork));
        ValidateBounded(MaxEffects, limits.MaxEffects, nameof(MaxEffects));
        ValidateBounded(MaxEvents, limits.MaxEvents, nameof(MaxEvents));
        ValidateBounded(MaxChildren, limits.MaxChildResolutions, nameof(MaxChildren));
    }

    private static void ValidateBounded(int value, int maximum, string name)
    {
        ValidatePositive(value, name);
        if (value > maximum)
        {
            throw new ArgumentOutOfRangeException(name, value, $"{name} cannot exceed the session limit {maximum}.");
        }
    }

    private static void ValidatePositive(int value, string name)
    {
        if (value <= 0)
        {
            throw new ArgumentOutOfRangeException(name, value, "A resolution budget must be positive.");
        }
    }
}
