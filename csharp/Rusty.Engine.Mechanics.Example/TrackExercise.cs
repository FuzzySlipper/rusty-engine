using System.Runtime.CompilerServices;
using Rusty.Engine.Mechanics;

internal static class TrackExercise
{
    public static void Run()
    {
        var maximum = new Stat(100, minimum: 0);
        var health = new Track(maximum, current: 70);
        var progression = new Track(maximum, current: 70,
            maximumChangePolicy: TrackMaximumChangePolicy.PreserveMissingAmount);
        Check(ReferenceEquals(maximum, health.Maximum) && ReferenceEquals(maximum, progression.Maximum), "maximum reference identity");
        maximum.BaseValue = 120;
        Equal(70, health.Current, "default growth preserves current");
        Equal(90, progression.Current, "growth preserves missing amount");
        health.SetCurrent(90);
        progression.SetCurrent(90);
        maximum.BaseValue = 100;
        health.SetCurrent(90);
        progression.SetCurrent(90);
        maximum.BaseValue = 80;
        Equal(80, health.Current, "default shrink clamps current");
        Equal(70, progression.Current, "shrink preserves missing amount");
        var bonus = maximum.AddModifier(20);
        Equal(80, health.Current, "modifier growth does not heal ordinary health");
        Equal(90, progression.Current, "modifier growth reconciles dependent track");
        maximum.RemoveModifier(bonus);
        Equal(70, progression.Current, "modifier removal reconciles before returning");

        var resource = new Track(100, current: 30);
        Check(!resource.TrySpend(31), "overspend rejected");
        Equal(30, resource.Current, "overspend leaves state unchanged");
        Reject(() => resource.Spend(31));
        Reject(() => resource.Spend(-1));
        Reject(() => resource.Restore(-1));
        Reject(() => resource.SetCurrent(double.NaN));
        Reject(() => resource.SetCurrent(101));
        Equal(30, resource.Current, "invalid direct mutations leave state unchanged");
        Equal(10, resource.Spend(10), "spend applied amount");
        Equal(80, resource.Restore(200), "restore saturates and reports applied amount");
        Equal(100, resource.Current, "restore reaches maximum");
        resource.SetCurrent(500, clamp: true);
        Equal(100, resource.Value, "explicit clamped assignment");
        resource.Minimum = 90;
        Reject(() => resource.Minimum = 101);
        Equal(90, resource.Minimum, "invalid minimum unchanged");
        Reject(() => resource.Maximum.BaseValue = 80);
        Equal(100, resource.MaximumValue, "invalid maximum unchanged");
        var fractions = new Track(1, current: 0.75);
        fractions.Spend(0.125);
        Equal(0.625, fractions.Current, "fractional spending");
        var whole = new Track(100, current: 2.9, quantum: 1,
            rounding: MidpointRounding.ToZero, integerRounding: MidpointRounding.ToZero);
        Check(whole.ValueInt == 2 && whole.ValueInt64 == 2, "explicit whole-number track policy");
        var endpoint = new Track(0.4, quantum: 1);
        Equal(0.4, endpoint.Current, "default current equals off-grid maximum");
        endpoint.Spend(0);
        Equal(0.4, endpoint.Current, "zero spend preserves endpoint");
        endpoint.Maximum.BaseValue = 2;
        Equal(0.4, endpoint.Current, "maximum growth preserves off-grid current");
        endpoint.Restore(0);
        Equal(0.4, endpoint.Current, "zero restore preserves off-grid current");

        var roundedUp = new Track(0.4, quantum: 1, rounding: MidpointRounding.ToPositiveInfinity);
        roundedUp.Maximum.BaseValue = 2;
        roundedUp.Spend(0.1);
        Equal(0.4, roundedUp.Current, "spending cannot heal an off-grid value");
        var roundedDown = new Track(0.4, quantum: 1, rounding: MidpointRounding.ToNegativeInfinity);
        roundedDown.Maximum.BaseValue = 2;
        roundedDown.Restore(0.1);
        Equal(0.4, roundedDown.Current, "restoring cannot damage an off-grid value");

        SharedFailureIsolation();
        DetachedPlanning();
        AbandonedDependency();
    }

    private static void SharedFailureIsolation()
    {
        var maximum = new Stat(100);
        var first = new Track(maximum, current: 90);
        var second = new Track(maximum, current: 70, minimum: 50);
        Reject(() => maximum.BaseValue = 40);
        Equal(100, maximum.BaseValue, "failed stat base is not published");
        Equal(90, first.Current, "first dependent not changed before second rejects");
        Equal(70, second.Current, "rejecting dependent unchanged");
        Reject(() => maximum.SetBounds(0, 40));
        Equal(double.MaxValue, maximum.Maximum, "failed stat bounds not published");
        Reject(() => maximum.AddModifier(-70));
        Equal(100, maximum.Value, "failed modifier not admitted");
        Reject(() => maximum.SetQuantization(1000));
        Equal(0, maximum.Quantum, "failed quantization not published");
        var statId = StatId.Parse("maximum");
        var source = new StatSource(new IntrinsicSourceIdentity(null, SourceInstanceId.Parse("penalty")),
            SourceDefinitionId.Parse("penalty"), 0,
            [new(statId, StackingGroupId.Parse("penalty"), MechanicsStackingPolicy.Sum, new StatContribution.Add(-70))]);
        Reject(() => maximum.SetSources(statId, [source]));
        Equal(100, maximum.Value, "failed authored sources not admitted");
        maximum.BaseValue = 60;
        Equal(60, first.Current, "valid update still reconciles all tracks");
        Equal(60, second.Current, "valid update reconciles second track");

        var supported = new Stat(40);
        var modifier = supported.AddModifier(60);
        var dependent = new Track(supported, minimum: 50);
        Reject(() => supported.RemoveModifier(modifier));
        Equal(100, supported.Value, "failed removal retains modifier");
        dependent.Minimum = 0;
        Check(supported.RemoveModifier(modifier), "modifier remains removable after rejection");
        Equal(40, dependent.Current, "successful retry is a new direct operation");

        var hugeMaximum = new Stat(double.MaxValue);
        var smallCurrent = new Track(hugeMaximum, current: 1,
            maximumChangePolicy: TrackMaximumChangePolicy.PreserveMissingAmount);
        hugeMaximum.BaseValue = double.MaxValue;
        Equal(1, smallCurrent.Current, "unchanged maximum cannot erase small current by cancellation");
        GC.KeepAlive(second);
    }

    private static void DetachedPlanning()
    {
        var maximum = new Stat(100);
        var live = new Track(maximum, current: 70);
        var copy = new Track(maximum.Copy(), live.Current);
        copy.Maximum.BaseValue = 120;
        copy.Spend(10);
        Equal(100, maximum.Value, "copy has independent maximum");
        Equal(70, live.Current, "detached action cannot mutate live track");
        var corrected = new Stat(double.MaxValue);
        corrected.AddModifier(-double.MaxValue);
        corrected.Quantum = double.MaxValue * 0.6;
        Equal(0, corrected.Copy().Value, "copy does not evaluate a temporarily incomplete modifier set");
    }

    private static void AbandonedDependency()
    {
        var maximum = new Stat(100);
        WeakReference<Track> abandoned = AddAbandonedTrack(maximum);
        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        Check(!abandoned.TryGetTarget(out _), "maximum must not retain an abandoned track");
        maximum.BaseValue = 20;
        Equal(20, maximum.Value, "abandoned minimum no longer restricts stat");
    }

    [MethodImpl(MethodImplOptions.NoInlining)]
    private static WeakReference<Track> AddAbandonedTrack(Stat maximum) => new(new Track(maximum, minimum: 90));
    private static void Equal(double expected, double actual, string message) => Check(expected == actual, message);
    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
    private static void Reject(Action operation)
    {
        try { operation(); }
        catch (ArgumentException) { return; }
        catch (MechanicsException) { return; }
        catch (OverflowException) { return; }
        throw new InvalidOperationException("Invalid track operation was accepted.");
    }
}
