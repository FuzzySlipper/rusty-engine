using Rusty.Engine;

namespace CsharpCrossoverPerformance;

/// <summary>Small demand-mode workload shared by the CoreCLR and NativeAOT crossover probes.</summary>
public sealed class Product : IEngineProduct
{
    private const uint RootNode = 0;
    private readonly IEngineContext _engine;
    private readonly UiStream _stream;
    private ulong _sequence;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        _stream = _engine.Ui.OpenStream(new UiStreamRequest(
            "crossover-performance",
            "crossover.performance.v1"));
    }

    public void Start() => Publish();

    public ProductUpdateResult Update(ProductUpdate update)
    {
        Publish();
        return ProductUpdateResult.None;
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() => _stream.Dispose();

    private void Publish()
    {
        StructuredValueNode[] nodes =
        [
            new(
                Kind: StructuredValueKind.Number,
                BoolValue: 0,
                NumberValue: _sequence,
                KeyOffset: 0,
                KeyLen: 0,
                TextOffset: 0,
                TextLen: 0,
                FirstEdge: 0,
                ChildCount: 0),
        ];
        _engine.Ui.PublishProjection(new UiProjection(
            _stream,
            ++_sequence,
            new UiValue(nodes, System.Array.Empty<uint>(), RootNode, System.Array.Empty<byte>())));
    }
}
