using System;
using System.Collections.Generic;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Testing;

// Runs in a product's test project: the Engine's own services, in process,
// refuse what a running product would have refused.
internal static class EngineTestHostChecks
{
    public static void Run(string persistenceRoot, ReadOnlyMemory<byte> png, string? library = null)
    {
        using var host = EngineTestHost.Create(new EngineTestHostOptions
        {
            PersistenceRoot = persistenceRoot,
            Content = new Dictionary<string, ReadOnlyMemory<byte>>
            {
                ["textures/atlas.png"] = png,
                ["fonts/skin.woff2"] = "wOF2body"u8.ToArray(),
                ["shaders/tint.wgsl"] = System.Text.Encoding.UTF8.GetBytes(TintShader),
                ["shaders/broken.wgsl"] = System.Text.Encoding.UTF8.GetBytes(TintShader.Replace("surface.base", "missing")),
            },
            LibraryPath = library,
        });

        host.Call(engine =>
        {
            RenderResource texture = engine.Graphics.OpenResource(new RenderResourceRequest("textures/atlas.png")).Handle;
            SpriteAtlas atlas = engine.Graphics.CreateSpriteAtlas(new(texture,
                new SpriteAtlasFrame[] { new(1, Vector2.Zero, Vector2.One, false, Vector2.Zero) }));
            ExpectRefusal(texture.Dispose, "CSHARP_RENDER_RESOURCE_IN_USE");
            atlas.Dispose();
            texture.Dispose();
            ExpectRefusal(() => engine.Graphics.ReadTextureInfo(texture), "CSHARP_GRAPHICS_OPERATION");
        });

        host.Call(engine =>
        {
            // A normal map is linear data: the same PNG opened as colour is refused.
            using RenderResource colour = engine.Graphics.OpenResource(new RenderResourceRequest("textures/atlas.png")).Handle;
            using RenderResource data = engine.Graphics.OpenResource(new RenderResourceRequest(
                "textures/atlas.png", TextureFilter.Linear, TextureWrap.Repeat, TextureColorSpace.Linear)).Handle;
            MaterialRequest Mapped(RenderResource normalMap) => new(new(1, 1, 1, 1), colour, .8f, new(1, 1, 1, 1),
                Vector3.Zero, 0, false, MaterialAlphaMode.Opaque, .5f, 0, normalMap, 1);
            using Material material = engine.Graphics.CreateMaterial(Mapped(data));
            ExpectRefusal(() => engine.Graphics.CreateMaterial(Mapped(colour)), "CSHARP_MATERIAL_NORMAL_MAP");
            // Triplanar planes blend at a sharpness of 1 or more.
            using Material triplanar = engine.Graphics.CreateMaterial(Mapped(data) with { TriplanarSharpness = 4 });
            ExpectRefusal(() => engine.Graphics.CreateMaterial(Mapped(data) with { TriplanarSharpness = .5f }), "CSHARP_MATERIAL");
        });

        host.Call(engine =>
        {
            // A product shader is checked when it is opened, and held by its materials.
            ExpectRefusal(() => engine.Graphics.OpenResource(new RenderResourceRequest("shaders/broken.wgsl")), "CSHARP_SHADER");
            RenderResourceInfo tint = engine.Graphics.OpenResource(new RenderResourceRequest("shaders/tint.wgsl"));
            Require(tint.Kind == RenderResourceKind.Shader, "a .wgsl resource is a shader");
            RenderResource shader = tint.Handle;
            Material material = engine.Graphics.CreateMaterial(new MaterialRequest(new(1, 1, 1, 1), default, .8f,
                new(1, 1, 1, 1), Vector3.Zero, 0, false, MaterialAlphaMode.Opaque, .5f, 0, default, 1, 0,
                new MaterialShader(shader, new Vector4(1, 0, 0, 1))));
            ExpectRefusal(shader.Dispose, "CSHARP_RENDER_RESOURCE_IN_USE");
            material.Dispose();
            shader.Dispose();

            // Keywords open their own variant; a standard feature name is refused.
            RenderResourceRequest tintRequest = new("shaders/tint.wgsl");
            using RenderResource loud = engine.Graphics.OpenResource(tintRequest with { ShaderKeywords = "LOUD" }).Handle;
            ExpectRefusal(() => engine.Graphics.OpenResource(tintRequest with { ShaderKeywords = "NORMAL_MAP" }), "CSHARP_SHADER");
            // A shader's own texture is held by its material.
            RenderResource ramp = engine.Graphics.OpenResource(new RenderResourceRequest("textures/atlas.png")).Handle;
            Material ramped = engine.Graphics.CreateMaterial(new MaterialRequest(new(1, 1, 1, 1), default, .8f,
                new(1, 1, 1, 1), Vector3.Zero, 0, false, MaterialAlphaMode.Opaque, .5f, 0, default, 1, 0,
                new MaterialShader(loud, new Vector4(1, 0, 0, 1), default, default, default, ramp, default)));
            ExpectRefusal(ramp.Dispose, "CSHARP_RENDER_RESOURCE_IN_USE");
            ramped.Dispose();
            ramp.Dispose();
        });

        host.Call(engine =>
        {
            using SpatialSession session = engine.Spatial.CreateSession(new(1, 16, VoxelSurfaceMode.GreedyCubes));
            Vector3[] vertices = [new(0, 0, 0), new(1, 0, 0), new(1, 0, 1), new(0, 0, 1)];
            Triangle[] triangles = [new(0, 2, 1), new(0, 3, 2)];
            var placed = new StaticMeshInstance[] { new(1, 1, new(Vector3.Zero, Quaternion.Identity, Vector3.One)) };
            ExpectRefusal(() => engine.Spatial.ApplyCollisionResidency(new(session,
                ReadOnlyMemory<StaticMeshAsset>.Empty, ReadOnlyMemory<Vector3>.Empty, ReadOnlyMemory<Triangle>.Empty,
                placed, ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<ulong>.Empty)), "CSHARP_COLLISION_REPLACE");
            engine.Spatial.ApplyCollisionResidency(new(session,
                new StaticMeshAsset[] { new(1, default, 0, 4, 0, 2) }, vertices, triangles,
                placed, ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<ulong>.Empty));
            SpatialHit hit = engine.Spatial.CastRay(new(session, new(0.5f, 2, 0.5f), -Vector3.UnitY, 4, default,
                ReadOnlyMemory<SpatialEntityCollider>.Empty, ReadOnlyMemory<ulong>.Empty,
                ReadOnlyMemory<SpatialEntityCollider>.Empty));
            Require(hit.Present && hit.Instance == 1, "Admitted collision was not resident.");
        });

        host.Call(engine =>
        {
            using PersistenceStore first = engine.Persistence.OpenStore(new("scope-a"));
            using PersistenceStore second = engine.Persistence.OpenStore(new("scope-b"));
            Require(first.Handle != second.Handle, "Two scopes shared a store handle.");
            engine.Persistence.Save(new(first, "slot", PersistenceRevisionGuard.Any, 0, "a"u8.ToArray()));
            engine.Persistence.Save(new(second, "slot", PersistenceRevisionGuard.Any, 0, "b"u8.ToArray()));
            using PersistenceBlob blob = engine.Persistence.Load(new(first, "slot"));
            Require(engine.Persistence.ReadBlobBytes(blob).Span.SequenceEqual("a"u8), "A scope read another scope's value.");
            ExpectRefusal(() => engine.Persistence.OpenStore(new("../escape")), "CSHARP_PERSISTENCE_PATH");
        });

        host.Call(engine =>
        {
            using UiStream stream = engine.Ui.OpenStream(new("fixture.ui", "fixture.ui.v1"));
            var value = new UiValue(new StructuredValueNode[]
            {
                new(StructuredValueKind.Object, 0, 0, 0, 0, 0, 0, 0, 1),
                new(StructuredValueKind.String, 0, 0, 0, 5, 5, 0, 0, 0),
            }, new uint[] { 1 }, 0, "state"u8.ToArray());
            engine.Ui.PublishProjection(new(stream, 1, value));
            ExpectRefusal(() => engine.Ui.PublishProjection(new(stream, 1, value)), "CSHARP_UI_SEQUENCE");
        });

        host.Call(engine =>
        {
            // The UI gets a font file for @font-face and a picture for <img>; neither
            // stands in for the other.
            using ContentReference skin = engine.Content.OpenReference(new("fonts/skin.woff2"));
            using UiFont font = engine.Ui.OpenFont(new(skin));
            Require(font.Url().StartsWith("/__rusty/product/runtime/ui-fonts/", StringComparison.Ordinal), "A UI font had no font URL.");
            using ContentReference atlas = engine.Content.OpenReference(new("textures/atlas.png"));
            ExpectRefusal(() => engine.Ui.OpenFont(new(atlas)), "CSHARP_UI_FONT_FORMAT");
            using UiImage picture = engine.Ui.OpenImage(new(atlas));
            Require(picture.Url().StartsWith("/__rusty/product/runtime/ui-images/", StringComparison.Ordinal), "A UI image had no image URL.");
            ExpectRefusal(() => engine.Ui.OpenImage(new(skin)), "CSHARP_UI_IMAGE_FORMAT");
        });

        host.Call(engine =>
        {
            // Gameplay time answers as the standard 60 Hz realtime runtime: a
            // request reads back within its call, and a refusal keeps the last
            // valid one.
            Require(!engine.GameplayTime.Read().Selected, "Gameplay time was selected before any request.");
            GameplayTimeReadout held = engine.GameplayTime.Hold();
            Require(held.Selected && held.Held && held.FixedStepHz == 60, "A hold did not read back held.");
            Require(engine.GameplayTime.SetRate(0.1).Rate == 0.1, "A tenth of realtime did not read back.");
            ExpectRefusal(() => engine.GameplayTime.SetRate(1.5), "CSHARP_GAMEPLAY_TIME_RATE");
            ExpectRefusal(() => engine.GameplayTime.SetRate(double.NaN), "CSHARP_GAMEPLAY_TIME_RATE");
            Require(engine.GameplayTime.Read().Rate == 0.1, "A refused rate replaced the staged one.");
            GameplayTimeReadout advance = engine.GameplayTime.Advance(0.1);
            Require(advance.AdvanceRemainingSteps == 6 && advance.Rate == 1.0, "A 0.1 s advance was not six 60 Hz steps.");
            ExpectRefusal(() => engine.GameplayTime.Advance(0), "CSHARP_GAMEPLAY_TIME_ADVANCE");
            ExpectRefusal(() => engine.GameplayTime.Advance(1, 0), "CSHARP_GAMEPLAY_TIME_RATE");
            // No window to close: a Quit stays hidden, and asking refuses.
            Require(!engine.Host.Read().ExitAvailable, "Exit was available without a window.");
            ExpectRefusal(() => engine.Host.RequestExit(), "ENGINE_HOST_EXIT_UNAVAILABLE");
        });

        Bundles(library);
    }

    private const string TintShader = """
        #import rusty::types::Surface
        #import rusty::material::material
        #import rusty::shade::standard_shade

        fn shade(surface: Surface) -> vec4<f32> {
            let shaded = standard_shade(surface);
            return vec4<f32>(shaded.rgb * material.parameters[0].rgb, surface.base.a);
        }
        """;

    // Supplied content carrying the SDK's bundle inventory opens its bundles
    // as a staged Product does, by bundle-relative path; a short or missing
    // bundle file refuses its read.
    private static void Bundles(string? library)
    {
        static string Inventory(params (string Path, int Length)[] files) =>
            "{\"bundles\":[{\"id\":\"rooms\",\"root\":\"rooms\",\"files\":["
            + string.Join(",", Array.ConvertAll(files, file =>
                $"{{\"path\":\"{file.Path}\",\"byteLength\":{file.Length},\"sha256\":\"{new string('0', 64)}\"}}"))
            + "]}]}";
        EngineTestHost Host(string inventory, Dictionary<string, ReadOnlyMemory<byte>> files)
        {
            files[".rusty-bundles.json"] = System.Text.Encoding.UTF8.GetBytes(inventory);
            return EngineTestHost.Create(new EngineTestHostOptions { Content = files, LibraryPath = library });
        }

        using (EngineTestHost host = Host(Inventory(("first.txt", 5), ("nested/second.txt", 6)), new()
        {
            ["rooms/first.txt"] = "hello"u8.ToArray(),
            ["rooms/nested/second.txt"] = "world!"u8.ToArray(),
        }))
        {
            host.Call(engine =>
            {
                ProductContent content = new(ReadOnlyMemory<ProductContentFile>.Empty, engine.Content);
                ContentBundleInfo[] bundles = content.ListBundles().ToArray();
                Require(bundles.Length == 1 && bundles[0] == new ContentBundleInfo("rooms", 2, 11), "the inventory's bundle is listed");
                using ProductContentBundle rooms = content.OpenBundle("rooms");
                Require(rooms.ReadText("first.txt") == "hello" && rooms.ReadText("nested/second.txt") == "world!",
                    "bundle files read by bundle-relative path");
            });
        }
        // Opening costs the inventory alone: a bundle whose files are missing
        // or of the wrong length opens, and only reading such a file refuses.
        using (EngineTestHost host = Host(Inventory(("first.txt", 5), ("gone.txt", 3), ("short.txt", 9)), new()
        {
            ["rooms/first.txt"] = "hello"u8.ToArray(),
            ["rooms/short.txt"] = "cut"u8.ToArray(),
        }))
        {
            host.Call(engine =>
            {
                ProductContent content = new(ReadOnlyMemory<ProductContentFile>.Empty, engine.Content);
                using ProductContentBundle rooms = content.OpenBundle("rooms");
                Require(rooms.Entries.Length == 3, "an open lists the inventory without reading bodies");
                Require(rooms.ReadText("first.txt") == "hello", "a readable file reads beside unreadable ones");
                ExpectRefusal(() => rooms.ReadFile("gone.txt"), "PRODUCT_SOURCE_MISSING");
                ExpectRefusal(() => rooms.ReadFile("short.txt"), "PRODUCT_BUNDLE_FILE_CHANGED");
            });
        }
    }

    private static void ExpectRefusal(Action operation, string code)
    {
        try { operation(); }
        catch (EngineCallException refusal) when (refusal.Diagnostics.Length > 0 && refusal.Diagnostics.Span[0].Code == code) { return; }
        catch (EngineCallException refusal)
        {
            throw new InvalidOperationException($"Expected {code}, got: {refusal.Message}", refusal);
        }
        throw new InvalidOperationException($"Expected {code}, but the Engine accepted the call.");
    }

    private static void ExpectRefusal<T>(Func<T> operation, string code) => ExpectRefusal(() => { operation(); }, code);

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
