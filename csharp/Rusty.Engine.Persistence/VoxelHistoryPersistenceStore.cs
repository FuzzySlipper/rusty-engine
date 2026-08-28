namespace Rusty.Engine.Persistence;

/// <summary>
/// Product-selected durable location for the Engine-owned runtime voxel edit
/// history of one spatial session. This composes generated Voxel and
/// Persistence services; it does not define a second history model or codec.
/// </summary>
public sealed class VoxelHistoryPersistenceStore : IDisposable
{
    private readonly IVoxelService _voxel;
    private readonly IPersistenceService _persistence;
    private readonly PersistenceStore _store;
    private readonly VoxelHistoryCodecInfo _codec;

    public VoxelHistoryPersistenceStore(IEngineContext engine, string scope)
    {
        ArgumentNullException.ThrowIfNull(engine);
        ArgumentException.ThrowIfNullOrWhiteSpace(scope);
        _voxel = engine.Voxel;
        _persistence = engine.Persistence;
        _store = _persistence.OpenStore(new PersistenceOpenRequest(scope));
        _codec = _voxel.ReadHistoryCodecInfo();
    }

    public PersistenceSaveReceipt Save(
        string key,
        SpatialSession session,
        PersistenceRevisionGuard guard = PersistenceRevisionGuard.Any,
        ulong expectedRevision = 0)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(key);
        ReadOnlyMemory<byte> export = _voxel.ExportHistory(new VoxelHistoryExportRequest(session));
        if ((ulong)export.Length > _codec.MaxEncodedBytes)
        {
            throw new InvalidOperationException("Voxel history export did not match the active Engine codec contract.");
        }

        return _persistence.Save(new PersistenceSaveRequest(
            _store,
            key,
            _codec.SchemaVersion,
            guard,
            expectedRevision,
            export));
    }

    public VoxelHistoryPersistenceLoad LoadAndRestore(string key, SpatialSession session)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(key);
        using PersistenceBlob blob = _persistence.Load(new PersistenceLoadRequest(_store, key));
        PersistenceBlobInfo info = _persistence.DescribeBlob(blob);
        if (!info.Present)
        {
            return new VoxelHistoryPersistenceLoad(false, 0, default);
        }
        if (info.SchemaVersion != _codec.SchemaVersion)
        {
            throw new InvalidOperationException($"Voxel history at '{key}' has schema {info.SchemaVersion}, but Engine requires {_codec.SchemaVersion}.");
        }
        if (info.PayloadLen > _codec.MaxEncodedBytes)
        {
            throw new InvalidOperationException($"Voxel history at '{key}' exceeds the Engine codec bound.");
        }

        byte[] payload = new byte[checked((int)info.PayloadLen)];
        _persistence.CopyBlob(new PersistenceCopyBlobRequest(blob, payload));
        VoxelHistoryRestoreReceipt restore = _voxel.RestoreHistory(new VoxelHistoryRestoreRequest(session, payload));
        return new VoxelHistoryPersistenceLoad(true, info.Revision, restore);
    }

    public void Dispose() => _store.Dispose();
}

/// <summary>Persistence revision and typed Engine cursor/revision facts after a successful restore.</summary>
public readonly record struct VoxelHistoryPersistenceLoad(
    bool Present,
    ulong PersistenceRevision,
    VoxelHistoryRestoreReceipt Restore);
