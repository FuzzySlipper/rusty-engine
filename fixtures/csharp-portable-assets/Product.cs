using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpPortableAssets;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const ulong SpriteObject = 8647;
    private readonly IEngineContext engine;
    private readonly PortableAssetContent sprite;
    private readonly SpriteAtlas atlas;
    private readonly Appearance appearance;
    private readonly SpritePlayback playback;
    private readonly RenderResource texture;
    private readonly RenderResource model;
    private int loads;
    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        using var loose = engine.Content.OpenReference(new("loose/asset.json"));
        using var bundle = context.Content.OpenBundle("packed");
        using var packed = bundle.OpenReference("asset.json");
        sprite = new(engine.Content, loose, "sprite");
        using var equivalent = new PortableAssetContent(engine.Content, packed, "sprite");
        if (!sprite.Facts.Frames.Span.SequenceEqual(equivalent.Facts.Frames.Span) ||
            !sprite.Facts.AnimationFrames.Span.SequenceEqual(equivalent.Facts.AnimationFrames.Span))
            throw new InvalidOperationException("Loose/bundle descriptor semantics differ");
        if (sprite.FindAnimation("idle", "right") is not null) throw new InvalidOperationException("Missing direction was invented");
        using var source = sprite.OpenMember("sheet");
        texture = engine.Graphics.OpenResourceFromContent(new(source, TextureFilter.Nearest, TextureWrap.Clamp)).Handle;
        atlas = sprite.CreateAtlas(engine.Graphics, texture, "sheet");
        appearance = engine.Graphics.CreateSpriteFromAtlas(new(atlas, 0, new(.5f,.5f), new(180,180), BillboardMode.None, SpriteSizeMode.Pixel, 100, SpriteDepthPolicy.DepthTestOff, new(1,1,1,1)));
        engine.Graphics.SetSpriteViewport(new(appearance, true, new(.3f,.3f), new(.4f,.4f), new(.5f,.5f), SpriteViewportFit.Contain));
        playback = sprite.CreatePlayback(engine.Graphics, appearance, atlas, sprite.FindAnimation("idle", "front")!);
        engine.Graphics.ControlSpritePlayback(new(playback, SpritePlaybackControl.Start));
        using var body = new PortableAssetContent(engine.Content, packed, "body");
        using var modelSource = body.OpenMember("body");
        model = engine.Animation.OpenAnimatedMeshFromContent(new(modelSource));
        var namedClip = body.Facts.Relationships.Span.ToArray().Single(row => row.Kind == PortableAssetRelationshipKind.AnimationClip && row.Name == "standing").Target;
        if (!engine.Animation.ReadClips(model).Span.ToArray().Any(clip => clip.Name == namedClip || clip.Id == namedClip)) throw new InvalidOperationException("Named glTF clip did not resolve");
        for (int i = 0; i < 3; i++) { using var reloaded = new PortableAssetContent(engine.Content, packed, "body"); using var member = reloaded.OpenMember("body"); loads++; }
        Publish();
    }
    private void Publish() => engine.Graphics.PublishSnapshot(new AppearanceFact[] { new(SpriteObject, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), appearance, true, RenderLayer.Viewmodel) });
    [DebugCommand("portable.inspect")]
    public string Inspect() => $"equivalent=true; frames={sprite.Facts.Frames.Length}; timing=0.2,0.4,0.2; missingDirection=true; modelClips={engine.Animation.ReadClips(model).Length}; reloads={loads}; frame={engine.Graphics.ReadSpritePlayback(playback).FrameIndex}";
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public ProductUpdateResult Update(ProductUpdate update) { engine.Graphics.AdvanceSpritePlayback(new(playback)); Publish(); return ProductUpdateResult.None; }
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { playback.Dispose(); appearance.Dispose(); atlas.Dispose(); texture.Dispose(); model.Dispose(); sprite.Dispose(); }
}
