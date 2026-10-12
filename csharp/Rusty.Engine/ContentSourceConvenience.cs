using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Rusty.Engine;

/// <summary>What a <see cref="ContentSourceEdit"/> does at its pointer.</summary>
public enum ContentSourceEditKind
{
    /// <summary>Replaces the value the pointer names.</summary>
    Replace,
    /// <summary>Adds an object member (replacing an existing one) or inserts an array element; <c>-</c> appends.</summary>
    Add,
    /// <summary>Removes an object member or array element.</summary>
    Remove,
}

/// <summary>One JSON pointer edit to a source JSON file.</summary>
public readonly record struct ContentSourceEdit(ContentSourceEditKind Kind, string Pointer, JsonNode? Value = null)
{
    public static ContentSourceEdit Replace(string pointer, JsonNode? value) => new(ContentSourceEditKind.Replace, pointer, value);
    public static ContentSourceEdit Add(string pointer, JsonNode? value) => new(ContentSourceEditKind.Add, pointer, value);
    public static ContentSourceEdit Remove(string pointer) => new(ContentSourceEditKind.Remove, pointer);
}

/// <summary>A source file as read, with the SHA-256 an edit names it by.</summary>
public readonly record struct ContentSourceDocument(string Path, ReadOnlyMemory<byte> Bytes, ContentSha256 Sha256)
{
    public string Text => Encoding.UTF8.GetString(Bytes.Span);
}

/// <summary>
/// The developer content source in a <c>rusty dev</c> session: read a source content file and
/// save JSON pointer edits into it. The Engine keeps every byte outside the edited values and
/// writes the staged copy too, so the session keeps running. Elsewhere every call throws
/// <see cref="EngineCallException"/> with <c>CONTENT_SOURCE_UNAVAILABLE</c>.
/// </summary>
public static class ContentSourceConvenience
{
    /// <summary>Reads the source of a content-root-relative path.</summary>
    public static ContentSourceDocument ReadDocument(this IContentSourceService source, string path)
    {
        ArgumentNullException.ThrowIfNull(source);
        ReadOnlyMemory<byte> bytes = source.Read(new(path));
        Span<byte> digest = stackalloc byte[SHA256.HashSizeInBytes];
        SHA256.HashData(bytes.Span, digest);
        return new(path, bytes, new(
            BinaryPrimitives.ReadUInt64BigEndian(digest),
            BinaryPrimitives.ReadUInt64BigEndian(digest[8..]),
            BinaryPrimitives.ReadUInt64BigEndian(digest[16..]),
            BinaryPrimitives.ReadUInt64BigEndian(digest[24..])));
    }

    /// <summary>
    /// Applies the edits to the file <paramref name="read"/> came from, refused with
    /// <c>CONTENT_SOURCE_STALE</c> if it changed since. Returns the new SHA-256.
    /// </summary>
    public static ContentSha256 Edit(this IContentSourceService source, ContentSourceDocument read, params ReadOnlySpan<ContentSourceEdit> edits)
    {
        ArgumentNullException.ThrowIfNull(source);
        return source.Edit(new(read.Path, read.Sha256, Patch(edits)));
    }

    /// <summary>The edits as the RFC 6902 operations the Engine reads.</summary>
    public static string Patch(ReadOnlySpan<ContentSourceEdit> edits)
    {
        using var buffer = new MemoryStream();
        using (var writer = new Utf8JsonWriter(buffer))
        {
            writer.WriteStartArray();
            foreach (ContentSourceEdit edit in edits)
            {
                writer.WriteStartObject();
                writer.WriteString("op", edit.Kind switch
                {
                    ContentSourceEditKind.Replace => "replace",
                    ContentSourceEditKind.Add => "add",
                    ContentSourceEditKind.Remove => "remove",
                    _ => throw new ArgumentOutOfRangeException(nameof(edits)),
                });
                writer.WriteString("path", edit.Pointer);
                if (edit.Kind != ContentSourceEditKind.Remove)
                {
                    writer.WritePropertyName("value");
                    if (edit.Value is null) writer.WriteNullValue();
                    else edit.Value.WriteTo(writer);
                }
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
        }
        return Encoding.UTF8.GetString(buffer.ToArray());
    }
}
