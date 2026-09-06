namespace Rusty.Engine;

public readonly partial record struct CameraTargetReference
{
    /// <summary>Selects the Engine primary presentation surface.</summary>
    public static readonly CameraTargetReference Primary = new(0);

    public CameraTargetReference(CameraTarget target)
        : this(target.Handle.Value)
    {
    }
}
