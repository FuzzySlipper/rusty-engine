using Rusty.Engine.Entities;
using Rusty.Engine.Mechanics;

const long BaseStrength = 40;
const long EquipmentBonus = 4;
const long BlessingBonus = 6;
const long ResourceStart = 30;
const long ResourceSpend = 10;
const double ContinuousBase = 0.5;
const double ContinuousBonus = 0.25;

ExerciseTypedValues();
ExerciseExactStatEvaluation();
ExerciseContinuousStatEvaluation();
ExerciseExactTrackAtomicity();
ExerciseEffectPolicies();

static void ExerciseTypedValues()
{
    StatId strength = StatId.Parse("strength");
    Require(!StatId.TryParse("Strength", out _), "invalid typed identity was admitted");
    Require(strength.Value == "strength", "typed identity lost its value");

    ExactRatio ratio = new(2, 4);
    Require(ratio.Numerator == 1 && ratio.Denominator == 2, "ratio was not normalized");
    Require(ratio.Apply(new ExactValue(11)).Raw == 5, "exact ratio did not round toward zero once");

    ContinuousValue negativeZero = ContinuousValue.FromBits(0x8000_0000_0000_0000);
    Require(negativeZero.Bits == 0, "continuous negative zero was not normalized");
    ExpectMechanicsError(
        () => ContinuousValue.FromBits(0x7ff0_0000_0000_0000),
        "non-finite continuous value was admitted");
}

static void ExerciseExactStatEvaluation()
{
    StatId strength = StatId.Parse("strength");
    StackingGroupId additions = StackingGroupId.Parse("strength-additions");
    StackingGroupId scale = StackingGroupId.Parse("strength-scale");
    ExactStatDefinition definition = new(strength, new ExactValue(0), new ExactValue(100));
    ExactSource equipment = new(
        new EquippedItemSourceIdentity(new EntityId(1), new EntityId(2), SourceDefinitionId.Parse("equipment")),
        SourceDefinitionId.Parse("equipment"),
        priority: 10,
        [new ExactStatContributionDefinition(
            strength,
            additions,
            MechanicsStackingPolicy.Sum,
            new ExactStatContribution.Add(new ExactValue(EquipmentBonus)))]);
    ExactSource blessing = new(
        new IntrinsicSourceIdentity(new EntityId(1), SourceInstanceId.Parse("blessing")),
        SourceDefinitionId.Parse("blessing"),
        priority: 20,
        [
            new ExactStatContributionDefinition(
                strength,
                additions,
                MechanicsStackingPolicy.Sum,
                new ExactStatContribution.Add(new ExactValue(BlessingBonus))),
            new ExactStatContributionDefinition(
                strength,
                scale,
                MechanicsStackingPolicy.Highest,
                new ExactStatContribution.Scale(new ExactRatio(3, 2))),
        ]);

    ExactStatEvaluation evaluation = ExactStatEvaluator.Evaluate(
        definition,
        new ExactValue(BaseStrength),
        [blessing, equipment]);
    Require(evaluation.Value.Raw == 75, "exact stat modifiers did not combine deterministically");
    Require(evaluation.Decisions[0].SourceDefinition.Value == "equipment",
        "source order did not use priority before identity");
    Require(evaluation.Decisions.All(decision => decision.Outcome == MechanicsDecisionOutcome.Applied),
        "applicable exact contributions were unexpectedly suppressed");

    ExpectMechanicsError(
        () => ExactStatEvaluator.Evaluate(definition, new ExactValue(BaseStrength), [equipment, equipment]),
        "duplicate source identity was silently accepted");
}

static void ExerciseContinuousStatEvaluation()
{
    StatId accuracy = StatId.Parse("accuracy");
    ContinuousStatDefinition definition = new(
        accuracy,
        new ContinuousValue(0.0),
        new ContinuousValue(2.0));
    ContinuousSource source = new(
        new RequestSourceIdentity(OperationId.Parse("aim"), SourceInstanceId.Parse("focus")),
        SourceDefinitionId.Parse("focus"),
        priority: 1,
        [new ContinuousStatContributionDefinition(
            accuracy,
            StackingGroupId.Parse("accuracy-additions"),
            MechanicsStackingPolicy.Sum,
            new ContinuousStatContribution.Add(new ContinuousValue(ContinuousBonus)))]);

    ContinuousStatEvaluation evaluation = ContinuousStatEvaluator.Evaluate(
        definition,
        new ContinuousValue(ContinuousBase),
        [source]);
    Require(evaluation.Value.Value == 0.75, "continuous stat addition was not retained");
}

static void ExerciseExactTrackAtomicity()
{
    ExactTrackDefinition definition = new(
        TrackId.Parse("resource"),
        new ExactValue(0),
        new ExactTrackMaximum.Fixed(new ExactValue(100)));
    ExactTrack track = new(definition, new ExactValue(ResourceStart));
    ExactTrackMutationReceipt spent = track.Spend(new ExactValue(ResourceSpend));
    Require(spent.After.Raw == 20 && spent.AppliedAmount.Raw == ResourceSpend,
        "exact track spend was not applied");

    ExactValue beforeRejectedSpend = track.Current;
    ExpectMechanicsError(
        () => track.Spend(new ExactValue(100)),
        "overspend was accepted");
    Require(track.Current == beforeRejectedSpend, "rejected spend changed track state");

    ExactTrackMutationReceipt restored = track.Restore(new ExactValue(100));
    Require(restored.After.Raw == 100 && restored.AppliedAmount.Raw == 80,
        "restore did not clamp to the maximum");
    ExactTrackReconciliationReceipt clamped = track.Reconcile(
        definition.ResolveBounds(new ExactValue(60)),
        ExactTrackReconciliationPolicy.ClampToMaximum);
    Require(clamped.Before.Raw == 100 && track.Current.Raw == 60,
        "track reconciliation did not clamp atomically");

    ExactTrackBounds beforeRejectedReconcile = track.Bounds;
    ExpectMechanicsError(
        () => track.Reconcile(
            definition.ResolveBounds(new ExactValue(40)),
            ExactTrackReconciliationPolicy.PreserveCurrent),
        "no-stranding preserve policy was ignored");
    Require(track.Current.Raw == 60 && track.Bounds == beforeRejectedReconcile,
        "rejected reconciliation changed track state");
}

static void ExerciseEffectPolicies()
{
    SourceDefinitionId auraSource = SourceDefinitionId.Parse("aura-source");
    EffectDefinition independent = new(
        EffectDefinitionId.Parse("ward"),
        StackingGroupId.Parse("wards"),
        EffectStackingPolicy.IndependentByProvenance,
        maximumInstances: 2,
        maximumStacks: 3,
        [auraSource]);
    EffectState independentState = new(new EntityId(1));
    independentState.Apply(
        independent,
        EffectInstanceId.Parse("ward-one"),
        new IntrinsicSourceIdentity(new EntityId(1), SourceInstanceId.Parse("item-one")),
        stacks: 2);
    EffectMutationReceipt second = independentState.Apply(
        independent,
        EffectInstanceId.Parse("ward-two"),
        new IntrinsicSourceIdentity(new EntityId(1), SourceInstanceId.Parse("item-two")),
        stacks: 1);
    Require(second.ActivatedSources.Count == 1, "effect source activation count was incorrect");
    ExpectMechanicsError(
        () => independentState.Apply(
            independent,
            EffectInstanceId.Parse("ward-three"),
            new IntrinsicSourceIdentity(new EntityId(1), SourceInstanceId.Parse("item-one")),
            stacks: 1),
        "independent provenance conflict was ignored");

    EffectDefinition refresh = new(
        EffectDefinitionId.Parse("focus"),
        StackingGroupId.Parse("focus"),
        EffectStackingPolicy.Refresh,
        maximumInstances: 0,
        maximumStacks: 3);
    EffectState refreshState = new();
    refreshState.Apply(
        refresh,
        EffectInstanceId.Parse("focus-instance"),
        new RequestSourceIdentity(OperationId.Parse("cast-one"), SourceInstanceId.Parse("cast")),
        stacks: 1);
    EffectMutationReceipt refreshed = refreshState.Refresh(
        EffectInstanceId.Parse("focus-instance"),
        new RequestSourceIdentity(OperationId.Parse("cast-two"), SourceInstanceId.Parse("cast")),
        stacks: 3);
    Require(refreshed.Kind == EffectMutationKind.Refresh
        && refreshState.Effects.Single().Stacks == 3,
        "refresh did not replace the existing activation");

    EffectDefinition replace = new(
        EffectDefinitionId.Parse("stance"),
        StackingGroupId.Parse("stances"),
        EffectStackingPolicy.Replace,
        maximumInstances: 0,
        maximumStacks: 1);
    EffectState replaceState = new();
    replaceState.Apply(
        replace,
        EffectInstanceId.Parse("stance-old"),
        new RequestSourceIdentity(OperationId.Parse("stance"), SourceInstanceId.Parse("old")),
        stacks: 1);
    EffectMutationReceipt replaced = replaceState.Replace(
        replace,
        EffectInstanceId.Parse("stance-new"),
        new RequestSourceIdentity(OperationId.Parse("stance"), SourceInstanceId.Parse("new")),
        stacks: 1);
    Require(replaced.Removed.Count == 1 && replaceState.Effects.Single().Instance.Value == "stance-new",
        "replace did not remove the prior group member");
    EffectMutationReceipt expired = replaceState.Expire(EffectInstanceId.Parse("stance-new"));
    Require(expired.Kind == EffectMutationKind.Expire && replaceState.Effects.Count == 0,
        "explicit effect expiry was not caller-driven");
}

static void ExpectMechanicsError(Action action, string message)
{
    try
    {
        action();
    }
    catch (Exception exception) when (
        exception is MechanicsException or MechanicsArithmeticException or ArgumentException)
    {
        return;
    }

    throw new InvalidOperationException(message);
}

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}
