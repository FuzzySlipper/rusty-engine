using System.Numerics;
using Rusty.Engine;

namespace CsharpViewportSprite;

/// <summary>Small package-consumer product for the retained viewport sprite path.</summary>
public sealed class Product : IEngineProduct
{
    private const ulong WeaponObjectId = 7_895;
    private readonly IEngineContext _engine;
    private readonly SpriteAtlas _atlas;
    private readonly Appearance _appearance;
    private readonly SpritePlayback _playback;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        RenderResourceInfo texture = _engine.Graphics.OpenResource(new RenderResourceRequest("trial.png"));
        _atlas = _engine.Graphics.CreateSpriteAtlas(new SpriteAtlasCreateRequest(
            texture.Handle,
            new SpriteAtlasFrame[]
            {
                new SpriteAtlasFrame(0, Vector2.Zero, Vector2.One, true, new Vector2(320, 200)),
                new SpriteAtlasFrame(1, Vector2.Zero, Vector2.One, true, new Vector2(200, 320)),
            }));
        _appearance = _engine.Graphics.CreateSpriteFromAtlas(new SpriteFromAtlasRequest(
            _atlas,
            0,
            new Vector2(0.5f, 0.0f),
            new Vector2(320, 200),
            BillboardMode.None,
            SpriteSizeMode.Pixel,
            100,
            SpriteDepthPolicy.DepthTestOff,
            new Color(1, 1, 1, 1)));
        _engine.Graphics.SetSpriteViewport(new SpriteViewportUpdateRequest(
            _appearance,
            true,
            Vector2.Zero,
            new Vector2(0.42f, 0.32f),
            new Vector2(0.5f, 0.0f),
            SpriteViewportFit.Contain));
        _playback = _engine.Graphics.CreateSpritePlayback(new SpritePlaybackCreateRequest(
            _appearance,
            _atlas,
            new SpritePlaybackFrame[] { new(0, 0.12), new(1, 0.12) },
            ReadOnlyMemory<SpritePlaybackMarker>.Empty,
            SpritePlaybackLoopMode.Loop,
            1.0));
        Publish();
    }

    public void Start() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        _engine.Graphics.AdvanceSpritePlayback(new SpritePlaybackAdvanceRequest(_playback));
        return ProductUpdateResult.None;
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        _playback.Dispose();
        _appearance.Dispose();
        _atlas.Dispose();
    }

    private void Publish()
    {
        _engine.Graphics.PublishSnapshot(
        [
            new AppearanceFact(
                WeaponObjectId,
                false,
                0,
                new Transform(new Vector3(9, -4, 7), Quaternion.Identity, new Vector3(7, 3, 2)),
                _appearance,
                true,
                RenderLayer.Viewmodel),
        ]);
    }
}
