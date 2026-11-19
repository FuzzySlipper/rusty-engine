using System.Diagnostics;
using System.Text;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpPersistenceQueue;

/// Saves an 8 KB blob every update and times the Save call in three runs: written
/// in the call to a durable store (the baseline), queued to a durable store, and
/// queued to a cache store. After each run it waits for the queue, checks that
/// Load reads every key's newest blob and that a stale guard is refused, then
/// queues a burst in one update and times how long the worker takes per write.
public sealed class Product(ProductCreateContext context) : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const int BlobBytes = 8 * 1024;
    private const int Keys = 64;
    private const int BaselineUpdates = 120;
    private const int QueuedUpdates = 600;
    private const int BurstSaves = 256;

    private static readonly Run[] Runs =
    [
        new("written-durable", Cache: false, Deferred: false, BaselineUpdates),
        new("queued-durable", Cache: false, Deferred: true, QueuedUpdates),
        new("queued-cache", Cache: true, Deferred: true, QueuedUpdates),
    ];

    private readonly IPersistenceService persistence = context.Engine.Persistence;
    private readonly StringBuilder report = new();
    private readonly List<double> saveMilliseconds = [];
    private readonly List<double> controlMilliseconds = [];
    private readonly List<string> failures = [];
    private int run;
    private int step;
    private PersistenceStore? store;
    private readonly ulong[] revisions = new ulong[Keys];
    private long burstStarted;
    private ulong burstTicket;

    private sealed record Run(string Name, bool Cache, bool Deferred, int Updates);

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        if (run >= Runs.Length) return ProductUpdateResult.None;
        Run current = Runs[run];
        if (store is null)
        {
            store = persistence.OpenStore(new PersistenceOpenRequest(current.Name, current.Cache));
            // As a cache looks a key up before it writes one: a later save of
            // a key the runtime has loaded reads no file.
            for (int key = 0; key < Keys; key++)
            {
                using PersistenceBlob blob = persistence.Load(new PersistenceLoadRequest(store, Key(key)));
                Require(!persistence.DescribeBlob(blob).Present, $"{current.Name} {Key(key)} was already stored");
            }
        }
        if (step < current.Updates)
        {
            Save(current, step);
            // The control: an Engine call that touches no file, timed in the
            // same update, shows what the host's load alone costs a call.
            long started = Stopwatch.GetTimestamp();
            persistence.ReadSaves(store);
            controlMilliseconds.Add(Stopwatch.GetElapsedTime(started).TotalMilliseconds);
        }
        else if (step == current.Updates)
        {
            if (!Settled()) return ProductUpdateResult.None;
            Check(current);
            burstStarted = Stopwatch.GetTimestamp();
            for (int index = 0; index < BurstSaves; index++)
            {
                burstTicket = Save(current, current.Updates + index, timed: false).Ticket;
            }
        }
        else
        {
            if (!Settled()) return ProductUpdateResult.None;
            double burst = Stopwatch.GetElapsedTime(burstStarted).TotalMilliseconds;
            Finish(current, burst);
            return ProductUpdateResult.None;
        }

        step++;
        return ProductUpdateResult.None;
    }

    private PersistenceSaveReceipt Save(Run current, int index, bool timed = true)
    {
        int key = index % Keys;
        byte[] payload = Payload(index);
        long started = Stopwatch.GetTimestamp();
        // Each save names the revision it replaces; a key's first save names none.
        PersistenceRevisionGuard guard = revisions[key] == 0 ? PersistenceRevisionGuard.Absent : PersistenceRevisionGuard.Exact;
        PersistenceSaveReceipt receipt = persistence.Save(new PersistenceSaveRequest(
            store!, Key(key), guard, revisions[key], payload, current.Deferred));
        if (timed) saveMilliseconds.Add(Stopwatch.GetElapsedTime(started).TotalMilliseconds);
        Require(receipt.Outcome == (current.Deferred ? PersistenceSaveOutcome.Queued : PersistenceSaveOutcome.Saved),
            $"{current.Name} save {index} answered {receipt}");
        Require(receipt.Revision == revisions[key] + 1, $"{current.Name} save {index} took revision {receipt.Revision}");
        revisions[key] = receipt.Revision;
        return receipt;
    }

    private bool Settled()
    {
        PersistenceSaveReadout readout = persistence.ReadSaves(store!);
        Require(readout.Failed == 0, $"a queued save failed: {persistence.ReadSaveFailureText(store!)}");
        return readout.Finished == readout.Queued;
    }

    /// Load reads every key's newest blob, and a guard naming an older revision is refused.
    private void Check(Run current)
    {
        int last = current.Updates - 1;
        for (int key = 0; key < Keys; key++)
        {
            int index = last - ((last - key) % Keys + Keys) % Keys;
            using PersistenceBlob blob = persistence.Load(new PersistenceLoadRequest(store!, Key(key)));
            PersistenceBlobInfo info = persistence.DescribeBlob(blob);
            Require(info.Present && info.Revision == revisions[key], $"{current.Name} {Key(key)} loaded revision {info.Revision}, saved {revisions[key]}");
            Require(persistence.ReadBlobBytes(blob).Span.SequenceEqual(Payload(index)), $"{current.Name} {Key(key)} did not load its newest blob");
        }
        PersistenceSaveReceipt stale = persistence.Save(new PersistenceSaveRequest(
            store!, Key(0), PersistenceRevisionGuard.Exact, revisions[0] - 1, Payload(0), current.Deferred));
        Require(stale.Outcome == PersistenceSaveOutcome.RevisionConflict && stale.Revision == revisions[0], $"a stale guard answered {stale}");
    }

    private void Finish(Run current, double burstMilliseconds)
    {
        double[] sorted = [.. saveMilliseconds];
        Array.Sort(sorted);
        double[] control = [.. controlMilliseconds];
        Array.Sort(control);
        string line = string.Create(System.Globalization.CultureInfo.InvariantCulture,
            $"PERSISTENCE_QUEUE {current.Name}: saves={sorted.Length} p50={Percentile(sorted, 0.5):F3}ms p99={Percentile(sorted, 0.99):F3}ms max={sorted[^1]:F3}ms; control p99={Percentile(control, 0.99):F3}ms max={control[^1]:F3}ms; burst of {BurstSaves} written in {burstMilliseconds:F1}ms ({burstMilliseconds / BurstSaves:F3}ms per write); loads and guard checked");
        Console.WriteLine(line);
        report.AppendLine(line);
        if (current.Deferred && sorted[^1] >= 0.5)
        {
            failures.Add($"{current.Name}: a queued Save took {sorted[^1]:F3}ms");
        }
        store!.Dispose();
        store = null;
        saveMilliseconds.Clear();
        controlMilliseconds.Clear();
        Array.Clear(revisions);
        step = 0;
        run++;
        if (run == Runs.Length)
        {
            string verdict = failures.Count == 0 ? "PERSISTENCE_QUEUE_PASSED" : $"PERSISTENCE_QUEUE_FAILED {string.Join("; ", failures)}";
            Console.WriteLine(verdict);
            report.AppendLine(verdict);
        }
    }

    private static string Key(int key) => $"chunk-{key}";

    /// Each save's bytes name it, so a load shows which save it read.
    private static byte[] Payload(int index)
    {
        byte[] payload = new byte[BlobBytes];
        BitConverter.TryWriteBytes(payload, index);
        payload.AsSpan(sizeof(int)).Fill((byte)index);
        return payload;
    }

    private static double Percentile(double[] sorted, double fraction) =>
        sorted[Math.Min(sorted.Length - 1, (int)Math.Ceiling(fraction * sorted.Length) - 1)];

    private static void Require(bool condition, string failure)
    {
        if (!condition) throw new InvalidOperationException($"PERSISTENCE_QUEUE_FAILED {failure}");
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    [DebugCommand("persistence.report", Description = "Read-only: each finished run's Save timings and burst write time.")]
    public string Report() => run < Runs.Length ? $"{report}run {run + 1} of {Runs.Length}, update {step}" : report.ToString();

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() => store?.Dispose();
}
