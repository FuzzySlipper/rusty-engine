using System.Buffers;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.Json.Serialization.Metadata;

namespace Rusty.Engine.Persistence;

/// <summary>
/// Optional <see cref="System.Text.Json"/> codec over <see cref="ProductStateStore{TState}"/>.
/// Standard serialization behavior applies: no Engine serialization DSL, product identity
/// registry, polymorphic component resolver, or global serializer settings are involved.
/// Malformed bytes fail in deserialization; a JSON null document fails rather than decoding
/// to a missing value.
/// </summary>
public sealed class JsonProductStateCodec<TState> : IProductStateCodec<TState>
{
    private readonly JsonTypeInfo<TState> _typeInfo;

    /// <summary>
    /// Explicit serialization metadata, such as a source-generated context's
    /// <c>JsonTypeInfo&lt;TState&gt;</c>. This is the NativeAOT-safe path: it performs no
    /// reflection discovery and survives trimming.
    /// </summary>
    public JsonProductStateCodec(JsonTypeInfo<TState> typeInfo)
    {
        ArgumentNullException.ThrowIfNull(typeInfo);
        _typeInfo = typeInfo;
    }

    /// <summary>
    /// CoreCLR convenience over reflection-based metadata from the supplied options (or the
    /// shared defaults when null). A caller-configured resolver is honored first; otherwise
    /// metadata falls back to runtime reflection. This path is not trimming- or
    /// NativeAOT-safe: under NativeAOT use the <see cref="JsonTypeInfo{T}"/> overload with a
    /// source-generated context instead.
    /// </summary>
    public JsonProductStateCodec(JsonSerializerOptions? options = null)
        : this(ResolveReflectionMetadata(options))
    {
    }

    public void Encode(in TState state, IBufferWriter<byte> destination)
    {
        ArgumentNullException.ThrowIfNull(destination);
        using var writer = new Utf8JsonWriter(destination);
        JsonSerializer.Serialize(writer, state, _typeInfo);
    }

    public TState Decode(ReadOnlySpan<byte> payload)
        => JsonSerializer.Deserialize(payload, _typeInfo) is TState state
            ? state
            : throw new InvalidOperationException("JSON product state decoded to null.");

    private static JsonTypeInfo<TState> ResolveReflectionMetadata(JsonSerializerOptions? options)
    {
        JsonSerializerOptions effective = options ?? JsonSerializerOptions.Default;
        try
        {
            if (effective.GetTypeInfo(typeof(TState)) is JsonTypeInfo<TState> configured)
            {
                return configured;
            }
        }
        catch (NotSupportedException)
        {
            // No configured resolver covers TState; fall through to reflection below.
        }
        var fallback = new JsonSerializerOptions(effective);
        fallback.TypeInfoResolverChain.Add(new DefaultJsonTypeInfoResolver());
        if (fallback.GetTypeInfo(typeof(TState)) is JsonTypeInfo<TState> reflected)
        {
            return reflected;
        }
        throw new InvalidOperationException($"No JSON metadata is available for {typeof(TState).Name}.");
    }
}
