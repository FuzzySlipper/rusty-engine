using System.Text;

namespace Rusty.Engine;

/// <summary>An independently loaded immutable content collection: a build bundle or an opened container.</summary>
/// <remarks>
/// File paths are bundle-relative. Reads copy requested bytes into managed memory;
/// enumeration of Entries copies metadata only. Dispose releases the collection's
/// native ownership, not previously returned managed files or independently retained
/// ContentReferences/resources. Those consumers have their own lifetimes.
/// </remarks>
public sealed class ProductContentBundle : IDisposable
{
    private readonly IContentService service;
    private readonly ContentBundle handle;
    private readonly Dictionary<string, ContentReferenceInfo> files;
    private readonly ReadOnlyMemory<ContentReferenceInfo> entries;
    private bool disposed;

    internal ProductContentBundle(string id, IContentService service, ContentBundle handle)
    {
        Id = id;
        this.service = service;
        this.handle = handle;
        entries = service.ReadBundleFiles(handle);
        Identity = service.ReadBundleIdentity(handle);
        files = new(StringComparer.Ordinal);
        foreach (var file in entries.Span) files.Add(file.Path, file);
    }

    /// <summary>
    /// Open a content container at a filesystem path the product chose (packed with
    /// <c>rusty pack-content</c>) as a bundle with this same surface. Opening checks the
    /// container's header and inventory; each file's bytes are read when first used. A missing,
    /// truncated or corrupt container throws <see cref="EngineCallException"/> with
    /// <c>PRODUCT_SOURCE_IO</c>, <c>PRODUCT_CONTAINER_TRUNCATED</c>, <c>PRODUCT_CONTAINER_CORRUPT</c>
    /// or <c>PRODUCT_CONTAINER_NOT_A_CONTAINER</c>. Tools use it through an
    /// <c>EngineTestHost</c>'s <c>Content</c>.
    /// </summary>
    public static ProductContentBundle OpenContainer(IContentService content, string path)
    {
        ArgumentNullException.ThrowIfNull(content);
        ArgumentNullException.ThrowIfNull(path);
        ContentBundle handle = content.OpenContainer(new(path));
        try { return new ProductContentBundle(path, content, handle); }
        catch { handle.Dispose(); throw; }
    }

    /// <summary>The bundle ID, or the path a container was opened from.</summary>
    public string Id { get; }

    /// <summary>
    /// The collection's identity: SHA-256 over each file's bundle-relative path and SHA-256, in
    /// path order. It follows the files, not how they are stored, so a container packed from a
    /// directory has the identity of a build bundle with the same files.
    /// </summary>
    public ContentSha256 Identity { get; }

    /// <summary>File inventory in Engine UTF-8 path order; accessing it does not copy file bodies.</summary>
    public ReadOnlyMemory<ContentReferenceInfo> Entries { get { ThrowIfDisposed(); return entries; } }

    /// <summary>Retain Engine content for native services without copying through managed memory.</summary>
    public ContentReference OpenReference(string path)
    {
        ThrowIfDisposed();
        RequireFile(path);
        return service.OpenBundleReference(new(handle, path));
    }

    public ProductContentFile ReadFile(string path)
    {
        ThrowIfDisposed();
        ContentReferenceInfo info = RequireFile(path);
        using ContentReference reference = service.OpenBundleReference(new(handle, path));
        ReadOnlyMemory<byte> bytes = service.ReadBytes(new(reference, 0, checked((uint)info.ByteLength)));
        return new(Encoding.UTF8.GetBytes(path), bytes);
    }

    public bool TryReadFile(string path, out ProductContentFile file)
    {
        ThrowIfDisposed();
        if (!files.ContainsKey(path)) { file = default; return false; }
        file = ReadFile(path);
        return true;
    }

    public ReadOnlyMemory<byte> ReadBytes(string path) => ReadFile(path).Bytes;
    public string ReadText(string path) => ReadFile(path).ReadText();

    public ProductContentFile[] ReadDirectory(string path = "", bool recursive = false)
    {
        ThrowIfDisposed();
        ArgumentNullException.ThrowIfNull(path);
        if (path.StartsWith('/')) return [];
        string directory = path.TrimEnd('/');
        string prefix = directory.Length == 0 ? "" : directory + "/";
        List<ProductContentFile> result = [];
        foreach (var file in entries.Span)
            if (file.Path.StartsWith(prefix, StringComparison.Ordinal) &&
                (recursive || !file.Path.AsSpan(prefix.Length).Contains('/')))
                result.Add(ReadFile(file.Path));
        return result.ToArray();
    }

    private ContentReferenceInfo RequireFile(string path) => files.TryGetValue(path, out var file)
        ? file : throw new FileNotFoundException($"ProductContent bundle file was not found: {Id}/{path}", path);

    private void ThrowIfDisposed() => ObjectDisposedException.ThrowIf(disposed, this);

    public void Dispose()
    {
        if (disposed) return;
        handle.Dispose();
        disposed = true;
    }
}
