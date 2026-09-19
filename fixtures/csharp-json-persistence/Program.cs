using System.Text.Json;
using System.Text.Json.Serialization;
using Rusty.Engine;
using Rusty.Engine.Persistence;

// NativeAOT fidelity proof for JsonProductStateCodec<T>: the only serialization metadata on
// this path is a source-generated JsonSerializerContext. Nothing here uses reflection-based
// serialization, so the whole save/load roundtrip must survive trimming and native compile.
// Build: dotnet publish fixtures/csharp-json-persistence -c Release (PublishAot is set).
// Run the published native binary; exit 0 means the proof held.
return ExpeditionProof.Run();

internal static class ExpeditionProof
{
    public static int Run()
    {
        var log = new ExpeditionLog
        {
            Title = "Northern Relay",
            Waypoints =
            [
                new ExpeditionWaypoint { Name = "Frostgate", Reached = true },
                new ExpeditionWaypoint { Name = "Thawline", Reached = false },
            ],
            Supplies = new Dictionary<string, int> { ["rations"] = 6, ["rope"] = 2 },
        };

        var persistence = new MemoryPersistenceService();
        using var store = new ProductStateStore<ExpeditionLog>(
            new ProofEngineContext(persistence), "json-aot-proof",
            new JsonProductStateCodec<ExpeditionLog>(ExpeditionJsonContext.Default.ExpeditionLog));
        PersistenceSaveReceipt saved = store.Save("expedition", log);
        Require(saved.Outcome == PersistenceSaveOutcome.Saved && saved.SchemaVersion == 0,
            "the AOT save did not report a versionless save");
        ProductStateLoad<ExpeditionLog> loaded = store.Load("expedition");
        ExpeditionLog? expedition = loaded.State;
        Require(loaded.Present && loaded.Revision == saved.Revision && expedition is not null
            && expedition.Title == "Northern Relay"
            && expedition.Waypoints.Count == 2
            && expedition.Waypoints[0].Name == "Frostgate" && expedition.Waypoints[0].Reached
            && !expedition.Waypoints[1].Reached
            && expedition.Supplies["rations"] == 6 && expedition.Supplies["rope"] == 2,
            "the AOT roundtrip did not preserve nested data and collections");

        Require(!store.Load("never-saved").Present, "a missing AOT save did not report absent");
        persistence.Seed("json-aot-proof", "corrupt", 0, "{not json"u8.ToArray());
        JsonException? malformed = null;
        try
        {
            store.Load("corrupt");
        }
        catch (JsonException error)
        {
            malformed = error;
        }
        Require(malformed is not null, "invalid JSON did not fail with an understandable error");
        persistence.Seed("json-aot-proof", "nulldoc", 0, "null"u8.ToArray());
        InvalidOperationException? nullDoc = null;
        try
        {
            store.Load("nulldoc");
        }
        catch (InvalidOperationException error) when (error.Message.Contains("decoded to null", StringComparison.Ordinal))
        {
            nullDoc = error;
        }
        Require(nullDoc is not null, "a JSON null document did not fail the load");
        return 0;
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}

internal sealed class ExpeditionWaypoint
{
    public string Name { get; set; } = string.Empty;
    public bool Reached { get; set; }
}

internal sealed class ExpeditionLog
{
    public string Title { get; set; } = string.Empty;
    public List<ExpeditionWaypoint> Waypoints { get; set; } = [];
    public Dictionary<string, int> Supplies { get; set; } = new();
}

[JsonSerializable(typeof(ExpeditionLog))]
internal partial class ExpeditionJsonContext : JsonSerializerContext
{
}

internal sealed class ProofEngineContext(IPersistenceService persistence) : IEngineContext
{
    public IDiagnosticsService Diagnostics => throw new NotSupportedException();
    public IDynamicsService Dynamics => throw new NotSupportedException();
    public IMotionService Motion => throw new NotSupportedException();
    public IKinematicService Kinematic => throw new NotSupportedException();
    public ISpatialService Spatial => throw new NotSupportedException();
    public IPerceptionService Perception => throw new NotSupportedException();
    public IWorldOriginService WorldOrigin => throw new NotSupportedException();
    public IVoxelService Voxel => throw new NotSupportedException();
    public IVoxelContentService VoxelContent => throw new NotSupportedException();
    public IContentService Content => throw new NotSupportedException();
    public IAuthoredContentService AuthoredContent => throw new NotSupportedException();
    public IGraphicsService Graphics => throw new NotSupportedException();
    public IImplicitSurfacesService ImplicitSurfaces => throw new NotSupportedException();
    public IPresentationService Presentation => throw new NotSupportedException();
    public IAnimationService Animation => throw new NotSupportedException();
    public IAudioService Audio => throw new NotSupportedException();
    public ICameraViewService CameraView => throw new NotSupportedException();
    public IRandomService Random => throw new NotSupportedException();
    public IVoxelScenePresentationService VoxelScenePresentation => throw new NotSupportedException();
    public IPersistenceService Persistence { get; } = persistence;
    public IContentStoreService ContentStore => throw new NotSupportedException();
    public IUiService Ui => throw new NotSupportedException();
}

internal sealed class MemoryPersistenceService : IPersistenceService
{
    private sealed record Stored(ulong Revision, byte[] Payload);

    private readonly Dictionary<ulong, string> _scopes = [];
    private readonly Dictionary<ulong, Stored> _blobs = [];
    private readonly Dictionary<(string Scope, string Key), Stored> _saved = [];
    private ulong _nextHandle = 1;

    public PersistenceStore OpenStore(PersistenceOpenRequest request)
    {
        ulong handle = _nextHandle++;
        _scopes.Add(handle, request.Scope);
        return new PersistenceStore(new PersistenceStoreHandle(handle), () => _scopes.Remove(handle));
    }

    public PersistenceSaveReceipt Save(PersistenceSaveRequest request)
    {
        string scope = _scopes[request.Store.Handle.Value];
        var key = (scope, request.Key);
        _saved.TryGetValue(key, out Stored? previous);
        ulong revision = (previous?.Revision ?? 0) + 1;
        _saved[key] = new Stored(revision, request.Payload.ToArray());
        return new PersistenceSaveReceipt(revision, request.SchemaVersion);
    }

    public PersistenceBlob Load(PersistenceLoadRequest request)
    {
        string scope = _scopes[request.Store.Handle.Value];
        _saved.TryGetValue((scope, request.Key), out Stored? saved);
        ulong handle = _nextHandle++;
        _blobs.Add(handle, saved ?? new Stored(0, []));
        return new PersistenceBlob(new PersistenceBlobHandle(handle), () => _blobs.Remove(handle));
    }

    public PersistenceBlobInfo DescribeBlob(PersistenceBlob blob)
    {
        Stored saved = _blobs[blob.Handle.Value];
        return new PersistenceBlobInfo(saved.Revision != 0, 0, saved.Revision, (nuint)saved.Payload.Length);
    }

    public void CopyBlob(PersistenceCopyBlobRequest request)
        => _blobs[request.Blob.Handle.Value].Payload.CopyTo(request.Destination.Span);

    public ReadOnlyMemory<byte> ReadBlobBytes(PersistenceBlob blob)
        => _blobs[blob.Handle.Value].Payload;

    public void Seed(string scope, string key, uint schemaVersion, byte[] payload)
        => _saved[(scope, key)] = new Stored(1, payload);
}
