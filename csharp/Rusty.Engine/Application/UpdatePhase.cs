namespace Rusty.Engine.Application;

/// <summary>A named position in a product-owned update pass.</summary>
public readonly record struct UpdatePhase(string Name)
{
    public static UpdatePhase Input { get; } = new("input");
    public static UpdatePhase Update { get; } = new("update");
    public static UpdatePhase LateUpdate { get; } = new("late-update");
    public static UpdatePhase Presentation { get; } = new("presentation");
}
