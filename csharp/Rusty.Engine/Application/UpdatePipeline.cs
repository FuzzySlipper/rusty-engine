using Rusty.Engine;

namespace Rusty.Engine.Application;

/// <summary>Runs product callbacks once for each host-supplied update.</summary>
public sealed class UpdatePipeline
{
    private static readonly UpdatePhase[] DefaultPhases =
    [
        UpdatePhase.Input,
        UpdatePhase.Update,
        UpdatePhase.LateUpdate,
        UpdatePhase.Presentation,
    ];

    private readonly IEngineContext _engine;
    private readonly UpdatePhase[] _phases;
    private readonly Dictionary<UpdatePhase, List<ProductUpdateCallback>> _callbacks = new();

    public UpdatePipeline(IEngineContext engine, IEnumerable<UpdatePhase>? phases = null)
    {
        _engine = engine ?? throw new ArgumentNullException(nameof(engine));
        _phases = (phases ?? DefaultPhases).ToArray();
    }

    public IReadOnlyList<UpdatePhase> Phases => _phases;

    public void Register(UpdatePhase phase, ProductUpdateCallback callback)
    {
        ArgumentNullException.ThrowIfNull(callback);
        if (Array.IndexOf(_phases, phase) < 0)
        {
            throw new ArgumentException($"The phase '{phase.Name}' is not part of this pipeline.", nameof(phase));
        }

        if (!_callbacks.TryGetValue(phase, out List<ProductUpdateCallback>? callbacks))
        {
            callbacks = [];
            _callbacks.Add(phase, callbacks);
        }

        callbacks.Add(callback);
    }

    public void Run(ProductUpdate update)
    {
        foreach (UpdatePhase phase in _phases)
        {
            if (!_callbacks.TryGetValue(phase, out List<ProductUpdateCallback>? callbacks))
            {
                continue;
            }

            foreach (ProductUpdateCallback callback in callbacks)
            {
                callback(_engine, update);
            }
        }
    }
}

public delegate void ProductUpdateCallback(IEngineContext engine, ProductUpdate update);
