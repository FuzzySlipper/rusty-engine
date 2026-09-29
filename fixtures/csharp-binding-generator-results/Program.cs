using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using Rusty.Engine;

namespace Rusty.Engine.NativeProduct;

// Exercises the generated copy of borrowed results against a hand-written
// native table. Like a Rust service bridge, the fixture keeps only its latest
// result or refusal and poisons and frees it on the next call, so a wrapper
// that returned before copying would read freed memory.
internal static unsafe class Program
{
    private static readonly List<(nint Pointer, nuint Length)> Latest = [];
    private static readonly HashSet<ulong> OwnedFixtures = [];
    private static readonly List<(string Value, byte[] Payload)> ReplacedTags = [];
    private static ulong _nextOwned = 1;
    private static int _ownedFixtureDestroyed;

    private static void Main()
    {
        NativeResultFixtureApi api = new()
        {
            context = null,
            read_items = new NativeReadResultFixtureItems { Pointer = &ReadItems },
            read_owned_fixture = new NativeReadOwnedFixture { Pointer = &ReadOwnedFixture },
            read_invalid_owned_fixture = new NativeReadInvalidOwnedFixture { Pointer = &ReadInvalidOwnedFixture },
            read_optional_owned_fixture = new NativeReadOptionalOwnedFixture { Pointer = &ReadOptionalOwnedFixture },
            replace_tags = new NativeReplaceResultFixtureTags { Pointer = &ReplaceTags },
            destroy_owned_fixture = new NativeDestroyOwnedFixture { Pointer = &DestroyOwnedFixture },
        };
        ResultFixtureServiceImplementation service = new(api);

        OwnedFixtureInfo requiredOwned = service.ReadOwnedFixture();
        Require(requiredOwned.Handle.Handle.Value != 0 && requiredOwned.Revision == 41 && requiredOwned.Label == "owned", "required owned output field was not retained");
        requiredOwned.Handle.Dispose();
        Require(_ownedFixtureDestroyed == 1 && OwnedFixtures.Count == 0, "required owned output field was not released exactly once");

        try
        {
            service.ReadInvalidOwnedFixture();
            throw new InvalidOperationException("owned output conversion did not reject an invalid inline label");
        }
        catch (InvalidOperationException error) when (error.Message.Contains("Inline animation feedback text", StringComparison.Ordinal))
        {
        }
        Require(_ownedFixtureDestroyed == 2 && OwnedFixtures.Count == 0, "failed owned output conversion did not release its handle");

        OptionalOwnedFixtureReceipt missingOptional = service.ReadOptionalOwnedFixture(0);
        Require(missingOptional.Handle is null && missingOptional.AdmittedCount == 0, "zero optional owned field was not preserved as null");
        OptionalOwnedFixtureReceipt admittedOptional = service.ReadOptionalOwnedFixture(1);
        Require(admittedOptional.Handle is not null && admittedOptional.AdmittedCount == 1, "optional owned output field was not retained");
        OwnedFixture admittedHandle = admittedOptional.Handle ?? throw new InvalidOperationException("admitted optional handle was absent");
        admittedHandle.Dispose();
        Require(_ownedFixtureDestroyed == 3 && OwnedFixtures.Count == 0, "optional owned output field was not released exactly once");

        byte[] payload = [0x00, 0xC3, 0xA9, 0xFF];
        service.ReplaceTags(new ReplaceResultFixtureTagsRequest(new ResultFixtureTag[] {
            new ResultFixtureTag("café", payload),
            new ResultFixtureTag(string.Empty, ReadOnlyMemory<byte>.Empty),
        }));
        payload[0] = 0x7F;
        Require(ReplacedTags.Count == 2, "borrowed tag input count was not delivered");
        Require(ReplacedTags[0].Value == "café" && ReplacedTags[0].Payload.SequenceEqual(new byte[] { 0x00, 0xC3, 0xA9, 0xFF }), "non-ASCII UTF-8 and byte input were not copied synchronously");
        Require(ReplacedTags[1].Value == string.Empty && ReplacedTags[1].Payload.Length == 0, "empty borrowed UTF-8 and bytes were not delivered");

        ResultFixtureItemResult empty = service.ReadItems(new ResultFixtureRequest(0));
        Require(empty.Entries.IsEmpty, "empty result did not become an empty managed collection");
        Require(empty.Observations.IsEmpty, "empty secondary result collection did not become an empty managed collection");
        Require(empty.Total == 0 && !empty.Truncated && empty.Completeness == ResultFixtureCompleteness.Complete && empty.Revision == 10 && empty.ContentHash == 0 && empty.Anchor == new System.Numerics.Vector2(1, 2), "empty result metadata was not copied");

        ResultFixtureItemResult copied = service.ReadItems(new ResultFixtureRequest(1));
        // The next call poisons and frees the fixture's storage for `copied`.
        _ = service.ReadItems(new ResultFixtureRequest(0));
        Require(copied.Entries.Length == 1, "one-element result was not copied");
        Require(copied.Observations.Length == 2, "secondary result collection was not copied");
        Require(copied.Total == 3 && copied.Truncated && copied.Completeness == ResultFixtureCompleteness.Truncated && copied.Revision == 11 && copied.ContentHash == 0xC0FFEE && copied.Anchor == new System.Numerics.Vector2(3, 4), "collection result metadata was not copied");
        ResultFixtureItem item = copied.Entries.Span[0];
        Require(item.Label == "café" && item.Ordinal == 7, "non-ASCII nested UTF-8 was not copied");
        Require(item.Payload.Span.SequenceEqual(new byte[] { 0x00, 0xC3, 0xA9, 0xFF }), "nested bytes were not copied");
        Require(copied.Observations.Span[0] == new ResultFixtureObservation(21, 3) && copied.Observations.Span[1] == new ResultFixtureObservation(34, 5), "secondary collection values were not copied");

        try
        {
            service.ReadItems(new ResultFixtureRequest(2));
            throw new InvalidOperationException("rich diagnostic failure did not throw");
        }
        catch (EngineCallException error)
        {
            Require(error.Service == "ResultFixture" && error.Operation == "ReadItems" && error.Status == 0, "generated operation identity was not used");
            Require(error.Diagnostics.Length == 1, "owner diagnostic was not copied");
            Require(error.Message.Contains("FIXTURE_DENIED: fixture rejected request", StringComparison.Ordinal), "exception message lost the native reason");
            EngineDiagnostic diagnostic = error.Diagnostics.Span[0];
            Require(diagnostic.Code == "FIXTURE_DENIED" && diagnostic.Message == "fixture rejected request" && diagnostic.Source == "fixture", "owner diagnostic fields were not copied");
        }

        try
        {
            service.ReadItems(new ResultFixtureRequest(3));
            throw new InvalidOperationException("invalid UTF-8 diagnostic did not fail copying");
        }
        catch (DecoderFallbackException)
        {
        }
        Console.WriteLine("BINDING_GENERATOR_RESULT_FIXTURE_PASSED");
    }

    private static byte* Keep(ReadOnlySpan<byte> source)
    {
        byte* copy = KeepArray<byte>(source.Length);
        source.CopyTo(new Span<byte>(copy, source.Length));
        return copy;
    }

    private static T* KeepArray<T>(int count) where T : unmanaged
    {
        nuint length = (nuint)(Math.Max(count, 1) * sizeof(T));
        T* copy = (T*)NativeMemory.Alloc(length);
        Latest.Add(((nint)copy, length));
        return copy;
    }

    // Poisons and frees the previous result's storage, as a bridge's next
    // call replaces it.
    private static void ReplaceLatest()
    {
        foreach ((nint pointer, nuint length) in Latest)
        {
            NativeMemory.Fill((void*)pointer, length, 0xA5);
            NativeMemory.Free((void*)pointer);
        }
        Latest.Clear();
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int ReadOwnedFixture(void* _, NativeOwnedFixtureInfo* result)
    {
        if (result is null) return 0;
        ulong handle = _nextOwned++;
        OwnedFixtures.Add(handle);
        *result = new NativeOwnedFixtureInfo
        {
            handle = new NativeOwnedFixtureHandle { value = handle },
            revision = 41,
            label = FeedbackText("owned"),
        };
        return 1;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int ReadInvalidOwnedFixture(void* _, NativeOwnedFixtureInfo* result)
    {
        if (result is null) return 0;
        ulong handle = _nextOwned++;
        OwnedFixtures.Add(handle);
        NativeAnimationFeedbackText invalidLabel = default;
        invalidLabel.len = 97;
        *result = new NativeOwnedFixtureInfo
        {
            handle = new NativeOwnedFixtureHandle { value = handle },
            revision = 42,
            label = invalidLabel,
        };
        return 1;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int ReadOptionalOwnedFixture(void* _, uint admitted, NativeOptionalOwnedFixtureReceipt* result)
    {
        if (result is null || admitted > 1) return 0;
        if (admitted == 0)
        {
            *result = new NativeOptionalOwnedFixtureReceipt { admitted_count = 0 };
            return 1;
        }
        ulong handle = _nextOwned++;
        OwnedFixtures.Add(handle);
        *result = new NativeOptionalOwnedFixtureReceipt
        {
            handle = new NativeOwnedFixtureHandle { value = handle },
            admitted_count = 1,
        };
        return 1;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int ReplaceTags(void* _, NativeReplaceResultFixtureTagsRequest* request)
    {
        if (request is null || request->tags_len != 2 || request->tags is null) return 0;
        ReplacedTags.Clear();
        for (int index = 0; index < checked((int)request->tags_len); index++)
        {
            NativeResultFixtureTag tag = request->tags[index];
            if ((tag.value.len != 0 && tag.value.bytes is null) || (tag.payload.len != 0 && tag.payload.bytes is null)) return 0;
            string value = tag.value.len == 0 ? string.Empty : new UTF8Encoding(false, true).GetString(new ReadOnlySpan<byte>(tag.value.bytes, checked((int)tag.value.len)));
            byte[] payload = tag.payload.len == 0 ? [] : new ReadOnlySpan<byte>(tag.payload.bytes, checked((int)tag.payload.len)).ToArray();
            ReplacedTags.Add((value, payload));
        }
        return 1;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int ReadItems(void* _, NativeResultFixtureRequest request, NativeResultFixtureItemResult* result, NativeOperationErrorReceipt* error)
    {
        if (result is null || error is null) return 0;
        ReplaceLatest();
        *error = default;
        if (request.include_item is 2 or 3)
        {
            byte[] codeSource = request.include_item == 3 ? [0xFF] : Encoding.UTF8.GetBytes("FIXTURE_DENIED");
            byte[] messageSource = Encoding.UTF8.GetBytes("fixture rejected request");
            byte[] sourceSource = Encoding.UTF8.GetBytes("fixture");
            NativeEngineDiagnostic* diagnostics = KeepArray<NativeEngineDiagnostic>(1);
            *diagnostics = new NativeEngineDiagnostic
            {
                code = new NativeUtf8Slice { bytes = Keep(codeSource), len = (nuint)codeSource.Length },
                message = new NativeUtf8Slice { bytes = Keep(messageSource), len = (nuint)messageSource.Length },
                source = new NativeUtf8Slice { bytes = Keep(sourceSource), len = (nuint)sourceSource.Length },
            };
            *error = new NativeOperationErrorReceipt { diagnostics = diagnostics, diagnostics_len = 1 };
            return 0;
        }
        if (request.include_item == 0)
        {
            *result = new NativeResultFixtureItemResult
            {
                entries = null,
                entries_len = 0,
                total = 0,
                truncated = 0,
                completeness = NativeResultFixtureCompleteness.NativeResultFixtureCompleteness_Complete,
                revision = 10,
                content_hash = 0,
                anchor = new NativeVec2 { x = 1, y = 2 },
            };
            return 1;
        }

        byte[] labelSource = Encoding.UTF8.GetBytes("café");
        byte[] payloadSource = [0x00, 0xC3, 0xA9, 0xFF];
        NativeResultFixtureItem* entries = KeepArray<NativeResultFixtureItem>(1);
        NativeResultFixtureObservation* observations = KeepArray<NativeResultFixtureObservation>(2);
        *entries = new NativeResultFixtureItem
        {
            label = new NativeUtf8Slice { bytes = Keep(labelSource), len = (nuint)labelSource.Length },
            payload = new NativeByteSlice { bytes = Keep(payloadSource), len = (nuint)payloadSource.Length },
            ordinal = 7,
        };
        observations[0] = new NativeResultFixtureObservation { revision = 21, kind = 3 };
        observations[1] = new NativeResultFixtureObservation { revision = 34, kind = 5 };
        *result = new NativeResultFixtureItemResult
        {
            entries = entries,
            entries_len = 1,
            observations = observations,
            observations_len = 2,
            total = 3,
            truncated = 1,
            completeness = NativeResultFixtureCompleteness.NativeResultFixtureCompleteness_Truncated,
            revision = 11,
            content_hash = 0xC0FFEE,
            anchor = new NativeVec2 { x = 3, y = 4 },
        };
        return 1;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int DestroyOwnedFixture(void* _, NativeOwnedFixtureHandle handle)
    {
        if (!OwnedFixtures.Remove(handle.value)) return 0;
        _ownedFixtureDestroyed++;
        return 1;
    }

    private static NativeAnimationFeedbackText FeedbackText(string value)
    {
        byte[] bytes = Encoding.UTF8.GetBytes(value);
        NativeAnimationFeedbackText result = default;
        result.len = (nuint)bytes.Length;
        bytes.CopyTo(MemoryMarshal.CreateSpan(ref result.bytes.e0, bytes.Length));
        return result;
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
