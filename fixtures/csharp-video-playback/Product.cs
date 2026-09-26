using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;
namespace CsharpVideoPlayback;
public sealed class Product(ProductCreateContext context) : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private VideoPlaybackHandle active;
    [DebugCommand("video.proof.play")]
    public string Play()
    {
        active = context.Engine.Video.Play(new("content/proof.webm"));
        return $"playback={active.Value}";
    }
    [DebugCommand("video.proof.fail")]
    public string Fail()
    {
        active = context.Engine.Video.Play(new("content/invalid.webm"));
        return $"playback={active.Value}";
    }
    [DebugCommand("video.proof.skip")]
    public void Skip() => context.Engine.Video.Skip(active);
    [DebugCommand("diagnostics.proof")]
    public string Diagnostics()
    {
        using AudioClip unknownClip = new(new(ulong.MaxValue), static () => { });
        AudioSourceDescriptor source = new(unknownClip, AudioBus.Ambient, 1, 1, false,
            0, 1, 0, AudioEmitterKind.Global2d, Vector3.Zero, 0, Vector3.Zero);
        Check(() => context.Engine.Audio.CreateVoice(source), "Audio", "CSHARP_AUDIO_CLIP_HANDLE");
        Check(() => context.Engine.Graphics.OpenResource(new("content/missing.png", TextureFilter.Nearest, TextureWrap.Clamp)), "Graphics", "CSHARP_RENDER_RESOURCE_UNKNOWN");
        return "diagnostics validated";
    }
    private void Check(Action action, string service, string expectedCode)
    {
        try { action(); }
        catch (EngineCallException error)
        {
            if (error.Service != service || error.Diagnostics.IsEmpty)
                throw new InvalidOperationException("Engine reason was lost", error);
            EngineDiagnostic diagnostic = error.Diagnostics.Span[0];
            if (diagnostic.Code != expectedCode || string.IsNullOrWhiteSpace(diagnostic.Message)
                || !error.Message.Contains(diagnostic.Code, StringComparison.Ordinal))
                throw new InvalidOperationException("Engine reason was not copied", error);
            Console.WriteLine($"FIXTURE_NATIVE_REASON {service}: {error.Message}");
            context.Engine.Diagnostics.Publish(new(DiagnosticsSeverity.Info, DiagnosticsDisposition.Accepted,
                "diagnostics-fixture", "FIXTURE_NATIVE_REASON", error.Message, service));
            return;
        }
        throw new InvalidOperationException("Deliberately missing resource was accepted");
    }
    [DebugCommand("video.proof.inspect")]
    public string Inspect()
    {
        VideoRealizationReadout readout = context.Engine.Video.ReadRealization();
        List<string> facts = [];
        for (uint index = 0; index < readout.RetainedFactCount; index++)
            facts.Add(context.Engine.Video.ReadRealizationFactAt(new(index)).ToString());
        return $"active={active.Value};readout={readout};facts={string.Join(";", facts)}";
    }
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public void Attach() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { }
}
