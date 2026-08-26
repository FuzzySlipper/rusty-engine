using System.Numerics;
using Rusty.Engine;

namespace CsharpNativeAotTrial;

// This project owns only trusted product meaning. The composition project
// receives the generated native ABI and exports as internal source.
public sealed class Product : IEngineProduct
{
    private readonly IEngineContext _engine;
    private readonly Rng _rng;
    private readonly Rng _forkedRng;
    private readonly SpatialSession _spatial;
    private readonly UiStreamHandle _uiStream;
    private readonly AppearanceHandle _appearance;
    private int _turns;
    private float _x;
    private bool _started;
    private bool _paused;
    private bool _shutdown;
    private ulong _uiSequence;
    private ulong _lastRandom;
    private LookState _look;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        _appearance = _engine.Appearance.CreatePrimitive(new PrimitiveAppearanceRequest(1, 0, new Color(0.25f, 0.75f, 1.0f, 1.0f)));
        KeyedRngReceipt keyed = _engine.Random.DrawKeyed(new KeyedRngRequest(17, "nativeaot-trial", "create", -10, 10));
        if (keyed != _engine.Random.DrawKeyed(new KeyedRngRequest(17, "nativeaot-trial", "create", -10, 10)))
        {
            throw new InvalidOperationException("keyed random sequence changed during creation");
        }
        _rng = _engine.Random.CreateScoped(new ScopedRngCreateRequest(17, "nativeaot-trial"));
        _forkedRng = _engine.Random.ForkScoped(new ScopedRngForkRequest(_rng, "child"));
        _lastRandom = _engine.Random.NextU64(_forkedRng).Value;
        _uiStream = _engine.Ui.OpenStream(new UiStreamRequest("nativeaot-trial", "nativeaot.trial.hud"));
        _spatial = _engine.Spatial.CreateSession(new SpatialSessionConfig(1.0, 16, 0));
        _engine.Spatial.ReplaceNavigation(new NavigationReplaceRequest(
            _spatial,
            new PlanarNavConfig(1, 1.0, 16, 0),
            new[] { new PlanarNavCell(0, 0, 0), new PlanarNavCell(1, 0, 0) }));
        _ = _engine.Spatial.ProposeNavigationStep(new NavigationStepRequest(
            _spatial,
            new Vector3(0.5f),
            new Vector3(1.5f, 0.5f, 0.5f),
            0.5f,
            16));
    }

    public void Start()
    {
        _started = true;
        _paused = false;
        PublishPresentation();
    }

    public void Update(ProductUpdate update)
    {
        if (!_started || _paused || _shutdown)
        {
            throw new InvalidOperationException("the product is not accepting updates");
        }
        foreach (ProductInputEvent input in update.Input)
        {
            if (input.Kind == 1 && input.Edge == 1 && input.Label.Span.SequenceEqual("KeyW"u8))
            {
                _x += 1.0f;
            }
            if (input.Kind == 3)
            {
                _look = _engine.Look.Integrate(new LookRequest(_look, new Vector2(input.X, input.Y), LookConfig())).State;
            }
        }
        _turns++;
        _lastRandom = _engine.Random.NextBoundedU32(new ScopedRngBoundedRequest(_rng, 100)).Value;
        PublishPresentation();
    }

    public void Pause() => _paused = true;
    public void Resume() => _paused = false;
    public void Shutdown() => _shutdown = true;

    public void Dispose()
    {
        _ = _engine.Random.NextBool(_rng);
        _forkedRng.Dispose();
        _rng.Dispose();
        _spatial.Dispose();
    }

    private static LookConfig LookConfig() => new(0.01f, 0.01f, -1.4f, 1.4f, 1.0f, 0, 0, 1, 0);

    private void PublishPresentation()
    {
        PublishAppearanceSnapshot();
        _engine.Ui.PublishProjection(new UiProjection(_uiStream, ++_uiSequence, UiValue()));
    }

    private UiValue UiValue()
    {
        StructuredValueNode[] nodes =
        [
            new(5, 0, 0, 0, 0, 0, 0, 0, 3),
            new(2, 0, _turns, 0, 5, 0, 0, 0, 0),
            new(2, 0, _look.YawRadians, 5, 3, 0, 0, 0, 0),
            new(2, 0, _x, 8, 1, 0, 0, 0, 0),
        ];
        return new UiValue(nodes, new uint[] { 1, 2, 3 }, 0, "turnsyawx"u8.ToArray());
    }

    private void PublishAppearanceSnapshot()
    {
        if (_turns >= 2)
        {
            _engine.Appearance.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
            return;
        }
        _engine.Appearance.PublishSnapshot(
        [
            new AppearanceFact(41, new Transform(new Vector3(_x, 0, 0), Quaternion.Identity, Vector3.One), _appearance, 1, 0),
        ]);
    }
}
