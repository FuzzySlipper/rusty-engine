#nullable enable
using System;
using System.IO;
using System.IO.Compression;
using System.Numerics;
using Rusty.Engine;

// #8767 review fixture: every Period updates, create a texture from new bytes
// (a new, never-fetched identity) and a sprite using it, publish it, then
// release both HoldUpdates later. A browser whose resource fetch arrives after
// the release gets 404 from the host.
public sealed class ResourceChurn : IDisposable
{
    private const ulong ObjectId = 82_700;
    private const uint Period = 60;
    private readonly IEngineContext engine;
    private readonly uint holdUpdates;
    private RenderResource? texture;
    private Appearance? sprite;
    private uint updates;
    private uint serial;

    public ResourceChurn(IEngineContext engine)
    {
        this.engine = engine;
        holdUpdates = uint.TryParse(Environment.GetEnvironmentVariable("CHURN_HOLD_UPDATES"), out uint hold) ? hold : 1;
    }

    /// Returns the churn fact to include in this update's snapshot, if any.
    public AppearanceFact? Update()
    {
        uint phase = updates++ % Period;
        if (phase == 0 && sprite is null)
        {
            serial += 1;
            using ContentReference reference = engine.Content.AdmitReference(new ContentAdmissionRequest(
                $"churn/{serial}.png", Png(serial), ReadOnlyMemory<ContentSourceFile>.Empty));
            texture = engine.Graphics.OpenResourceFromContent(new RenderResourceContentRequest(
                reference, TextureFilter.Nearest, TextureWrap.Clamp)).Handle;
            sprite = engine.Graphics.CreateSprite(new SpriteAppearanceRequest(
                texture, Vector2.Zero, Vector2.One, new Vector2(0.5f, 0.5f), new Vector2(1, 1),
                BillboardMode.Spherical, SpriteSizeMode.World, 0, SpriteDepthPolicy.Default, new Color(1, 1, 1, 1),
                new SpriteMaterialDescriptor(SpriteLightingMode.Unlit, default, default, 0, 0,
                    SpriteAlphaMode.Blend, 0, SpriteShadowPolicy.None)));
            Console.WriteLine($"CHURN created {serial}");
        }
        else if (phase == holdUpdates && sprite is not null)
        {
            return null; // Released after this update's snapshot omits the object.
        }
        return sprite is null
            ? null
            : new AppearanceFact(ObjectId, false, 0,
                new Transform(new Vector3(0, 1, 0), Quaternion.Identity, Vector3.One), sprite, true, RenderLayer.Scene);
    }

    /// Called after the snapshot without the object was published.
    public void ReleaseIfDue()
    {
        if ((updates - 1) % Period != holdUpdates || sprite is null) return;
        sprite.Dispose();
        texture!.Dispose();
        sprite = null;
        texture = null;
        Console.WriteLine($"CHURN released {serial}");
    }

    public void Dispose()
    {
        sprite?.Dispose();
        texture?.Dispose();
    }

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
