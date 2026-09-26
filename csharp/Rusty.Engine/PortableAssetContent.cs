using System.Numerics;

namespace Rusty.Engine;

/// <summary>An Engine-resolved selection from a portable asset descriptor.</summary>
/// <remarks>Parsing and dependency resolution occur in Rust. This helper composes
/// copied semantic facts with the ordinary content and sprite playback services.</remarks>
public sealed class PortableAssetContent : IDisposable
{
    private readonly IContentService content;
    private readonly PortableAsset asset;
    private bool disposed;

    public PortableAssetContent(IContentService content, ContentReference descriptor, string assetId)
    {
        this.content = content;
        asset = content.LoadPortableAsset(new(descriptor, assetId));
        try { Facts = content.ReadPortableAsset(asset); }
        catch { asset.Dispose(); throw; }
    }

    public PortableAssetReadoutLeaseReceipt Facts { get; }

    /// <summary>The returned reference independently retains its file and dependencies.</summary>
    public ContentReference OpenMember(string memberId)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        return content.OpenPortableAssetMember(new(asset, memberId));
    }

    /// <summary>Resolve an explicit action/direction pair. Missing directions remain missing.</summary>
    public string? FindAnimation(string action, string direction)
    {
        foreach (var mapping in Facts.Actions.Span)
            if (mapping.Action == action && mapping.Direction == direction) return mapping.Animation;
        return null;
    }

    /// <summary>Compose ordered authored timing with an existing Engine sprite atlas.</summary>
    /// <remarks>Atlas frame IDs correspond to the zero-based Facts.Frames order.
    /// This does not create another clock; callers advance the returned playback
    /// through Graphics using ordinary Engine update facts.</remarks>
    public SpritePlayback CreatePlayback(IGraphicsService graphics, Appearance appearance, SpriteAtlas atlas, string animation, double rate = 1)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        var ids = Facts.Frames.ToArray().Select((frame, index) => (Name: frame.Id, Index: checked((uint)index)))
            .ToDictionary(pair => pair.Name, pair => pair.Index, StringComparer.Ordinal);
        var rows = Facts.AnimationFrames.ToArray().Where(row => row.Animation == animation).ToArray();
        if (rows.Length == 0) throw new ArgumentException($"Asset '{Facts.AssetId}' has no animation '{animation}'.", nameof(animation));
        var frames = rows.Select(row => new SpritePlaybackFrame(ids[row.FrameId], row.DurationSeconds)).ToArray();
        return graphics.CreateSpritePlayback(new(appearance, atlas, frames, ReadOnlyMemory<SpritePlaybackMarker>.Empty,
            rows[0].Looping ? SpritePlaybackLoopMode.Loop : SpritePlaybackLoopMode.OneShot, rate));
    }

    /// <summary>Create an ordinary atlas using Engine-decoded texture dimensions.</summary>
    public SpriteAtlas CreateAtlas(IGraphicsService graphics, RenderResource texture, string textureId)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        var info = graphics.ReadTextureInfo(texture);
        return graphics.CreateSpriteAtlas(new(texture, AtlasFrames(textureId, new(info.Width, info.Height))));
    }

    /// <summary>Convert a single-texture descriptor into existing atlas frame values.</summary>
    /// <remarks>Texture extent is the decoded image size. Separate-image descriptors
    /// retain their distinct texture IDs and must be realized using those resources.</remarks>
    public SpriteAtlasFrame[] AtlasFrames(string textureId, Vector2 textureExtent)
    {
        if (!float.IsFinite(textureExtent.X) || !float.IsFinite(textureExtent.Y) || textureExtent.X <= 0 || textureExtent.Y <= 0)
            throw new ArgumentOutOfRangeException(nameof(textureExtent));
        return Facts.Frames.ToArray().Select((frame, index) =>
        {
            if (frame.TextureId != textureId) throw new ArgumentException("This descriptor uses multiple textures; select its individual frame resources.", nameof(textureId));
            if (frame.Origin.X + frame.Extent.X > textureExtent.X || frame.Origin.Y + frame.Extent.Y > textureExtent.Y)
                throw new ArgumentException($"Frame '{frame.Id}' exceeds texture '{textureId}'.", nameof(textureExtent));
            return new SpriteAtlasFrame(checked((uint)index), frame.Origin / textureExtent, (frame.Origin + frame.Extent) / textureExtent, true, frame.Extent);
        }).ToArray();
    }

    public void Dispose()
    {
        if (disposed) return;
        asset.Dispose();
        disposed = true;
    }
}
