using System.Text;

namespace Rusty.Engine;

/// <summary>A snapshot of bundled product files admitted by the Engine host.</summary>
/// <remarks>
/// Paths are case-sensitive, content-root-relative names with '/' separators.
/// Reads access the admitted memory snapshot, not the host filesystem. The product
/// owns file formats and interpretation. Retained path and payload memory must not
/// be mutated by callers.
/// </remarks>
public sealed class ProductContent
{
    private readonly Dictionary<string, ProductContentFile> byPath;
    private readonly KeyValuePair<string, ProductContentFile>[] orderedFiles;

    public ProductContent(ReadOnlyMemory<ProductContentFile> files)
    {
        Files = files.ToArray();
        byPath = new(StringComparer.Ordinal);
        foreach (ProductContentFile file in Files.Span)
            byPath.Add(file.RelativePath, file);
        orderedFiles = byPath.OrderBy(pair => pair.Key, StringComparer.Ordinal).ToArray();
    }

    /// <summary>The admitted entries in their original order, for existing bulk consumers.</summary>
    public ReadOnlyMemory<ProductContentFile> Files { get; }

    /// <summary>Find an optional file by its exact content-relative path.</summary>
    public bool TryReadFile(string path, out ProductContentFile file) => byPath.TryGetValue(path, out file);

    /// <summary>Read a required file, or throw <see cref="FileNotFoundException"/>.</summary>
    public ProductContentFile ReadFile(string path) => TryReadFile(path, out ProductContentFile file)
        ? file : throw new FileNotFoundException($"Bundled product content was not found: {path}", path);

    public ReadOnlyMemory<byte> ReadBytes(string path) => ReadFile(path).Bytes;

    /// <summary>Decode a bundled file as UTF-8, accepting an optional UTF-8 BOM.</summary>
    public string ReadText(string path) => ReadFile(path).ReadText();

    /// <summary>Read files directly in a directory, or in its entire subtree when recursive.</summary>
    /// <remarks>
    /// An empty path selects the content root. A trailing '/' is optional. Results
    /// are ordered by full relative path using ordinal comparison. Missing or empty
    /// directories return an empty array; directory entries themselves are not retained.
    /// No filename, including _index.json, has special Engine meaning.
    /// </remarks>
    public ProductContentFile[] ReadDirectory(string path = "", bool recursive = false)
    {
        ArgumentNullException.ThrowIfNull(path);
        string directory = path.TrimEnd('/');
        // A leading slash is not the content root: all names are relative.
        if (path.StartsWith('/')) return [];
        string prefix = directory.Length == 0 ? "" : directory + "/";
        List<ProductContentFile> result = [];
        foreach ((string name, ProductContentFile file) in orderedFiles)
        {
            if (!name.StartsWith(prefix, StringComparison.Ordinal)) continue;
            if (!recursive && name.AsSpan(prefix.Length).Contains('/')) continue;
            result.Add(file);
        }
        return result.ToArray();
    }
}

public readonly partial record struct ProductContentFile
{
    /// <summary>The UTF-8 path decoded as a content-root-relative name.</summary>
    public string RelativePath => Encoding.UTF8.GetString(Path.Span);

    public string Name
    {
        get
        {
            string path = RelativePath;
            return path[(path.LastIndexOf('/') + 1)..];
        }
    }

    public string ReadText()
    {
        ReadOnlySpan<byte> bytes = Bytes.Span;
        if (bytes.StartsWith(Encoding.UTF8.Preamble)) bytes = bytes[Encoding.UTF8.Preamble.Length..];
        return Encoding.UTF8.GetString(bytes);
    }
}
