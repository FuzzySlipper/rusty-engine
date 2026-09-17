using System.Globalization;
using Rusty.Engine.Mechanics;

namespace Rusty.Engine.Debugging;

public sealed partial class EntityStoreDebugModule
{
    /// <summary>Explicitly registers bounded live Stats, Effects, Inventory and Equipment projections.
    /// Call once on this module; it neither discovers nor registers stores or components.</summary>
    public void RegisterMechanicsProjections(int maximumEntries = 16)
    {
        if (maximumEntries is < 1 or > MaximumPageSize)
            throw new ArgumentOutOfRangeException(nameof(maximumEntries));
        // Validate the entire registration before changing this module.
        Type[] types = [typeof(StatsComponent), typeof(EffectsComponent), typeof(InventoryComponent), typeof(EquipmentComponent)];
        if (types.Any(_typedProjections.ContainsKey))
            throw new InvalidOperationException("A mechanics component projection is already registered.");
        RegisterProjection<StatsComponent>((in StatsComponent value) => FormatStats(value, maximumEntries));
        RegisterProjection<EffectsComponent>((in EffectsComponent value) => FormatEffects(value, maximumEntries));
        RegisterProjection<InventoryComponent>((in InventoryComponent value) => FormatInventory(value, maximumEntries));
        RegisterProjection<EquipmentComponent>((in EquipmentComponent value) => FormatEquipment(value, maximumEntries));
    }

    private static string Number(double value) => value.ToString("R", CultureInfo.InvariantCulture);

    private static string FormatStats(StatsComponent value, int limit)
    {
        var output = new DebugOutput();
        output.Append($"stats={value.Stats.Count};tracks={value.Tracks.Count}");
        foreach (var (id, stat) in value.Stats.Take(limit))
            output.Append($"stat={id.Value}:value={Number(stat.Value)}:base={Number(stat.BaseValue)}");
        foreach (var (id, track) in value.Tracks.Take(limit))
            output.Append($"track={id.Value}:current={Number(track.Current)}:minimum={Number(track.Minimum)}:maximum={Number(track.MaximumValue)}");
        if (value.Stats.Count > limit || value.Tracks.Count > limit) output.Append("entries-truncated=true");
        return output.ToString();
    }

    private static string FormatEffects(EffectsComponent value, int limit)
    {
        var effects = value.Effects;
        var output = new DebugOutput();
        output.Append($"effects={effects.Count}");
        foreach (var effect in effects.Take(limit))
            output.Append($"effect={effect.Instance.Value}:definition={effect.DefinitionId.Value}:stacks={effect.Stacks}:source={effect.Provenance}");
        if (effects.Count > limit) output.Append("entries-truncated=true");
        return output.ToString();
    }

    private static string FormatInventory(InventoryComponent value, int limit)
    {
        var view = value.View();
        var output = new DebugOutput();
        output.Append($"owner={value.Owner.Value};stacks={view.Stacks.Count};items={view.UniqueItems.Count}");
        foreach (var stack in view.Stacks.Take(limit))
            output.Append($"stack={stack.Definition.Value}:quantity={stack.Quantity}");
        foreach (var item in view.UniqueItems.Take(limit))
            output.Append($"item={item.Entity.Value}:definition={item.Definition.Value}");
        foreach (var capacity in view.Capacity.Take(limit))
            output.Append($"capacity={capacity.Metric.Value}:used={capacity.Used}:maximum={capacity.Maximum}");
        if (view.Stacks.Count > limit || view.UniqueItems.Count > limit || view.Capacity.Count > limit)
            output.Append("entries-truncated=true");
        return output.ToString();
    }

    private static string FormatEquipment(EquipmentComponent value, int limit)
    {
        var assignments = value.Assignments;
        var output = new DebugOutput();
        output.Append($"owner={value.Owner.Value};assignments={assignments.Count}");
        foreach (var assignment in assignments.Take(limit))
            output.Append($"slot={assignment.Slot.Value}:item={assignment.Item.Value}");
        if (assignments.Count > limit) output.Append("entries-truncated=true");
        return output.ToString();
    }
}
