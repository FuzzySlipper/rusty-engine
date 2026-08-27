using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using Rusty.Engine;

namespace Rusty.Engine.NativeProduct;

internal static unsafe class Program
{
    private static readonly Dictionary<ulong, (nint Entries, nint Label, nint Payload)> Leases = [];
    private static readonly Dictionary<ulong, (nint Diagnostics, nint Code, nint Message, nint Source, nint Service, nint Operation)> DiagnosticLeases = [];
    private static ulong _nextLease = 1;
    private static int _destroyed;
    private static int _diagnosticDestroyed;

    private static void Main()
    {
        NativeLeaseFixtureApi api = new()
        {
            context = null,
            read_items = new NativeReadLeaseFixtureItems { Pointer = &ReadItems },
            destroy_item_lease = new NativeDestroyLeaseFixtureItemLease { Pointer = &DestroyItemLease },
            destroy_operation_diagnostic_lease = new NativeDestroyLeaseFixtureOperationDiagnosticLease { Pointer = &DestroyOperationDiagnosticLease },
        };
        LeaseFixtureServiceImplementation service = new(api);

        LeaseFixtureItemLeaseReceipt empty = service.ReadItems(new LeaseFixtureRequest(0));
        Require(empty.Entries.IsEmpty, "empty lease did not become an empty managed collection");
        Require(empty.Total == 0 && !empty.Truncated && empty.Completeness == LeaseFixtureCompleteness.Complete && empty.Revision == 10 && empty.ContentHash == 0 && empty.Anchor == new System.Numerics.Vector2(1, 2), "empty lease metadata was not copied");
        Require(_destroyed == 1 && Leases.Count == 0, "empty lease was not released exactly once");

        LeaseFixtureItemLeaseReceipt copied = service.ReadItems(new LeaseFixtureRequest(1));
        Require(copied.Entries.Length == 1, "one-element lease was not copied");
        Require(copied.Total == 3 && copied.Truncated && copied.Completeness == LeaseFixtureCompleteness.Truncated && copied.Revision == 11 && copied.ContentHash == 0xC0FFEE && copied.Anchor == new System.Numerics.Vector2(3, 4), "collection lease metadata was not copied");
        LeaseFixtureItem item = copied.Entries.Span[0];
        Require(item.Label == "café" && item.Ordinal == 7, "non-ASCII nested UTF-8 was not copied");
        Require(item.Payload.Span.SequenceEqual(new byte[] { 0x00, 0xC3, 0xA9, 0xFF }), "nested bytes were not copied");
        Require(_destroyed == 2 && Leases.Count == 0, "one-element lease was not released exactly once");

        try
        {
            service.ReadItems(new LeaseFixtureRequest(4));
            throw new InvalidOperationException("success with an operation diagnostic lease did not fail");
        }
        catch (InvalidOperationException error) when (error.Message.Contains("success with an operation diagnostic lease", StringComparison.Ordinal))
        {
        }
        Require(_diagnosticDestroyed == 1 && DiagnosticLeases.Count == 0 && Leases.Count == 0, "success-path diagnostic lease was not released exactly once");

        try
        {
            service.ReadItems(new LeaseFixtureRequest(2));
            throw new InvalidOperationException("rich diagnostic failure did not throw");
        }
        catch (EngineCallException error)
        {
            Require(error.Service == "LeaseFixture" && error.Operation == "ReadItems" && error.Status == -7, "stable operation identity was not copied");
            Require(error.Diagnostics.Length == 1, "owner diagnostic was not copied");
            EngineDiagnostic diagnostic = error.Diagnostics.Span[0];
            Require(diagnostic.Code == "FIXTURE_DENIED" && diagnostic.Message == "fixture rejected request" && diagnostic.Source == "fixture", "owner diagnostic fields were not copied");
        }
        Require(_diagnosticDestroyed == 2 && DiagnosticLeases.Count == 0, "rich diagnostic lease was not released exactly once");

        try
        {
            service.ReadItems(new LeaseFixtureRequest(3));
            throw new InvalidOperationException("invalid UTF-8 diagnostic did not fail copying");
        }
        catch (DecoderFallbackException)
        {
        }
        Require(_diagnosticDestroyed == 3 && DiagnosticLeases.Count == 0, "diagnostic lease was not released after managed UTF-8 copying failed");
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int ReadItems(void* _, NativeLeaseFixtureRequest request, NativeLeaseFixtureItemLease* result, NativeOperationErrorReceipt* error)
    {
        if (result is null || error is null) return 0;
        *error = default;
        if (request.include_item is 2 or 3 or 4)
        {
            byte[] codeSource = request.include_item == 3 ? [0xFF] : Encoding.UTF8.GetBytes("FIXTURE_DENIED");
            byte[] messageSource = Encoding.UTF8.GetBytes("fixture rejected request");
            byte[] serviceSource = Encoding.UTF8.GetBytes("LeaseFixture");
            byte[] operationSource = Encoding.UTF8.GetBytes("ReadItems");
            byte[] sourceSource = Encoding.UTF8.GetBytes("fixture");
            byte* code = (byte*)NativeMemory.Alloc((nuint)codeSource.Length);
            byte* message = (byte*)NativeMemory.Alloc((nuint)messageSource.Length);
            byte* service = (byte*)NativeMemory.Alloc((nuint)serviceSource.Length);
            byte* operation = (byte*)NativeMemory.Alloc((nuint)operationSource.Length);
            byte* source = (byte*)NativeMemory.Alloc((nuint)sourceSource.Length);
            NativeEngineDiagnostic* diagnostics = (NativeEngineDiagnostic*)NativeMemory.Alloc((nuint)sizeof(NativeEngineDiagnostic));
            codeSource.CopyTo(new Span<byte>(code, codeSource.Length));
            messageSource.CopyTo(new Span<byte>(message, messageSource.Length));
            serviceSource.CopyTo(new Span<byte>(service, serviceSource.Length));
            operationSource.CopyTo(new Span<byte>(operation, operationSource.Length));
            sourceSource.CopyTo(new Span<byte>(source, sourceSource.Length));
            *diagnostics = new NativeEngineDiagnostic { code = new NativeUtf8Slice { bytes = code, len = (nuint)codeSource.Length }, message = new NativeUtf8Slice { bytes = message, len = (nuint)messageSource.Length }, source = new NativeUtf8Slice { bytes = source, len = (nuint)sourceSource.Length } };
            ulong diagnosticHandle = _nextLease++;
            DiagnosticLeases.Add(diagnosticHandle, ((nint)diagnostics, (nint)code, (nint)message, (nint)source, (nint)service, (nint)operation));
            *error = new NativeOperationErrorReceipt { service = new NativeUtf8Slice { bytes = service, len = (nuint)serviceSource.Length }, operation = new NativeUtf8Slice { bytes = operation, len = (nuint)operationSource.Length }, status = request.include_item == 4 ? 1 : -7, diagnostics = new NativeEngineDiagnosticLease { handle = new NativeEngineDiagnosticLeaseHandle { value = diagnosticHandle }, diagnostics = diagnostics, diagnostics_len = 1 } };
            if (request.include_item == 4) return 1;
            return 0;
        }
        ulong handle = _nextLease++;
        if (request.include_item == 0)
        {
            Leases.Add(handle, default);
            *result = new NativeLeaseFixtureItemLease
            {
                handle = new NativeLeaseFixtureItemLeaseHandle { value = handle },
                entries = null,
                entries_len = 0,
                total = 0,
                truncated = 0,
                completeness = NativeLeaseFixtureCompleteness.NativeLeaseFixtureCompleteness_Complete,
                revision = 10,
                content_hash = 0,
                anchor = new NativeVec2 { x = 1, y = 2 },
            };
            return 1;
        }

        byte[] labelSource = Encoding.UTF8.GetBytes("café");
        byte[] payloadSource = [0x00, 0xC3, 0xA9, 0xFF];
        byte* label = (byte*)NativeMemory.Alloc((nuint)labelSource.Length);
        byte* payload = (byte*)NativeMemory.Alloc((nuint)payloadSource.Length);
        NativeLeaseFixtureItem* entries = (NativeLeaseFixtureItem*)NativeMemory.Alloc((nuint)sizeof(NativeLeaseFixtureItem));
        labelSource.CopyTo(new Span<byte>(label, labelSource.Length));
        payloadSource.CopyTo(new Span<byte>(payload, payloadSource.Length));
        *entries = new NativeLeaseFixtureItem
        {
            label = new NativeUtf8Slice { bytes = label, len = (nuint)labelSource.Length },
            payload = new NativeByteSlice { bytes = payload, len = (nuint)payloadSource.Length },
            ordinal = 7,
        };
        Leases.Add(handle, ((nint)entries, (nint)label, (nint)payload));
        *result = new NativeLeaseFixtureItemLease
        {
            handle = new NativeLeaseFixtureItemLeaseHandle { value = handle },
            entries = entries,
            entries_len = 1,
            total = 3,
            truncated = 1,
            completeness = NativeLeaseFixtureCompleteness.NativeLeaseFixtureCompleteness_Truncated,
            revision = 11,
            content_hash = 0xC0FFEE,
            anchor = new NativeVec2 { x = 3, y = 4 },
        };
        return 1;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int DestroyItemLease(void* _, NativeLeaseFixtureItemLeaseHandle handle)
    {
        if (!Leases.Remove(handle.value, out (nint Entries, nint Label, nint Payload) lease)) return 0;
        if (lease.Entries != 0) NativeMemory.Free((void*)lease.Entries);
        if (lease.Label != 0) NativeMemory.Free((void*)lease.Label);
        if (lease.Payload != 0) NativeMemory.Free((void*)lease.Payload);
        _destroyed++;
        return 1;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int DestroyOperationDiagnosticLease(void* _, NativeEngineDiagnosticLeaseHandle handle)
    {
        if (!DiagnosticLeases.Remove(handle.value, out (nint Diagnostics, nint Code, nint Message, nint Source, nint Service, nint Operation) lease)) return 0;
        NativeMemory.Free((void*)lease.Diagnostics);
        NativeMemory.Free((void*)lease.Code);
        NativeMemory.Free((void*)lease.Message);
        NativeMemory.Free((void*)lease.Source);
        NativeMemory.Free((void*)lease.Service);
        NativeMemory.Free((void*)lease.Operation);
        _diagnosticDestroyed++;
        return 1;
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
