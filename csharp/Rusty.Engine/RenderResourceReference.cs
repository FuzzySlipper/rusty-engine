namespace Rusty.Engine;

/// <summary>A non-owning texture reference; default means no optional texture.</summary>
public readonly partial record struct RenderResourceReference
{
    public RenderResourceReference(RenderResource resource) : this(resource.Handle.Value) { }
    public static implicit operator RenderResourceReference(RenderResource resource) => new(resource);
}
