using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpAudioContainers;

public sealed class Product(ProductCreateContext context) : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private readonly List<AudioClip> clips = [];
    private readonly List<AudioVoice> voices = [];
    private ulong nextSignalId;
    public void Start()
    {
        foreach (string extension in new[] { "wav", "ogg", "opus", "mp3", "flac" })
        {
            string path = $"content/tone.{extension}";
            AudioClip clip = context.Engine.Audio.OpenClip(new(path));
            using ContentReference content = context.Engine.Content.OpenReference(new($"tone.{extension}"));
            using AudioClip same = context.Engine.Audio.OpenClipFromContent(new(content));
            if (clip.Handle != same.Handle) throw new InvalidOperationException("content and path admission diverged");
            clips.Add(clip);
        }
    }
    [DebugCommand("audio.proof.play", Description = "Emit and loop each admitted container through the ordinary Audio service.")]
    public string Play()
    {
        Stop();
        for (int index = 0; index < clips.Count; index++)
        {
            AudioSourceDescriptor descriptor = new(clips[index], AudioBus.Ambient, 0.08f, 1, true, 0, 1, 0, AudioEmitterKind.Global2d, Vector3.Zero, 0, Vector3.Zero);
            voices.Add(context.Engine.Audio.CreateVoice(descriptor));
            context.Engine.Audio.Emit(new($"container-{nextSignalId++}", descriptor with { Looping = false }));
        }
        return Inspect();
    }
    [DebugCommand("audio.proof.inspect", Description = "Read admission and browser realization facts.")]
    public string Inspect()
    {
        AudioRealizationReadout realization = context.Engine.Audio.ReadRealization();
        List<string> facts = [];
        for (uint index = 0; index < realization.RetainedFactCount; index++)
            facts.Add(context.Engine.Audio.ReadRealizationFactAt(new(index)).ToString());
        return $"{context.Engine.Audio.Read()};realization={realization};facts={string.Join(";", facts)}";
    }
    [DebugCommand("audio.proof.stop")]
    public void Stop() { foreach (AudioVoice voice in voices) voice.Dispose(); voices.Clear(); }
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Attach() { }
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { Stop(); foreach (AudioClip clip in clips) clip.Dispose(); clips.Clear(); }
}
