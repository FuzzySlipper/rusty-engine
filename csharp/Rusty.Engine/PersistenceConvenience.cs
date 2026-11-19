using System.Text;

namespace Rusty.Engine;

public readonly partial record struct PersistenceSaveReceipt
{
    public PersistenceSaveReceipt(ulong revision)
        : this(PersistenceSaveOutcome.Saved, revision, 0) { }

    /// <summary>A receipt for a save that was not queued.</summary>
    public PersistenceSaveReceipt(PersistenceSaveOutcome outcome, ulong revision)
        : this(outcome, revision, 0) { }
}

public readonly partial record struct PersistenceSaveRequest
{
    /// <summary>A save written in the call; set <c>Deferred</c> to write it off the update.</summary>
    public PersistenceSaveRequest(
        PersistenceStore Store,
        string Key,
        PersistenceRevisionGuard RevisionGuard,
        ulong ExpectedRevision,
        ReadOnlyMemory<byte> Payload)
        : this(Store, Key, RevisionGuard, ExpectedRevision, Payload, false)
    {
    }
}

public readonly partial record struct PersistenceOpenRequest
{
    /// <summary>A durable store; set <c>Cache</c> for regenerable data that may be lost on a crash.</summary>
    public PersistenceOpenRequest(string Scope)
        : this(Scope, false)
    {
    }
}

/// <summary>Text readouts for <see cref="IPersistenceService"/>.</summary>
public static class PersistenceConvenience
{
    /// <summary>Why the store's last failed deferred save failed; empty if none failed.</summary>
    public static string ReadSaveFailureText(this IPersistenceService persistence, PersistenceStore store)
    {
        ArgumentNullException.ThrowIfNull(persistence);
        return Encoding.UTF8.GetString(persistence.ReadSaveFailure(store).Span);
    }
}
