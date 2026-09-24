#nullable enable
using System;
using System.Numerics;
using Rusty.Engine;

// This fixture deliberately owns its admitted resources across updates. The
// package smoke advances Open -> Release -> Open, then leaves the reopened
// presentation available for a later browser attachment or visual exercise.
public sealed class ProductContentMixedBundleChecks : IDisposable
{
    private const ulong SpriteObjectId = 82_601;
    private const ulong StaticMeshObjectId = 82_602;
    private const ulong AnimatedMeshObjectId = 82_603;
    private const ulong BillboardId = 82_604;
    private readonly ProductContent content;
    private readonly IEngineContext engine;
    private MixedBundleResources? resources;
    private uint updateCount;

    public ProductContentMixedBundleChecks(ProductCreateContext context)
    {
        content = context.Content;
        engine = context.Engine;
    }

    public void Update()
    {
        switch (updateCount++)
        {
            case 0:
                Open();
                break;
            case 1:
                Release();
                break;
            case 2:
                Open();
                break;
            default:
                PublishLiveSnapshot();
                break;
        }
    }

    // This is intentionally callable by an ordinary C# visual exercise. It
    // runs after Create has returned, so bundle admission is not part of the
    // eager ProductContent snapshot.
    public void Open()
    {
        if (resources is not null)
        {
            PublishLiveSnapshot();
            return;
        }

        using ProductContentBundle bundle = content.OpenBundle("mixed");
        Require(bundle.Entries.Length == 10, "mixed inventory was discovered without loading its bodies at Create");
        Require(bundle.ReadText("readme.txt") == "bundle text body\n", "bundle text stays an ordinary product format");

        RenderResourceInfo textureInfo = OpenRenderResource(bundle, "texture.png", TextureFilter.Nearest);
        RenderResourceInfo fontInfo = OpenRenderResource(bundle, "NotoSans-Regular.woff2", TextureFilter.Linear);
        RenderResource texture = textureInfo.Handle;
        RenderResource font = fontInfo.Handle;
        Appearance staticMesh = OpenStaticMesh(bundle);
        RenderResource animatedMesh = OpenAnimatedMesh(bundle);
        RenderResource clipPack = OpenClipPack(bundle);
        AudioClip clip = OpenAudioClip(bundle);
        VoxelAsset voxelAsset = OpenVoxelAsset(bundle);

        using (ContentReference generalReference = bundle.OpenReference("general.bin"))
        {
            // Every reference used for Engine admission is now released. The
            // resource owners below must survive closing the bundle itself.
            bundle.Dispose();
            ReadOnlyMemory<byte> generalBytes = engine.Content.ReadBytes(
                new ContentReadBytesRequest(generalReference, 0, 1024));
            Require(generalBytes.Length == 22 && generalBytes.Span[0] == (byte)'b' && generalBytes.Span[^1] == 0xff,
                "general-byte reference survives its bundle close");
        }

        Require(textureInfo.Kind == RenderResourceKind.Texture && fontInfo.Kind == RenderResourceKind.Font,
            "typed renderer resources preserve their admitted kinds");
        engine.Animation.AssociateAnimationClipPack(new AnimationClipPackAssociationRequest(
            animatedMesh,
            clipPack,
            "Rusty Engine SDK package fixture",
            "CC0-1.0"));

        Appearance sprite = engine.Graphics.CreateSprite(new SpriteAppearanceRequest(
            texture,
            Vector2.Zero,
            Vector2.One,
            new Vector2(0.5f, 0.5f),
            new Vector2(1.5f, 1.5f),
            BillboardMode.Spherical,
            SpriteSizeMode.World,
            0,
            SpriteDepthPolicy.Default,
            new Color(1, 1, 1, 1),
            new SpriteMaterialDescriptor(
                SpriteLightingMode.Unlit,
                default,
                default,
                0,
                0,
                SpriteAlphaMode.Blend,
                0,
                SpriteShadowPolicy.None)));
        Appearance animatedAppearance = engine.Animation.CreateAnimatedMeshAppearance(
            new AnimatedMeshAppearanceRequest(animatedMesh));
        AnimationInstance animation = engine.Animation.CreateInstance(
            new AnimationInstanceRequest(animatedAppearance, AnimatedMeshObjectId));
        engine.Animation.SetPlayback(new AnimationPlaybackRequest(
            animation,
            AnimationPlaybackKind.Play,
            "bundle-idle",
            AnimationLoopMode.Repeat,
            1,
            1,
            true,
            0,
            false,
            0));
        PresentationBillboard fontBillboard = engine.Presentation.CreateBillboard(
            new PresentationBillboardDescriptor(
                BillboardId,
                new PresentationAnchor(PresentationAnchorKind.World, new Vector3(0, 2, 0), 0, Vector3.Zero),
                BillboardContentKind.Text,
                "fixture.bundle.font",
                "Bundle font",
                "",
                "",
                "",
                default,
                PresentationFontKind.Asset,
                font,
                "Noto Sans",
                18,
                new Color(1, 1, 1, 1),
                new Color(0, 0, 0, 0),
                100,
                PresentationBillboardLayer.AlwaysOnTop,
                true));
        AudioVoice voice = engine.Audio.CreateVoice(new AudioSourceDescriptor(
            clip,
            AudioBus.Sfx,
            1,
            1,
            true,
            0,
            1,
            0,
            AudioEmitterKind.Global2d,
            Vector3.Zero,
            0,
            Vector3.Zero));
        engine.Audio.ControlVoice(new AudioVoiceControlRequest(voice, AudioVoiceControl.Retrigger));

        resources = new MixedBundleResources(
            texture,
            font,
            staticMesh,
            sprite,
            animatedMesh,
            clipPack,
            animatedAppearance,
            animation,
            fontBillboard,
            clip,
            voice,
            voxelAsset);
        _ = engine.VoxelContent.ReadAsset(voxelAsset);
        Require(engine.Presentation.Read().ActiveBillboards > 0,
            "asset-font billboard remains admitted after bundle disposal");
        Require(engine.Audio.ReadVoice(new AudioVoiceReadRequest(voice)).Present,
            "voice retains its independently admitted audio clip after bundle disposal");
        PublishLiveSnapshot();
    }

    public void Release()
    {
        MixedBundleResources? current = resources;
        if (current is null)
        {
            return;
        }

        // A projected animation instance must release its playback target in
        // this transaction before the snapshot removes that target.
        current.DisposeAnimationBeforeRemovalSnapshot();
        engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        Require(engine.Graphics.ReadPresentation().RetainedObjectCount == 0,
            "mixed release must clear its appearance snapshot before disposal");
        current.DisposeRemaining();
        resources = null;
        Require(engine.Audio.Read().ActiveVoices == 0,
            "mixed release must dispose its voice before its audio clip");
    }

    public void Dispose() => Release();

    private void PublishLiveSnapshot()
    {
        MixedBundleResources? current = resources;
        if (current is null)
        {
            return;
        }

        // This fixture is the only Graphics snapshot publisher in this package
        // consumer. UI and voxel projection publication remain separate Engine
        // services and continue to run from Product.Update.
        engine.Graphics.PublishSnapshot(
        [
            new AppearanceFact(
                SpriteObjectId,
                false,
                0,
                new Transform(new Vector3(-2, 0, 0), Quaternion.Identity, Vector3.One),
                current.Sprite,
                true,
                RenderLayer.Scene),
            new AppearanceFact(
                StaticMeshObjectId,
                false,
                0,
                new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One),
                current.StaticMesh,
                true,
                RenderLayer.Scene),
            new AppearanceFact(
                AnimatedMeshObjectId,
                false,
                0,
                new Transform(new Vector3(2, 0, 0), Quaternion.Identity, Vector3.One),
                current.AnimatedAppearance,
                true,
                RenderLayer.Scene),
        ]);
    }

    private RenderResourceInfo OpenRenderResource(ProductContentBundle bundle, string path, TextureFilter filter)
    {
        using ContentReference reference = bundle.OpenReference(path);
        return engine.Graphics.OpenResourceFromContent(new RenderResourceContentRequest(
            reference, filter, TextureWrap.Clamp));
    }

    private Appearance OpenStaticMesh(ProductContentBundle bundle)
    {
        using ContentReference reference = bundle.OpenReference("triangle.static-mesh.json");
        return engine.Graphics.CreateStaticMeshFromContentReference(new StaticMeshContentReferenceRequest(
            reference, new Color(0.3f, 0.6f, 0.9f, 1)));
    }

    private RenderResource OpenAnimatedMesh(ProductContentBundle bundle)
    {
        // Exercise ordinary managed bytes, copied dependency entries, and a
        // failed replacement in a later update (not startup admission).
        byte[] bytes = bundle.ReadBytes("character.glb").ToArray();
        using ContentReference reference = engine.Content.AdmitReference(new ContentAdmissionRequest(
            "live/character.glb", bytes,
            new ContentSourceFile[] { new("live/texture.png", bundle.ReadBytes("texture.png")) }));
        bytes[0] = 0; // Engine owns a snapshot, not a borrowed managed array.
        RenderResource resource = engine.Animation.OpenAnimatedMeshFromContent(new AnimationContentRequest(reference));
        try
        {
            AnimatedMeshInfo info = engine.Animation.ReadMeshInfo(resource);
            Require(info.ClipCount > 0 && info.JointCount > 0 && info.BoundsMax.Y > info.BoundsMin.Y,
                "live mesh exposes Engine-admitted bounds, rig and clips");
            AnimationClipInfo clip = engine.Animation.ReadClips(resource).Span[0];
            Require(clip.Id.Length > 0 && clip.HasDuration && clip.DurationSeconds > 0,
                "live clip metadata is copied into safe C# values");
            using ContentReference malformed = engine.Content.AdmitReference(new ContentAdmissionRequest(
                "live/character.glb", bytes, ReadOnlyMemory<ContentSourceFile>.Empty));
            try
            {
                using RenderResource rejected = engine.Animation.OpenAnimatedMeshFromContent(new AnimationContentRequest(malformed));
                throw new InvalidOperationException("Malformed live GLB was accepted");
            }
            catch (EngineCallException error)
            {
                Require(error.Message.Contains("OpenAnimatedMeshFromContent", StringComparison.Ordinal),
                    "live import error names its operation");
            }
            return resource;
        }
        catch { resource.Dispose(); throw; }
    }

    private RenderResource OpenClipPack(ProductContentBundle bundle)
    {
        using ContentReference reference = bundle.OpenReference("clip-pack.glb");
        return engine.Animation.OpenAnimationClipPackFromContent(new AnimationContentRequest(reference));
    }

    private AudioClip OpenAudioClip(ProductContentBundle bundle)
    {
        using ContentReference reference = bundle.OpenReference("tone.wav");
        return engine.Audio.OpenClipFromContent(new AudioClipFromContentRequest(reference));
    }

    private VoxelAsset OpenVoxelAsset(ProductContentBundle bundle)
    {
        using ContentReference reference = bundle.OpenReference("wall.voxel.json");
        return engine.VoxelContent.LoadAssetFromContent(new LoadVoxelAssetFromContentRequest(reference));
    }

    private static void Require(bool condition, string description)
    {
        if (!condition) throw new InvalidOperationException("ProductContent mixed bundle: " + description);
    }

    private sealed class MixedBundleResources : IDisposable
    {
        private bool animationDisposed;
        internal MixedBundleResources(
            RenderResource texture,
            RenderResource font,
            Appearance staticMesh,
            Appearance sprite,
            RenderResource animatedMesh,
            RenderResource clipPack,
            Appearance animatedAppearance,
            AnimationInstance animation,
            PresentationBillboard fontBillboard,
            AudioClip clip,
            AudioVoice voice,
            VoxelAsset voxelAsset)
        {
            Texture = texture;
            Font = font;
            StaticMesh = staticMesh;
            Sprite = sprite;
            AnimatedMesh = animatedMesh;
            ClipPack = clipPack;
            AnimatedAppearance = animatedAppearance;
            Animation = animation;
            FontBillboard = fontBillboard;
            Clip = clip;
            Voice = voice;
            VoxelAsset = voxelAsset;
        }

        internal RenderResource Texture { get; }
        internal RenderResource Font { get; }
        internal Appearance StaticMesh { get; }
        internal Appearance Sprite { get; }
        internal RenderResource AnimatedMesh { get; }
        internal RenderResource ClipPack { get; }
        internal Appearance AnimatedAppearance { get; }
        internal AnimationInstance Animation { get; }
        internal PresentationBillboard FontBillboard { get; }
        internal AudioClip Clip { get; }
        internal AudioVoice Voice { get; }
        internal VoxelAsset VoxelAsset { get; }

        internal void DisposeAnimationBeforeRemovalSnapshot()
        {
            if (animationDisposed)
            {
                return;
            }

            Animation.Dispose();
            animationDisposed = true;
        }

        internal void DisposeRemaining()
        {
            Voice.Dispose();
            FontBillboard.Dispose();
            AnimatedAppearance.Dispose();
            Sprite.Dispose();
            StaticMesh.Dispose();
            VoxelAsset.Dispose();
            Clip.Dispose();
            AnimatedMesh.Dispose();
            ClipPack.Dispose();
            Font.Dispose();
            Texture.Dispose();
        }

        public void Dispose()
        {
            DisposeAnimationBeforeRemovalSnapshot();
            DisposeRemaining();
        }
    }
}
