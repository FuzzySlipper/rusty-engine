namespace Rusty.Engine;

public readonly partial record struct RenderResourceRequest
{
    /// <summary>Opens a resource with nearest filtering and clamp wrapping for PNG textures.</summary>
    public RenderResourceRequest(string path)
        : this(path, TextureFilter.Nearest, TextureWrap.Clamp) { }

    /// <summary>Opens a PNG texture as sRGB colour with the given sampling.</summary>
    public RenderResourceRequest(string path, TextureFilter filter, TextureWrap wrap)
        : this(path, filter, wrap, TextureColorSpace.Srgb) { }

    /// <summary>Opens a resource; a shader opens without keywords.</summary>
    public RenderResourceRequest(string path, TextureFilter filter, TextureWrap wrap, TextureColorSpace colorSpace)
        : this(path, filter, wrap, colorSpace, string.Empty) { }
}

public readonly partial record struct RenderResourceContentRequest
{
    /// <summary>Opens a PNG texture as sRGB colour with the given sampling.</summary>
    public RenderResourceContentRequest(ContentReference content, TextureFilter filter, TextureWrap wrap)
        : this(content, filter, wrap, TextureColorSpace.Srgb) { }

    /// <summary>Opens a resource; a shader opens without keywords.</summary>
    public RenderResourceContentRequest(ContentReference content, TextureFilter filter, TextureWrap wrap, TextureColorSpace colorSpace)
        : this(content, filter, wrap, colorSpace, string.Empty) { }

    /// <summary>Opens a resource in the call; a texture is decoded and uploaded before the call returns.</summary>
    public RenderResourceContentRequest(ContentReference content, TextureFilter filter, TextureWrap wrap, TextureColorSpace colorSpace, string shaderKeywords)
        : this(content, filter, wrap, colorSpace, shaderKeywords, false) { }
}

public readonly partial record struct StaticMeshContentReferenceRequest
{
    /// <summary>A static mesh uploaded in the call.</summary>
    public StaticMeshContentReferenceRequest(ContentReference content, Color color)
        : this(content, color, false) { }
}
