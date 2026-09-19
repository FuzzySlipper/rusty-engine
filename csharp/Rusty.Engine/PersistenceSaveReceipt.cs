namespace Rusty.Engine;

public readonly partial record struct PersistenceSaveReceipt
{
    public PersistenceSaveReceipt(ulong revision)
        : this(PersistenceSaveOutcome.Saved, revision) { }
}
