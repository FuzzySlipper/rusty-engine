using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using Rusty.Engine;

namespace Rusty.Engine.NativeProduct;

internal static unsafe class Program
{
    private static readonly Dictionary<ulong, (nint Entries, nint Label, nint Payload)> Leases = [];
    private static ulong _nextLease = 1;
    private static int _destroyed;

    private static void Main()
    {
        NativeLeaseFixtureApi api = new()
        {
            context = null,
            read_items = new NativeReadLeaseFixtureItems { Pointer = &ReadItems },
            destroy_item_lease = new NativeDestroyLeaseFixtureItemLease { Pointer = &DestroyItemLease },
        };
        LeaseFixtureServiceImplementation service = new(api);

        Require(service.ReadItems(new LeaseFixtureRequest(0)).IsEmpty, "empty lease did not become an empty managed collection");
        Require(_destroyed == 1 && Leases.Count == 0, "empty lease was not released exactly once");

        ReadOnlyMemory<LeaseFixtureItem> copied = service.ReadItems(new LeaseFixtureRequest(1));
        Require(copied.Length == 1, "one-element lease was not copied");
        LeaseFixtureItem item = copied.Span[0];
        Require(item.Label == "café" && item.Ordinal == 7, "non-ASCII nested UTF-8 was not copied");
        Require(item.Payload.Span.SequenceEqual(new byte[] { 0x00, 0xC3, 0xA9, 0xFF }), "nested bytes were not copied");
        Require(_destroyed == 2 && Leases.Count == 0, "one-element lease was not released exactly once");
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int ReadItems(void* _, NativeLeaseFixtureRequest request, NativeLeaseFixtureItemLease* result)
    {
        if (result is null) return 0;
        ulong handle = _nextLease++;
        if (request.include_item == 0)
        {
            Leases.Add(handle, default);
            *result = new NativeLeaseFixtureItemLease
            {
                handle = new NativeLeaseFixtureItemLeaseHandle { value = handle },
                entries = null,
                entries_len = 0,
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

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
