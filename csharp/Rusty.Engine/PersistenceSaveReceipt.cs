namespace Rusty.Engine;

public readonly partial record struct PersistenceSaveReceipt
{
    public PersistenceSaveReceipt(ulong revision, uint schemaVersion)
        : this(PersistenceSaveOutcome.Saved, revision, schemaVersion) { }
}
