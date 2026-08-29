using System;

namespace Rusty.Engine.Resolution;

public readonly record struct ResolutionAttemptCounts(
    int Evidence,
    int Work,
    int Effects,
    int Events,
    int Children);

/// <summary>A copied structural readout; product facts and effect payloads remain product-owned.</summary>
public readonly record struct ResolutionAttemptReceipt(
    ResolutionIdentity Identity,
    bool IsRoot,
    ResolutionAttemptStatus Status,
    ResolutionAttemptCounts Counts);

/// <summary>Compact outcome of one managed structural session.</summary>
public readonly record struct ResolutionReceipt(
    ReadOnlyMemory<ResolutionAttemptReceipt> Attempts,
    ResolutionCommitStatus Commit);
