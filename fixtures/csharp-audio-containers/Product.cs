using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpAudioContainers;

public sealed class Product(ProductCreateContext context) : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private readonly List<AudioClip> clips = [];
    private readonly List<AudioVoice> voices = [];
    private readonly List<AudioVoice> spatialVoices = [];
    private const ulong EmitterEntity = 900;
    private const float SpatialAttenuation = 20;
    private static readonly Vector3 WorldEmitter = new(3, 0, 0);
    private Camera? camera;
    private Appearance? marker;
    private AudioVoice? entityVoice;
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
        camera = context.Engine.CameraView.CreateCamera(CameraAt(0));
        context.Engine.CameraView.SetActiveCamera(camera);
        marker = context.Engine.Graphics.CreatePrimitive(new PrimitiveAppearanceRequest(PrimitiveGeometry.Cube, false, new Color(1, 0.5f, 0.2f, 1)));
    }
    private static CameraDescriptor CameraAt(float yawDegrees) => new(new CameraPose(Vector3.Zero, 0, yawDegrees), CameraBasisMode.Derived, default,
        new CameraProjection(CameraProjectionKind.Perspective, 60, 0, 0.1f, 100), CameraViewports.Full);
    private AudioSourceDescriptor Spatial(AudioEmitterKind kind, Vector3 position, ulong entity) =>
        new(clips[0], AudioBus.Ambient, 0.5f, 1, true, 1, SpatialAttenuation, 0, kind, position, entity, Vector3.Zero);
    [DebugCommand("audio.proof.face", Description = "Turn the active camera (the audio listener) to a yaw in degrees; 0 faces -Z, 90 faces +X.")]
    public string Face(float yawDegrees)
    {
        context.Engine.CameraView.UpdateCamera(new CameraUpdateRequest(camera!, CameraAt(yawDegrees)));
        return $"camera yaw {yawDegrees}";
    }
    [DebugCommand("audio.proof.world", Description = "Loop the WAV tone from a world-positioned emitter at (3, 0, 0).")]
    public string World()
    {
        spatialVoices.Add(context.Engine.Audio.CreateVoice(Spatial(AudioEmitterKind.World3d, WorldEmitter, 0)));
        return Inspect();
    }
    [DebugCommand("audio.proof.entity", Description = "Publish entity 900 as a cube at (x, 0, z) and loop the WAV tone attached to it.")]
    public string Entity(float x, float z)
    {
        context.Engine.Graphics.PublishSnapshot([new AppearanceFact(EmitterEntity, false, 0,
            new Transform(new Vector3(x, 0, z), Quaternion.Identity, Vector3.One), marker!, true, RenderLayer.Scene)]);
        entityVoice ??= context.Engine.Audio.CreateVoice(Spatial(AudioEmitterKind.EntityAttached, Vector3.Zero, EmitterEntity));
        return $"entity {EmitterEntity} at ({x}, 0, {z})";
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
    public void Stop()
    {
        foreach (AudioVoice voice in voices.Concat(spatialVoices)) voice.Dispose();
        voices.Clear();
        spatialVoices.Clear();
        entityVoice?.Dispose();
        entityVoice = null;
        context.Engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
    }
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public ProductUpdateResult Update(ProductUpdate update) => ProductUpdateResult.None;
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose()
    {
        Stop();
        foreach (AudioClip clip in clips) clip.Dispose();
        clips.Clear();
        marker?.Dispose();
        context.Engine.CameraView.ClearActiveCamera(new ClearActiveCameraRequest(0));
        camera?.Dispose();
    }
}
