using System;
using System.IO;
using System.IO.Compression;
using System.Numerics;
using Rusty.Engine;

namespace ChurnExercise;

// #8771 exercise. Every Period updates it creates a texture+sprite or a
// mesh+appearance from new bytes (an identity the browser has never fetched),
// publishes an object showing it, then removes the object and releases both:
// - CHURN_MODE=same: all in the same update (the case #8771 asks about);
// - CHURN_MODE=next: removed and released one update later;
// - CHURN_MODE=hold: removed and released 30 updates later (control).
// CHURN_KIND=texture (default) or mesh selects the resource.
public sealed class Product : IEngineProduct
{
    private const ulong ObjectId = 87_710;
    private const uint Period = 60;
    private readonly IEngineContext _engine;
    private readonly string _mode;
    private readonly bool _mesh;
    private readonly Material _material;
    private readonly Camera _camera;
    private RenderResource? _texture;
    private MeshResource? _meshResource;
    private Appearance? _appearance;
    private uint _updates;
    private uint _serial;
    private uint _releaseAt;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        _mode = Environment.GetEnvironmentVariable("CHURN_MODE") ?? "same";
        _mesh = Environment.GetEnvironmentVariable("CHURN_KIND") == "mesh";
        _material = _engine.Graphics.CreateMaterial(new MaterialRequest(
            new Color(0.8f, 0.5f, 0.2f, 1), default, 1, new Color(1, 1, 1, 1), default, 0, false,
            MaterialAlphaMode.Opaque, 0.5f));
        CameraQueries.TryLookAtPose(new(0, 2, 5), new(0, 1, 0), 0, out CameraPose pose);
        _camera = _engine.CameraView.CreateCamera(new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, 60, 0, .05f, 100), CameraViewports.Full));
        _engine.CameraView.SetActiveCamera(_camera);
    }

    public void Start() { }
    public void Attach() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        uint phase = _updates++ % Period;
        if (phase == 0 && _appearance is null)
        {
            Create();
            _engine.Graphics.PublishChanges(new(new[] { Fact() }, ReadOnlyMemory<ulong>.Empty,
                ReadOnlyMemory<MeshJointAttachment>.Empty));
            _releaseAt = _mode switch { "same" => _updates, "next" => _updates + 1, _ => _updates + 30 };
            Console.WriteLine($"CHURN created {_serial}");
        }
        if (_appearance is not null && _updates >= _releaseAt)
        {
            _engine.Graphics.PublishChanges(new(ReadOnlyMemory<AppearanceFact>.Empty, new[] { ObjectId },
                ReadOnlyMemory<MeshJointAttachment>.Empty));
            _appearance.Dispose();
            _texture?.Dispose();
            _meshResource?.Dispose();
            _appearance = null;
            _texture = null;
            _meshResource = null;
            Console.WriteLine($"CHURN released {_serial}");
        }
        return ProductUpdateResult.None;
    }

    private void Create()
    {
        _serial += 1;
        if (_mesh)
        {
            float top = 1 + _serial * 0.001f; // new bytes, new identity
            _meshResource = _engine.Graphics.CreateMeshResource(new MeshResourceCreateRequest(
                new[] { new Vector3(-0.5f, 0, 0), new Vector3(0.5f, 0, 0), new Vector3(0, top, 0) },
                new[] { Vector3.UnitZ, Vector3.UnitZ, Vector3.UnitZ },
                new[] { Vector2.Zero, Vector2.UnitX, Vector2.One },
                new uint[] { 0, 1, 2 },
                new[] { new MeshGroup(0, 0, 3) },
                new[] { new MeshMaterialBinding(0, _material) }));
            _appearance = _engine.Graphics.CreateMeshAppearance(_meshResource);
            return;
        }
        using ContentReference reference = _engine.Content.AdmitReference(new ContentAdmissionRequest(
            $"churn/{_serial}.png", Png(_serial), ReadOnlyMemory<ContentSourceFile>.Empty));
        _texture = _engine.Graphics.OpenResourceFromContent(new RenderResourceContentRequest(
            reference, TextureFilter.Nearest, TextureWrap.Clamp)).Handle;
        _appearance = _engine.Graphics.CreateSprite(new SpriteAppearanceRequest(
            _texture, Vector2.Zero, Vector2.One, new Vector2(0.5f, 0.5f), new Vector2(1, 1),
            BillboardMode.Spherical, SpriteSizeMode.World, 0, SpriteDepthPolicy.Default, new Color(1, 1, 1, 1),
            new SpriteMaterialDescriptor(SpriteLightingMode.Unlit, default, default, 0, 0,
                SpriteAlphaMode.Blend, 0, SpriteShadowPolicy.None)));
    }

    private AppearanceFact Fact() => new(ObjectId, false, 0,
        new Transform(new Vector3(0, 1, 0), Quaternion.Identity, Vector3.One), _appearance!, true, RenderLayer.Scene);

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public bool CompleteTimeline(ProductTimelineCompletion completion) => false;
    public void Dispose()
    {
        _appearance?.Dispose();
        _texture?.Dispose();
        _meshResource?.Dispose();
        _material.Dispose();
        _camera.Dispose();
    }

    // One RGBA pixel whose colour follows `seed`.
    private static byte[] Png(uint seed)
    {
        byte[] row = { 0, (byte)(seed * 53), (byte)(seed * 97), (byte)(seed * 193), 255 };
        using MemoryStream compressed = new();
        using (ZLibStream z = new(compressed, CompressionLevel.Optimal, leaveOpen: true)) z.Write(row);
        using MemoryStream png = new();
        png.Write(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 });
        Chunk(png, "IHDR", new byte[] { 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0 });
        Chunk(png, "IDAT", compressed.ToArray());
        Chunk(png, "IEND", Array.Empty<byte>());
        return png.ToArray();
    }

    private static void Chunk(Stream output, string type, byte[] data)
    {
        byte[] body = new byte[4 + data.Length];
        for (int i = 0; i < 4; i++) body[i] = (byte)type[i];
        data.CopyTo(body, 4);
        WriteBigEndian(output, (uint)data.Length);
        output.Write(body);
        WriteBigEndian(output, Crc32(body));
    }

    private static void WriteBigEndian(Stream output, uint value) =>
        output.Write(new[] { (byte)(value >> 24), (byte)(value >> 16), (byte)(value >> 8), (byte)value });

    private static uint Crc32(byte[] bytes)
    {
        uint crc = 0xffffffff;
        foreach (byte b in bytes)
        {
            crc ^= b;
            for (int k = 0; k < 8; k++) crc = (crc & 1) != 0 ? (crc >> 1) ^ 0xedb88320 : crc >> 1;
        }
        return ~crc;
    }
}
