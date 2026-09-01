using System.Diagnostics;
using System.Text.Json;

const int DefaultIterations = 10_000;
const int MaximumIterations = 100_000;
const int WarmupIterations = 1_000;

int iterations = args.Length == 0 ? DefaultIterations : int.Parse(args[0]);
if (iterations is < 1 or > MaximumIterations)
{
    throw new ArgumentOutOfRangeException(nameof(iterations), "iterations must be in 1..=100000");
}

var product = new ManagedProductProbe();
for (int index = 0; index < WarmupIterations; index++)
{
    product.Update(index, 1.0 / 60.0);
}

var durations = new double[iterations];
long allocatedBefore = GC.GetAllocatedBytesForCurrentThread();
for (int index = 0; index < iterations; index++)
{
    long started = Stopwatch.GetTimestamp();
    product.Update(index + WarmupIterations, 1.0 / 60.0);
    durations[index] = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
}
long allocatedBytes = GC.GetAllocatedBytesForCurrentThread() - allocatedBefore;
Array.Sort(durations);

Console.WriteLine($"RUSTY_PERF {JsonSerializer.Serialize(new
{
    schemaVersion = 1,
    lane = "managed-csharp-update",
    iterations,
    unit = "milliseconds",
    minimum = durations[0],
    median = Percentile(durations, 0.5),
    p95 = Percentile(durations, 0.95),
    maximum = durations[^1],
    mean = durations.Average(),
    allocatedBytes,
    checksum = product.Checksum,
    runtime = Environment.Version.ToString(),
})}");

static double Percentile(double[] sorted, double fraction)
{
    int index = (int)Math.Round((sorted.Length - 1) * fraction);
    return sorted[index];
}

file sealed class ManagedProductProbe
{
    private const int EntityCount = 256;
    private readonly double[] _positions = new double[EntityCount];
    private readonly double[] _velocities = Enumerable.Range(0, EntityCount)
        .Select(index => 0.25 + index * 0.001)
        .ToArray();

    public double Checksum { get; private set; }

    public void Update(int step, double deltaSeconds)
    {
        double checksum = 0;
        for (int index = 0; index < EntityCount; index++)
        {
            double velocity = _velocities[index] + Math.Sin((step + index) * 0.0001) * 0.01;
            double position = _positions[index] + velocity * deltaSeconds;
            _positions[index] = position;
            checksum += position;
        }
        Checksum = checksum;
    }
}
