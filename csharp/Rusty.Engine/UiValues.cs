using System.Globalization;
using System.Text;
using System.Text.Json.Nodes;

namespace Rusty.Engine;

/// <summary>
/// Builds the <see cref="UiValue"/> a <see cref="UiProjection"/> carries from
/// ordinary JSON, so a product can publish nested lists and objects without
/// laying out structured-value nodes itself.
/// </summary>
public static class UiValues
{
    /// <summary>The structured form of <paramref name="value"/>; null becomes JSON null.</summary>
    public static UiValue FromJson(JsonNode? value)
    {
        Builder builder = new();
        uint root = builder.Add(value, key: null);
        return new UiValue(builder.Nodes.ToArray(), builder.Edges.ToArray(), root, builder.Text.ToArray());
    }

    private sealed class Builder
    {
        public List<StructuredValueNode> Nodes { get; } = [];
        public List<uint> Edges { get; } = [];
        public List<byte> Text { get; } = [];

        public uint Add(JsonNode? value, string? key)
        {
            (uint keyOffset, uint keyLength) = key is null ? (0u, 0u) : Append(key);
            uint index = (uint)Nodes.Count;
            Nodes.Add(default);
            StructuredValueNode node = value switch
            {
                null => new(StructuredValueKind.Null, 0, 0, keyOffset, keyLength, 0, 0, 0, 0),
                JsonObject members => Container(StructuredValueKind.Object, keyOffset, keyLength,
                    members.Select(member => Add(member.Value, member.Key)).ToArray()),
                JsonArray items => Container(StructuredValueKind.Array, keyOffset, keyLength,
                    items.Select(item => Add(item, key: null)).ToArray()),
                JsonValue scalar => Scalar(scalar, keyOffset, keyLength),
                _ => throw new ArgumentException($"unsupported JSON node {value.GetType().Name}", nameof(value)),
            };
            Nodes[(int)index] = node;
            return index;
        }

        private StructuredValueNode Container(StructuredValueKind kind, uint keyOffset, uint keyLength, uint[] children)
        {
            // Children are added first, so their own edges never split these.
            uint firstEdge = (uint)Edges.Count;
            Edges.AddRange(children);
            return new(kind, 0, 0, keyOffset, keyLength, 0, 0, firstEdge, (uint)children.Length);
        }

        private StructuredValueNode Scalar(JsonValue value, uint keyOffset, uint keyLength)
        {
            if (value.TryGetValue(out bool flag))
                return new(StructuredValueKind.Bool, flag ? 1u : 0u, 0, keyOffset, keyLength, 0, 0, 0, 0);
            if (value.TryGetValue(out string? text))
            {
                (uint textOffset, uint textLength) = Append(text);
                return new(StructuredValueKind.String, 0, 0, keyOffset, keyLength, textOffset, textLength, 0, 0);
            }
            // A node built from an int or long holds that type, not double.
            double number = value.TryGetValue(out double exact)
                ? exact
                : double.Parse(value.ToJsonString(), CultureInfo.InvariantCulture);
            return new(StructuredValueKind.Number, 0, number, keyOffset, keyLength, 0, 0, 0, 0);
        }

        private (uint Offset, uint Length) Append(string text)
        {
            byte[] bytes = Encoding.UTF8.GetBytes(text);
            uint offset = (uint)Text.Count;
            Text.AddRange(bytes);
            return (offset, (uint)bytes.Length);
        }
    }
}
