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
            Content = new Dictionary<string, ReadOnlyMemory<byte>> { ["textures/atlas.png"] = png },
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
