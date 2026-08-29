namespace Rusty.Engine.Resolution;

/// <summary>Stable lineage for one structural attempt within a session.</summary>
public readonly record struct ResolutionIdentity(
    ulong Resolution,
    ulong Correlation,
    ulong? Parent,
    int Depth)
{
    public bool HasParent => Parent.HasValue;

    public static ResolutionIdentity Root(ulong resolution, ulong correlation) =>
        new(resolution, correlation, null, 0);

    internal ResolutionIdentity Child(ulong resolution) =>
        new(resolution, Correlation, Resolution, checked(Depth + 1));
}

public enum ResolutionMode
{
    Preview,
    Apply,
}

public enum ResolutionAttemptStatus
{
    Open,
    Planned,
    Rejected,
    Suspended,
    Faulted,
    LimitExceeded,
    ChildFailed,
}

public enum ResolutionCommitStatus
{
    NotAttempted,
    Previewed,
    Applied,
    TransactionFailed,
    Abandoned,
}
