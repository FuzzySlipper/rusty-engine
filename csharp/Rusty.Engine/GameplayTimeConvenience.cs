namespace Rusty.Engine;

/// <summary>
/// Gameplay time for a realtime product: hold the world, run it in slow
/// motion or at realtime, or advance it a bounded amount and hold.
/// </summary>
/// <remarks>
/// The Engine admits simulation steps; a request applies from the next host
/// observation, after the callback that made it returns. Once a product makes
/// any request, every observation delivers an <c>Update</c>, with
/// <see cref="ProductUpdateFacts.AdmittedStepCount"/> zero while no step is
/// due, so the product can read input, aim and present while the world holds.
/// Requests made later in one callback replace earlier ones.
/// </remarks>
public static class GameplayTimeConvenience
{
    /// <summary>Realtime simulation speed.</summary>
    public const double RealtimeRate = 1.0;

    /// <summary>Simulation at <paramref name="rate"/> of realtime, from 0 (held) to 1.</summary>
    public static GameplayTimeReadout SetRate(this IGameplayTimeService time, double rate)
    {
        ArgumentNullException.ThrowIfNull(time);
        return time.SelectRate(new GameplayTimeRateRequest(rate));
    }

    /// <summary>Stops simulation; the product still updates every observation.</summary>
    public static GameplayTimeReadout Hold(this IGameplayTimeService time) => time.SetRate(0.0);

    /// <summary>Runs simulation at realtime.</summary>
    public static GameplayTimeReadout RunRealtime(this IGameplayTimeService time) => time.SetRate(RealtimeRate);

    /// <summary>
    /// Runs simulation for <paramref name="seconds"/> of world time, rounded up
    /// to whole fixed steps, at <paramref name="rate"/>, then holds it. The
    /// readout's <see cref="GameplayTimeReadout.AdvanceRemainingSteps"/> is the
    /// number of steps the advance will admit.
    /// </summary>
    public static GameplayTimeReadout Advance(this IGameplayTimeService time, double seconds, double rate = RealtimeRate)
    {
        ArgumentNullException.ThrowIfNull(time);
        return time.Advance(new GameplayTimeAdvanceRequest(seconds, rate));
    }
}

public readonly partial record struct GameplayTimeReadout
{
    /// <summary>Whether the world admits no simulation now.</summary>
    public bool Held => Rate == 0.0;
}

public readonly partial record struct ProductUpdateFacts
{
    /// <summary>Facts for a product that has not selected gameplay time.</summary>
    public ProductUpdateFacts(
        ProductUpdateMode mode,
        ProductLifecycleState lifecycleState,
        ulong generation,
        ulong controlRevision,
        ulong observedHostTimeNanoseconds,
        ulong simulationStep,
        uint fixedStepHz,
        uint admittedStepCount,
        ulong droppedStepCount,
        double fixedDeltaSeconds)
        : this(mode, lifecycleState, generation, controlRevision, observedHostTimeNanoseconds, simulationStep,
            fixedStepHz, admittedStepCount, droppedStepCount, fixedDeltaSeconds, false,
            GameplayTimeConvenience.RealtimeRate, 0)
    {
    }
}
