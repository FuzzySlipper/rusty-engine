using System.Numerics;
using System.Text.Json;
using Rusty.Engine;

namespace CsharpCloakComparison;

/// <summary>
/// The cloak both modes draw, read from <c>cloak.json</c> (derived beside the
/// supplied mesh): the drawn cloak as the GLB has it, the coarse sheet the
/// soft cloak simulates under it, the joints its collar row hangs from, and
/// the cape bones the bone cloak simulates. Positions are at rest (the
/// T-pose), the character at the origin facing +Z.
/// </summary>
internal sealed class CloakAsset
{
    public required Vector3[] SkinPositions { get; init; }
    public required Vector2[] SkinUvs { get; init; }
    public required uint[] SkinTriangles { get; init; }
    public required Vector3[] ProxyPositions { get; init; }
    public required uint[] ProxyTriangles { get; init; }
    public required int ProxyColumns { get; init; }
    public required int ProxyRows { get; init; }
    /// <summary>The proxy's collar row and the joint each particle follows.</summary>
    public required (uint Particle, string Joint)[] Collar { get; init; }
    public required CapeColumn[] Columns { get; init; }

    /// <summary>One column of cape bones, top down, and the last bone's length and the column's width.</summary>
    internal sealed record CapeColumn(string[] Bones, float LastLength, float Width);

    public static CloakAsset Read(IEngineContext engine, string path)
    {
        using ContentReference source = engine.Content.OpenReference(new(path));
        ulong length = engine.Content.ReadReferenceInfo(source).Span[0].ByteLength;
        using JsonDocument json = JsonDocument.Parse(engine.Content.ReadBytes(new(source, 0, (uint)length)));
        JsonElement root = json.RootElement;
        JsonElement skin = root.GetProperty("skin"), proxy = root.GetProperty("proxy");
        return new CloakAsset
        {
            SkinPositions = Points(skin.GetProperty("positions")),
            SkinUvs = [.. skin.GetProperty("uvs").EnumerateArray().Select(uv => new Vector2(uv[0].GetSingle(), uv[1].GetSingle()))],
            SkinTriangles = Indices(skin.GetProperty("triangles")),
            ProxyPositions = Points(proxy.GetProperty("positions")),
            ProxyTriangles = Indices(proxy.GetProperty("triangles")),
            ProxyColumns = proxy.GetProperty("columns").GetInt32(),
            ProxyRows = proxy.GetProperty("rows").GetInt32(),
            Collar = [.. proxy.GetProperty("attachments").EnumerateArray().Select(attachment =>
                (attachment.GetProperty("particle").GetUInt32(), attachment.GetProperty("joint").GetString()!))],
            Columns = [.. root.GetProperty("bones").EnumerateArray().Select(column => new CapeColumn(
                [.. column.GetProperty("bones").EnumerateArray().Select(bone => bone.GetString()!)],
                column.GetProperty("length").GetSingle(),
                column.GetProperty("width").GetSingle()))],
        };
    }

    private static Vector3[] Points(JsonElement list) =>
        [.. list.EnumerateArray().Select(p => new Vector3(p[0].GetSingle(), p[1].GetSingle(), p[2].GetSingle()))];

    private static uint[] Indices(JsonElement list) =>
        [.. list.EnumerateArray().SelectMany(triangle => triangle.EnumerateArray().Select(index => index.GetUInt32()))];

    /// <summary>The proxy's bottom row: the hem.</summary>
    public IEnumerable<uint> Hem => Enumerable.Range((ProxyRows - 1) * ProxyColumns, ProxyColumns).Select(index => (uint)index);
}
