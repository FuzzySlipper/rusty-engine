using System.Text;

namespace Rusty.Engine;

/// <summary>An independently loaded immutable build-content collection.</summary>
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
        files = new(StringComparer.Ordinal);
        foreach (var file in entries.Span) files.Add(file.Path, file);
    }

    public string Id { get; }

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
