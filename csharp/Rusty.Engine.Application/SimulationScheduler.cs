using Rusty.Engine;

namespace Rusty.Engine.Application;

/// <summary>
/// Schedules managed product work against Engine-admitted simulation steps.
/// </summary>
/// <remarks>
/// This optional helper has no clock and does not retain scheduler state in an
/// Engine persistence format. Call <see cref="Advance"/> once from the
/// product's admitted update path, or use <see cref="Attach"/> with an
/// <see cref="UpdatePipeline"/>. A non-running lifecycle update performs no
/// work. A new Engine lifecycle generation cancels all pending work so a
/// product restart cannot execute callbacks retained from the prior run.
/// </remarks>
public sealed class SimulationScheduler
{
    private const ulong NoRepeat = 0;
    private const ulong NextAdmittedStepOffset = 1;

    private readonly Dictionary<ulong, ScheduledWork> _pending = [];
    private ulong _nextHandle = NextAdmittedStepOffset;
    private ulong _nextInsertionOrder = NextAdmittedStepOffset;
    private ulong _currentStep;
    private ulong _generation;
    private uint _completedCount;
    private uint _cancelledCount;
    private bool _hasCurrentStep;
    private bool _hasGeneration;

    /// <summary>Registers this scheduler in one optional product update phase.</summary>
    public void Attach(UpdatePipeline pipeline, UpdatePhase phase)
    {
        ArgumentNullException.ThrowIfNull(pipeline);
        pipeline.Register(phase, (_, update) => Advance(update));
    }

    /// <summary>Schedules one callback at an absolute Engine simulation step.</summary>
    public ScheduledWorkHandle ScheduleAt(ulong simulationStep, ScheduledWorkCallback callback)
    {
        return Add(simulationStep, NoRepeat, callback);
    }

    /// <summary>
    /// Schedules one callback after the given number of complete admitted
    /// steps. Zero means the next admitted step.
    /// </summary>
    public ScheduledWorkHandle ScheduleAfter(ulong delaySteps, ScheduledWorkCallback callback)
    {
        return Add(NextDueStep(delaySteps), NoRepeat, callback);
    }

    /// <summary>Schedules repeating work at an absolute Engine simulation step.</summary>
    public ScheduledWorkHandle ScheduleRepeatingAt(
        ulong firstSimulationStep,
        ulong repeatEverySteps,
        ScheduledWorkCallback callback)
    {
        ValidateRepeat(repeatEverySteps);
        return Add(firstSimulationStep, repeatEverySteps, callback);
    }

    /// <summary>
    /// Schedules repeating work after the given number of complete admitted
    /// steps. Zero means the next admitted step.
    /// </summary>
    public ScheduledWorkHandle ScheduleRepeatingAfter(
        ulong delaySteps,
        ulong repeatEverySteps,
        ScheduledWorkCallback callback)
    {
        ValidateRepeat(repeatEverySteps);
        return Add(NextDueStep(delaySteps), repeatEverySteps, callback);
    }

    /// <summary>Cancels pending work. Completed or unknown handles return false.</summary>
    public bool Cancel(ScheduledWorkHandle handle)
    {
        if (!_pending.Remove(handle.Id))
        {
            return false;
        }

        Increment(ref _cancelledCount);
        return true;
    }

    /// <summary>
    /// Moves pending work after the requested number of complete admitted
    /// steps. Zero means the next admitted step.
    /// </summary>
    public bool RescheduleAfter(ScheduledWorkHandle handle, ulong delaySteps)
    {
        return TryUpdate(handle, NextDueStep(delaySteps), null, null);
    }

    /// <summary>Moves pending work to an absolute Engine simulation step.</summary>
    public bool RescheduleAt(ScheduledWorkHandle handle, ulong simulationStep)
    {
        return TryUpdate(handle, simulationStep, null, null);
    }

    /// <summary>
    /// Replaces pending work while retaining its opaque handle. A repeat value
    /// of zero makes the replacement one-shot.
    /// </summary>
    public bool ReplaceAfter(
        ScheduledWorkHandle handle,
        ulong delaySteps,
        ulong repeatEverySteps,
        ScheduledWorkCallback callback)
    {
        ValidateCallback(callback);
        return TryUpdate(handle, NextDueStep(delaySteps), repeatEverySteps, callback);
    }

    /// <summary>Returns bounded scheduler counts without retaining callback history.</summary>
    public SchedulerReadout Readout => new((uint)_pending.Count, _completedCount, _cancelledCount);

    /// <summary>
    /// Consumes one Engine-admitted product update. Every admitted batch is
    /// expanded into its individual simulation steps. Realtime supplies its
    /// host-timing facts alongside those steps; demand and external updates
    /// may also admit simulation steps without those realtime-only facts.
    /// </summary>
    public void Advance(ProductUpdate update)
    {
        ProductUpdateFacts facts = update.Facts;
        ObserveGeneration(facts.Generation);
        if (facts.LifecycleState != ProductLifecycleState.Running)
        {
            return;
        }

        for (uint offset = 0; offset < facts.AdmittedStepCount; offset++)
        {
            _currentStep = checked(facts.SimulationStep + offset);
            _hasCurrentStep = true;
            DispatchStep(_currentStep, facts);
        }
    }

    private ScheduledWorkHandle Add(ulong dueStep, ulong repeatEverySteps, ScheduledWorkCallback callback)
    {
        ValidateCallback(callback);
        ulong id = _nextHandle++;
        _pending.Add(id, new ScheduledWork(id, dueStep, repeatEverySteps, callback, _nextInsertionOrder++));
        return new ScheduledWorkHandle(id);
    }

    private ulong NextDueStep(ulong delaySteps)
    {
        if (!_hasCurrentStep)
        {
            throw new InvalidOperationException(
                "Relative scheduling needs an Engine-admitted simulation step. Use an absolute scheduling method before the first update.");
        }

        return checked(_currentStep + delaySteps + NextAdmittedStepOffset);
    }

    private bool TryUpdate(
        ScheduledWorkHandle handle,
        ulong dueStep,
        ulong? repeatEverySteps,
        ScheduledWorkCallback? callback)
    {
        if (!_pending.TryGetValue(handle.Id, out ScheduledWork? work))
        {
            return false;
        }

        work.DueStep = dueStep;
        if (repeatEverySteps.HasValue)
        {
            work.RepeatEverySteps = repeatEverySteps.Value;
        }

        if (callback is not null)
        {
            work.Callback = callback;
        }

        work.Revision++;
        return true;
    }

    private void ObserveGeneration(ulong generation)
    {
        if (!_hasGeneration)
        {
            _generation = generation;
            _hasGeneration = true;
            return;
        }

        if (_generation == generation)
        {
            return;
        }

        _generation = generation;
        _hasCurrentStep = false;
        uint cancelled = (uint)_pending.Count;
        _pending.Clear();
        SaturatingAdd(ref _cancelledCount, cancelled);
    }

    private void DispatchStep(ulong simulationStep, ProductUpdateFacts facts)
    {
        ScheduledWork[] due = _pending.Values
            .Where(work => work.DueStep <= simulationStep)
            .OrderBy(work => work.InsertionOrder)
            .ToArray();

        foreach (ScheduledWork work in due)
        {
            if (!_pending.ContainsKey(work.Id))
            {
                continue;
            }

            ulong revision = work.Revision;
            work.Callback(new ScheduledWorkContext(facts, simulationStep));
            if (!_pending.TryGetValue(work.Id, out ScheduledWork? current) || current.Revision != revision)
            {
                continue;
            }

            if (current.RepeatEverySteps == NoRepeat)
            {
                _pending.Remove(current.Id);
                Increment(ref _completedCount);
                continue;
            }

            current.DueStep = checked(simulationStep + current.RepeatEverySteps);
            current.Revision++;
        }
    }

    private static void ValidateCallback(ScheduledWorkCallback callback)
    {
        ArgumentNullException.ThrowIfNull(callback);
    }

    private static void ValidateRepeat(ulong repeatEverySteps)
    {
        if (repeatEverySteps == NoRepeat)
        {
            throw new ArgumentOutOfRangeException(nameof(repeatEverySteps), "Repeating work needs at least one admitted simulation step between callbacks.");
        }
    }

    private static void Increment(ref uint count)
    {
        SaturatingAdd(ref count, NextAdmittedStepOffset);
    }

    private static void SaturatingAdd(ref uint count, ulong amount)
    {
        count = amount >= uint.MaxValue - count ? uint.MaxValue : count + (uint)amount;
    }

    private sealed class ScheduledWork
    {
        public ScheduledWork(ulong id, ulong dueStep, ulong repeatEverySteps, ScheduledWorkCallback callback, ulong insertionOrder)
        {
            Id = id;
            DueStep = dueStep;
            RepeatEverySteps = repeatEverySteps;
            Callback = callback;
            InsertionOrder = insertionOrder;
        }

        public ulong Id { get; }
        public ulong DueStep { get; set; }
        public ulong RepeatEverySteps { get; set; }
        public ScheduledWorkCallback Callback { get; set; }
        public ulong InsertionOrder { get; }
        public ulong Revision { get; set; }
    }
}

/// <summary>Opaque identity for managed scheduler work.</summary>
public readonly struct ScheduledWorkHandle : IEquatable<ScheduledWorkHandle>
{
    internal ScheduledWorkHandle(ulong id) => Id = id;

    internal ulong Id { get; }

    public bool Equals(ScheduledWorkHandle other) => Id == other.Id;

    public override bool Equals(object? obj) => obj is ScheduledWorkHandle other && Equals(other);

    public override int GetHashCode() => Id.GetHashCode();

    public static bool operator ==(ScheduledWorkHandle left, ScheduledWorkHandle right) => left.Equals(right);

    public static bool operator !=(ScheduledWorkHandle left, ScheduledWorkHandle right) => !left.Equals(right);
}

/// <summary>Managed callback invoked from one exact Engine-admitted simulation step.</summary>
public delegate void ScheduledWorkCallback(ScheduledWorkContext context);

/// <summary>
/// The original Engine facts for an update plus the exact step being dispatched
/// from that update's admitted batch. This does not provide a managed clock.
/// </summary>
public readonly record struct ScheduledWorkContext(ProductUpdateFacts Facts, ulong SimulationStep);

/// <summary>Bounded managed scheduler state; callbacks themselves are never retained as history.</summary>
public readonly record struct SchedulerReadout(uint Pending, uint Completed, uint Cancelled);
