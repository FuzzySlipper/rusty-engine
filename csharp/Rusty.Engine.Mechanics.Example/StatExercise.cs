using Rusty.Engine.Mechanics;

internal static class StatExercise
{
    public static void Run()
    {
        var health = new Stat(100, minimum: 0);
        var gear = health.AddModifier(20);
        var multiplier = health.AddModifier(1.5, StatModifierKind.Multiply);
        Equal(180, health.Value, "addition precedes multiplication");
        health.BaseValue = 80;
        Equal(150, health.Value, "base changes update the same stat");
        Check(health.RemoveModifier(gear), "remove gear");
        Equal(120, health.Value, "gear removed");
        Check(!health.RemoveModifier(gear), "removed handle cannot remove again");
        Check(!new Stat(1).RemoveModifier(multiplier), "handles belong to one stat");
        health.SetBounds(0, 110);
        Equal(110, health.Value, "resolved maximum clamps");

        var number = new Stat(2.5);
        Check(number.ValueInt == 3 && number.ValueInt64 == 3, "default positive tie");
        number.BaseValue = -2.5;
        Check(number.ValueInt == -3 && number.ValueInt64 == -3, "default negative tie");
        number.IntegerRounding = MidpointRounding.ToZero;
        Check(number.ValueInt == -2 && number.Value == -2.5, "integer conversion does not quantize value");
        number.BaseValue = 2.9;
        Check(number.ValueInt == 2, "legacy toward-zero conversion");
        number.IntegerRounding = MidpointRounding.ToEven;
        number.BaseValue = 2.5;
        Check(number.ValueInt == 2, "explicit midpoint-to-even");
        number.IntegerRounding = MidpointRounding.ToNegativeInfinity;
        number.BaseValue = -2.1;
        Check(number.ValueInt == -3, "explicit floor");

        var precise = new Stat(0.375);
        Equal(0.375, precise.Value, "fractions retained by default");
        Check(precise.ValueFloat == 0.375f, "float accessor");
        precise.SetQuantization(0.25);
        Equal(0.5, precise.Value, "positive quantum tie");
        precise.BaseValue = -0.375;
        Equal(-0.5, precise.Value, "negative quantum tie");
        precise.SetBounds(-0.4, 0.4);
        Equal(-0.4, precise.Value, "lower off-grid endpoint wins");
        precise.BaseValue = 0.375;
        Equal(0.4, precise.Value, "upper off-grid endpoint wins");
        var large = new Stat(double.MaxValue, quantum: double.Epsilon);
        Equal(double.MaxValue, large.Value, "tiny quantum does not overflow large valid value");
        Equal(double.MaxValue, new Stat(double.Epsilon, quantum: double.MaxValue,
            rounding: MidpointRounding.ToPositiveInfinity).Value, "directed rounding survives division underflow");
        Equal(2_000_000_000_000, new Stat(2_000_000_000_000).Value, "no arbitrary exact-value ceiling");

        var intact = new Stat(10, minimum: 0, maximum: 20);
        Reject(() => intact.BaseValue = double.NaN);
        Reject(() => intact.SetBounds(20, 10));
        Reject(() => intact.Maximum = double.PositiveInfinity);
        Reject(() => intact.Quantum = -1);
        Reject(() => intact.Rounding = (MidpointRounding)99);
        Reject(() => intact.IntegerRounding = (MidpointRounding)99);
        Reject(() => intact.AddModifier(double.PositiveInfinity));
        Reject(() => intact.AddModifier(1, (StatModifierKind)99));
        Reject(() => intact.AddModifier(double.MaxValue, StatModifierKind.Multiply));
        Reject(() => intact.AddModifier(-1, StatModifierKind.Maximum));
        Equal(10, intact.BaseValue, "rejected base preserved");
        Equal(10, intact.Value, "rejected changes preserve evaluated state");
        Check(intact.Maximum == 20 && intact.Quantum == 0, "rejected options preserved");
        var overflowOnRemoval = new Stat(double.MaxValue);
        var offset = overflowOnRemoval.AddModifier(-double.MaxValue);
        overflowOnRemoval.AddModifier(double.MaxValue);
        Reject(() => overflowOnRemoval.RemoveModifier(offset));
        Equal(double.MaxValue, overflowOnRemoval.Value, "failed removal preserves modifier and value");
        Reject(() => _ = new Stat((double)int.MaxValue + 1).ValueInt);
        Reject(() => _ = new Stat(9223372036854775808d).ValueInt64);
        Reject(() => _ = new Stat(double.MaxValue).ValueFloat);
        Check(new Stat(long.MinValue).ValueInt64 == long.MinValue, "valid Int64 endpoint");
        Check(new Stat(int.MaxValue).ValueInt == int.MaxValue, "valid Int32 endpoint");

        AuthoredSources();
    }

    private static void AuthoredSources()
    {
        StatId ability = StatId.Parse("ability");
        StackingGroupId group = StackingGroupId.Parse("gear");
        StatSource Source(string instance, string definition, double bonus, short priority = 0,
            MechanicsStackingPolicy policy = MechanicsStackingPolicy.Highest) => new(
                new IntrinsicSourceIdentity(null, SourceInstanceId.Parse(instance)),
                SourceDefinitionId.Parse(definition), priority,
                [new(ability, group, policy, new StatContribution.Add(bonus))]);
        var first = Source("first", "gear-a", 4, 10);
        var second = Source("second", "gear-b", 6, 20);
        var stat = new Stat(40, minimum: 0, maximum: 100, integerRounding: MidpointRounding.ToZero);
        stat.SetSources(ability, [second, first]);
        Equal(46, stat.Value, "highest authored contribution selected");
        var explanation = stat.Explain();
        Check(explanation.Decisions[0].Source == first.Identity
            && explanation.Decisions[0].Outcome == MechanicsDecisionOutcome.Suppressed,
            "explanation ordered by priority, with suppression");
        Check(stat.RemoveSource(second.Identity), "source removal");
        Equal(44, stat.Value, "source removal reveals suppressed gear");
        Equal(46, explanation.Value, "prior explanation is stable");
        var multiply = new StatSource(new IntrinsicSourceIdentity(null, SourceInstanceId.Parse("multiply")),
            SourceDefinitionId.Parse("multiply"), 0,
            [new(ability, StackingGroupId.Parse("multiply"), MechanicsStackingPolicy.Sum, new StatContribution.Multiply(1.5))]);
        stat.SetSources(ability, [multiply, first]);
        Equal(66, stat.Value, "authored addition before multiplier regardless of priority");
        stat.SetSources(ability, [Source("first", "same", 4, 10, MechanicsStackingPolicy.UniqueByDefinition),
            Source("second", "same", 6, 20, MechanicsStackingPolicy.UniqueByDefinition)]);
        Equal(44, stat.Value, "unique means source definition, not activation identity");
        Reject(() => stat.SetSources(ability, [first, Source("first", "other", 6, 20)]));
        Equal(44, stat.Value, "duplicate identity across priorities rejected without mutation");
        Reject(() => stat.SetSources(ability, [first, Source("second", "other", 6, 20, MechanicsStackingPolicy.Sum)]));
        Equal(44, stat.Value, "invalid stacking group rejected without mutation");

        var speed = new Stat(1, minimum: 0, maximum: 1);
        var speedId = StatId.Parse("speed");
        StatSource Slow(string id, double factor) => new(
            new IntrinsicSourceIdentity(null, SourceInstanceId.Parse(id)), SourceDefinitionId.Parse(id), 0,
            [new(speedId, StackingGroupId.Parse("slow"), MechanicsStackingPolicy.Lowest, new StatContribution.Maximum(factor))]);
        speed.SetSources(speedId, [Slow("slow-a", 0.75), Slow("slow-b", 0.4)]);
        Equal(0.4, speed.Value, "Rifles strongest fractional slow");
    }

    private static void Equal(double expected, double actual, string message) => Check(expected == actual, message);
    private static void Check(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
    private static void Reject(Action operation)
    {
        try { operation(); }
        catch (ArgumentException) { return; }
        catch (OverflowException) { return; }
        catch (MechanicsException) { return; }
        throw new InvalidOperationException("Invalid stat change was accepted.");
    }
}
