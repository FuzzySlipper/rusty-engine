namespace Rusty.Engine;

public readonly partial record struct RenderResourceRequest
{
    /// <summary>Opens a resource with nearest filtering and clamp wrapping for PNG textures.</summary>
    public RenderResourceRequest(string path)
        : this(path, TextureFilter.Nearest, TextureWrap.Clamp) { }
}
