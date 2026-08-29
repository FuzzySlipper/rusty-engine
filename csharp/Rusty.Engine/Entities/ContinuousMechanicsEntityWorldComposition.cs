using Rusty.Engine;

namespace Rusty.Engine.Entities;

/// <summary>
/// An explicit, ordered product composition of independent continuous catalogs over one exact
/// Mechanics world. It is deliberately a list supplied at construction, not a catalog registry.
/// </summary>
public sealed class ContinuousMechanicsEntityWorldComposition
{
    private readonly MechanicsEntityWorld _mechanicsWorld;
    private readonly IReadOnlyList<ContinuousMechanicsEntityWorld> _adapters;

    public ContinuousMechanicsEntityWorldComposition(
        MechanicsEntityWorld mechanicsWorld,
        IReadOnlyList<ContinuousMechanicsEntityWorld> adapters)
    {
        _mechanicsWorld = mechanicsWorld ?? throw new ArgumentNullException(nameof(mechanicsWorld));
        ArgumentNullException.ThrowIfNull(adapters);
        _adapters = adapters.ToArray();

        IContinuousMechanicsService? continuous = null;
        var catalogs = new HashSet<ulong>();
        foreach (ContinuousMechanicsEntityWorld adapter in _adapters)
        {
            ArgumentNullException.ThrowIfNull(adapter);
            if (!ReferenceEquals(adapter.ExactWorld, _mechanicsWorld))
            {
                throw new InvalidOperationException("Every continuous adapter must borrow this exact Mechanics world.");
            }
            if (continuous is not null && !ReferenceEquals(adapter.ContinuousService, continuous))
            {
                throw new InvalidOperationException("Every continuous adapter in one composition must use the same continuous service.");
            }
            continuous ??= adapter.ContinuousService;
            if (!catalogs.Add(adapter.ContinuousCatalog.Handle.Value))
            {
                throw new InvalidOperationException("A continuous catalog may appear only once in a composition.");
            }
        }
    }

    /// <summary>Exports exact facts once, followed by copied catalog-scoped receipts in order.</summary>
    public ContinuousMechanicsEntityWorldCompositionExport Export()
    {
        MechanicsWorldExportLeaseReceipt exact = _mechanicsWorld.Export();
        ContinuousMechanicsWorldExportLeaseReceipt[] continuous = _adapters
            .Select(adapter => adapter.ExportContinuous(exact))
            .ToArray();
        return new ContinuousMechanicsEntityWorldCompositionExport(exact, Array.AsReadOnly(continuous));
    }

    /// <summary>
    /// Stages an explicit ordered sequence of catalog images into one detached exact candidate.
    /// Every validation happens before the exact candidate can be published.
    /// </summary>
    public ContinuousMechanicsEntityWorldCompositionImportCandidate PrepareImport(
        EntityWorldRestorePlan plan,
        MechanicsWorldImportRequest mechanics,
        IReadOnlyList<ContinuousMechanicsEntityWorldImportPart> continuous,
        ulong? expectedManagedRevision = null)
    {
        ArgumentNullException.ThrowIfNull(plan);
        ArgumentNullException.ThrowIfNull(continuous);
        ContinuousMechanicsEntityWorldImportPart[] parts = continuous.ToArray();
        ValidateImportParts(mechanics, parts);

        var receipts = new List<ContinuousMechanicsWorldImportLeaseReceipt>(parts.Length);
        MechanicsEntityWorldImportCandidate exact = _mechanicsWorld.PrepareImport(
            plan,
            mechanics,
            expectedManagedRevision,
            (import, admitted) =>
            {
                foreach (ContinuousMechanicsEntityWorldImportPart part in parts)
                {
                    receipts.Add(part.Adapter.StageImport(import, admitted, part.Image));
                }
            });
        return new ContinuousMechanicsEntityWorldCompositionImportCandidate(exact, receipts);
    }

    private void ValidateImportParts(
        MechanicsWorldImportRequest mechanics,
        IReadOnlyList<ContinuousMechanicsEntityWorldImportPart> parts)
    {
        var catalogs = new HashSet<ulong>();
        var coveredEntities = new HashSet<ulong>();
        foreach (ContinuousMechanicsEntityWorldImportPart part in parts)
        {
            ArgumentNullException.ThrowIfNull(part);
            if (!_adapters.Contains(part.Adapter))
            {
                throw new InvalidOperationException("Continuous import parts must use an adapter selected by this composition.");
            }
            if (!catalogs.Add(part.Adapter.ContinuousCatalog.Handle.Value))
            {
                throw new InvalidOperationException("Continuous import parts must not repeat a catalog.");
            }
            foreach (ulong entity in ContinuousMechanicsEntityWorld.ValidateImage(mechanics, part.Image))
            {
                if (!coveredEntities.Add(entity))
                {
                    throw new InvalidOperationException("Continuous import parts must have disjoint exact entity subsets.");
                }
            }
        }
    }
}

/// <summary>One product-selected adapter and its decoded handle-free catalog image.</summary>
public sealed class ContinuousMechanicsEntityWorldImportPart
{
    public ContinuousMechanicsEntityWorldImportPart(
        ContinuousMechanicsEntityWorld adapter,
        ContinuousMechanicsWorldImportImage image)
    {
        Adapter = adapter ?? throw new ArgumentNullException(nameof(adapter));
        Image = image ?? throw new ArgumentNullException(nameof(image));
    }

    public ContinuousMechanicsEntityWorld Adapter { get; }
    public ContinuousMechanicsWorldImportImage Image { get; }
}

/// <summary>Copied exact receipt and ordered catalog-scoped continuous receipts.</summary>
public readonly record struct ContinuousMechanicsEntityWorldCompositionExport(
    MechanicsWorldExportLeaseReceipt Exact,
    IReadOnlyList<ContinuousMechanicsWorldExportLeaseReceipt> Continuous);

/// <summary>Owns only one exact candidate; continuous staging is represented by copied receipts.</summary>
public sealed class ContinuousMechanicsEntityWorldCompositionImportCandidate : IDisposable
{
    private MechanicsEntityWorldImportCandidate? _exact;

    internal ContinuousMechanicsEntityWorldCompositionImportCandidate(
        MechanicsEntityWorldImportCandidate exact,
        IEnumerable<ContinuousMechanicsWorldImportLeaseReceipt> continuousReceipts)
    {
        _exact = exact ?? throw new ArgumentNullException(nameof(exact));
        ArgumentNullException.ThrowIfNull(continuousReceipts);
        ContinuousReceipts = Array.AsReadOnly(continuousReceipts.ToArray());
    }

    public MechanicsWorldImportLeaseReceipt ExactReceipt
        => _exact?.Receipt ?? throw new ObjectDisposedException(nameof(ContinuousMechanicsEntityWorldCompositionImportCandidate));

    public IReadOnlyList<ContinuousMechanicsWorldImportLeaseReceipt> ContinuousReceipts { get; }

    public void Publish()
    {
        MechanicsEntityWorldImportCandidate exact = _exact
            ?? throw new ObjectDisposedException(nameof(ContinuousMechanicsEntityWorldCompositionImportCandidate));
        exact.Publish();
    }

    public void Dispose()
    {
        MechanicsEntityWorldImportCandidate? exact = Interlocked.Exchange(ref _exact, null);
        exact?.Dispose();
    }
}
