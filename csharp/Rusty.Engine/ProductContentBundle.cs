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
    private const uint ReadChunkBytes = 1024 * 1024;
    private readonly IContentService service;
    private readonly ContentBundle handle;
    private readonly Dictionary<string, ContentReferenceInfo> files;
    private readonly ContentReferenceInfo[] entries;
    private bool disposed;

    internal ProductContentBundle(string id, IContentService service, ContentBundle handle)
    {
        Id = id;
        this.service = service;
        this.handle = handle;
        entries = service.ReadBundleFiles(handle).ToArray()
            .OrderBy(file => file.Path, StringComparer.Ordinal).ToArray();
        files = entries.ToDictionary(file => file.Path, StringComparer.Ordinal);
    }

    public string Id { get; }

    /// <summary>Ordinal file inventory; accessing it does not copy file bodies.</summary>
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
        byte[] bytes = new byte[checked((int)info.ByteLength)];
        using ContentReference reference = OpenReference(path);
        int offset = 0;
        while (offset < bytes.Length)
        {
            uint count = (uint)Math.Min((long)ReadChunkBytes, bytes.Length - offset);
            ReadOnlyMemory<byte> chunk = service.ReadBytes(new(reference, (ulong)offset, count));
            if (chunk.Length != count) throw new IOException($"Incomplete ProductContent bundle read: {Id}/{path}");
            chunk.Span.CopyTo(bytes.AsSpan(offset));
            offset += chunk.Length;
        }
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
        return entries.Where(file => file.Path.StartsWith(prefix, StringComparison.Ordinal) &&
            (recursive || !file.Path.AsSpan(prefix.Length).Contains('/')))
            .Select(file => ReadFile(file.Path)).ToArray();
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
