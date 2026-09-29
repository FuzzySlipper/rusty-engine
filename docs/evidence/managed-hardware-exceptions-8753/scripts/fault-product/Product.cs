using System;
using Rusty.Engine;

namespace HardwareFault;

// #8753 evidence product. Every update catches a null dereference and a
// division by zero, both raised by the CPU. With FAULT_AT=N and
// FAULT_KIND=null-reference|divide-by-zero, update N lets that one escape.
public sealed class Product : IEngineProduct
{
    private readonly long _faultAt;
    private readonly string _faultKind;
    private long _updates;
    private long _caught;

    public Product(ProductCreateContext context)
    {
        _faultAt = long.TryParse(Environment.GetEnvironmentVariable("FAULT_AT"), out long at) ? at : -1;
        _faultKind = Environment.GetEnvironmentVariable("FAULT_KIND") ?? "null-reference";
    }

    public void Start() { }
    public void Attach() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        _updates++;
        try { _ = Missing()!.Length; }
        catch (NullReferenceException) { _caught++; }
        try { _ = 1 / Zero(); }
        catch (DivideByZeroException) { _caught++; }
        if (_updates == _faultAt)
        {
            Console.WriteLine($"FAULT {_faultKind} at update {_updates} after {_caught} caught");
            _ = _faultKind == "divide-by-zero" ? 1 / Zero() : Missing()!.Length;
        }
        return ProductUpdateResult.None;
    }

    private static string? Missing() => Environment.GetEnvironmentVariable("RUSTY_ENGINE_NO_SUCH_VARIABLE");
    private static int Zero() => Environment.ProcessorCount - Environment.ProcessorCount;

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public bool CompleteTimeline(ProductTimelineCompletion completion) => false;
    public void Dispose() { }
}
