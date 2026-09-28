using System;
using System.Numerics;
using Rusty.Engine;

namespace MovingWorkload;

// #8736 evidence product. Admits one generated mesh of about 8 MB, then every
// update republishes a snapshot of MovingObjects objects that all move. With
// BENCH_THROW_AT=N it throws on update N after moving everything and creating
// and disposing a texture, so the fault exercise can check what was kept.
public sealed class Product : IEngineProduct
{
    private const int MovingObjects = 1000;
    private const int GridSide = 400; // 160,000 vertices, about 8 MB with indices.
    private readonly IEngineContext _engine;
    private readonly Material _material;
    private readonly MeshResource _mesh;
    private readonly Appearance _appearance;
    private readonly AppearanceFact[] _facts = new AppearanceFact[MovingObjects];
    private readonly long _throwAt;
    private long _updates;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        _throwAt = long.TryParse(Environment.GetEnvironmentVariable("BENCH_THROW_AT"), out long at) ? at : -1;
        _material = _engine.Graphics.CreateMaterial(new MaterialRequest(
            new Color(0.4f, 0.7f, 0.9f, 1), default, 1, new Color(1, 1, 1, 1), default, 0, false,
            MaterialAlphaMode.Opaque, 0.5f));
        _mesh = _engine.Graphics.CreateMeshResource(Grid());
        _appearance = _engine.Graphics.CreateMeshAppearance(_mesh);
        Publish();
    }

    public void Start() { }
    public void Attach() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        _updates++;
        Publish();
        if (_updates == _throwAt)
        {
            RenderResource texture = _engine.Graphics.OpenResourceFromContent(new RenderResourceContentRequest(
                _engine.Content.AdmitReference(new ContentAdmissionRequest("bench/pixel.png", Pixel,
                    ReadOnlyMemory<ContentSourceFile>.Empty)), TextureFilter.Nearest, TextureWrap.Clamp)).Handle;
            texture.Dispose();
            Console.WriteLine($"BENCH throwing at update {_updates}");
            throw new InvalidOperationException($"bench exception at update {_updates}");
        }
        return ProductUpdateResult.None;
    }

    private void Publish()
    {
        float phase = _updates * 0.02f;
        for (int index = 0; index < MovingObjects; index++)
        {
            float x = (index % 40) * 3 + MathF.Sin(phase + index) * 0.5f;
            float z = (index / 40) * 3 + MathF.Cos(phase + index) * 0.5f;
            _facts[index] = new AppearanceFact((ulong)index + 1, false, 0,
                new Transform(new Vector3(x, 0, z), Quaternion.Identity, new Vector3(0.01f)),
                _appearance, true, RenderLayer.Scene);
        }
        _engine.Graphics.PublishSnapshot(_facts);
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public bool CompleteTimeline(ProductTimelineCompletion completion) => false;
    public void Dispose()
    {
        _appearance.Dispose();
        _mesh.Dispose();
        _material.Dispose();
    }

    private MeshResourceCreateRequest Grid()
    {
        Vector3[] positions = new Vector3[GridSide * GridSide];
        Vector3[] normals = new Vector3[positions.Length];
        Vector2[] uvs = new Vector2[positions.Length];
        uint[] indices = new uint[(GridSide - 1) * (GridSide - 1) * 6];
        for (int z = 0; z < GridSide; z++)
        for (int x = 0; x < GridSide; x++)
        {
            int i = z * GridSide + x;
            positions[i] = new Vector3(x, MathF.Sin(x * 0.1f) * MathF.Cos(z * 0.1f), z);
            normals[i] = Vector3.UnitY;
            uvs[i] = new Vector2(x / (float)GridSide, z / (float)GridSide);
        }
        int k = 0;
        for (int z = 0; z < GridSide - 1; z++)
        for (int x = 0; x < GridSide - 1; x++)
        {
            uint a = (uint)(z * GridSide + x), b = a + 1, c = a + GridSide, d = c + 1;
            indices[k++] = a; indices[k++] = c; indices[k++] = b;
            indices[k++] = b; indices[k++] = c; indices[k++] = d;
        }
        return new MeshResourceCreateRequest(positions, normals, uvs, indices,
            new[] { new MeshGroup(0, 0, (uint)indices.Length) },
            new[] { new MeshMaterialBinding(0, _material) });
    }

    private static readonly byte[] Pixel = Png();

    // One RGBA pixel; the Engine admits only 8-bit RGBA PNGs.
    private static byte[] Png()
    {
        byte[] row = { 0, 200, 80, 40, 255 };
        using System.IO.MemoryStream compressed = new();
        using (System.IO.Compression.ZLibStream z = new(compressed, System.IO.Compression.CompressionLevel.Optimal, true)) z.Write(row);
        using System.IO.MemoryStream png = new();
        png.Write(new byte[] { 137, 80, 78, 71, 13, 10, 26, 10 });
        Chunk(png, "IHDR", new byte[] { 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0 });
        Chunk(png, "IDAT", compressed.ToArray());
        Chunk(png, "IEND", Array.Empty<byte>());
        return png.ToArray();
    }

    private static void Chunk(System.IO.Stream output, string type, byte[] data)
    {
        byte[] body = new byte[4 + data.Length];
        for (int i = 0; i < 4; i++) body[i] = (byte)type[i];
        data.CopyTo(body, 4);
        Write(output, (uint)data.Length);
        output.Write(body);
        uint crc = 0xffffffff;
        foreach (byte value in body)
        {
            crc ^= value;
            for (int bit = 0; bit < 8; bit++) crc = (crc & 1) != 0 ? (crc >> 1) ^ 0xedb88320 : crc >> 1;
        }
        Write(output, ~crc);
    }

    private static void Write(System.IO.Stream output, uint value) =>
        output.Write(new[] { (byte)(value >> 24), (byte)(value >> 16), (byte)(value >> 8), (byte)value });
}
