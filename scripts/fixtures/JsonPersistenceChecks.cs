using System;
using System.Collections.Generic;
using System.Text.Json;
using System.Text.Json.Serialization;
using Rusty.Engine;
using Rusty.Engine.Persistence;

// Runs inside the packaged Product, using its actual generated Engine services.
internal static class JsonPersistenceChecks
{
    public static void Run(IEngineContext engine)
    {
        const string scope = "json-roundtrip";
        var codec = new JsonProductStateCodec<SavedJourney>(JourneyJson.Default.SavedJourney);
        var expected = new SavedJourney("North", [new SavedStop("Harbor", true), new SavedStop("Hill", false)],
            new Dictionary<string, int> { ["rations"] = 6, ["rope"] = 2 });
        ulong revision;
        using (var first = new ProductStateStore<SavedJourney>(engine, scope, codec))
        {
            if (first.Load("absent").Present) throw new InvalidOperationException("Missing save was present.");
            revision = first.Save("journey", expected).Revision;
            ulong disposableRevision = first.Save("discarded", expected).Revision;
            if (first.Delete("discarded", PersistenceRevisionGuard.Exact, disposableRevision + 1).Outcome
                != PersistenceDeleteOutcome.RevisionConflict || !first.Load("discarded").Present)
                throw new InvalidOperationException("Stale deletion changed saved state.");
            PersistenceDeleteReceipt deleted = first.Delete("discarded", PersistenceRevisionGuard.Exact, disposableRevision);
            if (deleted.Outcome != PersistenceDeleteOutcome.Deleted || deleted.Revision != disposableRevision)
                throw new InvalidOperationException("Deletion did not report the removed revision.");
        }
        using (var reopened = new ProductStateStore<SavedJourney>(engine, scope, codec))
        {
            if (reopened.Load("discarded").Present || reopened.Delete("discarded").Outcome != PersistenceDeleteOutcome.Missing)
                throw new InvalidOperationException("Deletion did not survive reopening.");
            var loaded = reopened.Load("journey");
            var value = loaded.State;
            if (!loaded.Present || loaded.Revision != revision || value is null
                || value.Title != "North" || value.Stops.Count != 2
                || value.Stops[0].Name != "Harbor" || !value.Stops[0].Reached
                || value.Stops[1].Name != "Hill" || value.Stops[1].Reached
                || value.Supplies["rations"] != 6 || value.Supplies["rope"] != 2)
                throw new InvalidOperationException("Disk JSON roundtrip lost state.");
            using var raw = engine.Persistence.OpenStore(new PersistenceOpenRequest(scope));
            engine.Persistence.Save(new PersistenceSaveRequest(raw, "malformed",
                PersistenceRevisionGuard.Any, 0, "{bad json"u8.ToArray()));
            try
            {
                reopened.Load("malformed");
                throw new InvalidOperationException("Malformed JSON was accepted.");
            }
            catch (JsonException) { }
        }
        // test-csharp-release-pair.sh seeds these files; each refusal leaves them unchanged.
        using var refusals = new ProductStateStore<SavedJourney>(engine, "storage-refusals", codec);
        ExpectRefusal(() => refusals.Load("retired"), PersistenceStorageFailure.UnrecognizedContainer);
        ExpectRefusal(() => refusals.Save("retired", expected), PersistenceStorageFailure.UnrecognizedContainer);
        ExpectRefusal(() => refusals.Delete("retired"), PersistenceStorageFailure.UnrecognizedContainer);
        ExpectRefusal(() => refusals.Load("malformed"), PersistenceStorageFailure.MalformedContainer);
        ExpectRefusal(() => refusals.Save("malformed", expected), PersistenceStorageFailure.MalformedContainer);
        ExpectRefusal(() => refusals.Load("unreadable"), PersistenceStorageFailure.Io);
        ExpectRefusal(() => refusals.Save("unreadable", expected), PersistenceStorageFailure.Io);
        if (refusals.Save("fresh", expected).Outcome != PersistenceSaveOutcome.Saved)
            throw new InvalidOperationException("A refused key stopped the store from saving others.");
    }

    private static void ExpectRefusal(Func<object> operation, PersistenceStorageFailure failure)
    {
        try { operation(); }
        catch (PersistenceStorageException refusal) when (refusal.Failure == failure) { return; }
        throw new InvalidOperationException($"Stored file was not refused as {failure}.");
    }
}

internal sealed record SavedStop(string Name, bool Reached);
internal sealed record SavedJourney(string Title, List<SavedStop> Stops, Dictionary<string, int> Supplies);
[JsonSerializable(typeof(SavedJourney))]
internal partial class JourneyJson : JsonSerializerContext { }
