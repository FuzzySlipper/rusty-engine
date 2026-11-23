using System;
using System.Collections.Generic;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Testing;

// Runs in a product's test project: the Engine's own services, in process,
// refuse what a running product would have refused.
internal static class EngineTestHostChecks
{
    public static void Run(string persistenceRoot, ReadOnlyMemory<byte> png, ReadOnlyMemory<byte> body, string? library = null)
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
                ["meshes/triangle.json"] = System.Text.Encoding.UTF8.GetBytes(TriangleMesh),
                ["models/prop.glb"] = TwoCubeProp(),
                ["models/body.glb"] = body,
                ["spatial/site.rspatial"] = SiteArtifact(),
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
            // Only a `rusty dev` session has source content to read or edit.
            ExpectRefusal(() => engine.ContentSource.ReadDocument("textures/atlas.png"), "CONTENT_SOURCE_UNAVAILABLE");
            ExpectRefusal(() => engine.ContentSource.Edit(new ContentSourceDocument("meshes/triangle.json", default, default),
                ContentSourceEdit.Remove("/positions")), "CONTENT_SOURCE_UNAVAILABLE");
        });

        host.Call(engine =>
        {
            // A GLB model's own triangles answer nearest points and rays
            // where the caller places it, in the caller's space: the prop's
            // grip cube spans x 9..11, y 1..3, z 1..3 there, its barrel cube
            // z -2.5..-1.5.
            using RenderResource prop = engine.Animation.OpenAnimatedMesh(new("models/prop.glb"));
            Transform placement = new(new Vector3(10, 0, 0), Quaternion.Identity, new Vector3(2));
            ModelSurfacePoint[] nearest = engine.Animation.QueryModelSurface(new(prop, placement,
                new[] { new Vector3(10, 2, 3.5f), new Vector3(10, 2, 2.2f), new Vector3(10, 20, 0) }, 2f)).ToArray();
            Require(nearest.Length == 3, "one answer per point");
            Require(nearest[0].Found && !nearest[0].Inside && Near(nearest[0].Point, new(10, 2, 3))
                && Near(nearest[0].Normal, Vector3.UnitZ) && MathF.Abs(nearest[0].Distance - .5f) < 1e-4f,
                $"a point off the grip finds its face: {nearest[0]}");
            Require(nearest[1].Found && nearest[1].Inside && MathF.Abs(nearest[1].Distance - .8f) < 1e-4f,
                $"a point in the grip is inside: {nearest[1]}");
            Require(!nearest[2].Found, "a point beyond reach finds nothing");
            ModelRayHit[] hits = engine.Animation.CastModelRays(new(prop, placement,
                new ModelRay[] { new(new(10, 2, 0), Vector3.UnitZ), new(new(10, 2, 0), -Vector3.UnitZ), new(new(10, 2, 2), Vector3.UnitY) }, 10f)).ToArray();
            Require(hits[0].Hit && !hits[0].BackFace && Near(hits[0].Point, new(10, 2, 1)) && Near(hits[0].Normal, -Vector3.UnitZ),
                $"a ray meets the grip: {hits[0]}");
            Require(hits[1].Hit && Near(hits[1].Point, new(10, 2, -1.5f)), $"and the barrel: {hits[1]}");
            Require(hits[2].Hit && hits[2].BackFace && Near(hits[2].Point, new(10, 3, 2)), $"a ray from inside meets a back face: {hits[2]}");

            // Pose sampling needs no renderer: the unrigged prop has no joints
            // at rest, and names no clip.
            Require(engine.Animation.SampleModelJointPose(AnimationModelPoseSampleRequest.OfClip(prop, "", 0, placement)).Length == 0,
                "an unrigged model samples no joints");
            ExpectRefusal(() => engine.Animation.SampleModelJointPose(AnimationModelPoseSampleRequest.OfClip(prop, "swing", .5f, placement)),
                "CSHARP_ANIMATION_POSE");
        });

        host.Call(engine =>
        {
            // Disposing an appearance costs what it holds, not how many are
            // live: disposing eight times as many takes about eight times as
            // long, for atlas sprites beside content meshes (internal
            // resources) and for content meshes, which each add a catalog
            // mesh and material.
            using RenderResource texture = engine.Graphics.OpenResource(new RenderResourceRequest("textures/atlas.png")).Handle;
            using SpriteAtlas atlas = engine.Graphics.CreateSpriteAtlas(new(texture,
                new SpriteAtlasFrame[] { new(1, Vector2.Zero, Vector2.One, false, Vector2.Zero), new(2, Vector2.Zero, Vector2.One, false, Vector2.Zero) }));
            Appearance Mesh() => engine.Graphics.CreateStaticMeshFromContent(new("meshes/triangle.json", new Color(1, 1, 1, 1)));
            TimeSpan Dispose(bool meshes, int count)
            {
                var appearances = new List<Appearance>(count + count / 1000 + 1);
                for (int index = 0; index < count; index++)
                {
                    if (meshes)
                    {
                        appearances.Add(Mesh());
                        continue;
                    }
                    appearances.Add(engine.Graphics.CreateSpriteFromAtlas(new(atlas, 1, Vector2.Zero, Vector2.One,
                        BillboardMode.Cylindrical, SpriteSizeMode.World, 0, SpriteDepthPolicy.Default, new Color(1, 1, 1, 1))));
                    if (index % 1000 == 0) appearances.Add(Mesh());
                }
                var started = System.Diagnostics.Stopwatch.StartNew();
                foreach (Appearance appearance in appearances) appearance.Dispose();
                return started.Elapsed;
            }
            foreach ((bool meshes, int few) in new[] { (false, 25_000), (true, 10_000) })
            {
                Dispose(meshes, few / 5);
                TimeSpan fewer = Dispose(meshes, few);
                TimeSpan many = Dispose(meshes, few * 8);
                Require(many < fewer * 24,
                    $"Disposal grew with the live count ({(meshes ? "content meshes" : "atlas sprites")}): {few * 8} took {many}, {few} took {fewer}.");
            }

            // A batch of 10,000 sprites is one appearance and one object.
            var instances = new SpriteBatchInstance[10_000];
            for (int index = 0; index < instances.Length; index++)
                instances[index] = new(new(index % 100, 0, index / 100), 1 + index % 3 * 0.25f, (uint)(1 + index % 2));
            Appearance batch = engine.Graphics.CreateSpriteBatch(new(atlas, instances, new(0.5f, 0), Vector2.One, BillboardMode.Cylindrical));
            ExpectRefusal(() => engine.Graphics.CreateSpriteBatch(new(atlas, new SpriteBatchInstance[] { new(Vector3.Zero, 1, 3) },
                new(0.5f, 0), Vector2.One, BillboardMode.Cylindrical)), "CSHARP_SPRITE_ATLAS_FRAME");
            engine.Graphics.PublishSnapshot(new AppearanceFact[]
            {
                new(9, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), batch, true, RenderLayer.Scene),
            });
            Require(engine.Graphics.ReadPresentation().RetainedObjectCount == 1, "A sprite batch was not one object.");
            ExpectRefusal(atlas.Dispose, "CSHARP_SPRITE_ATLAS_IN_USE");
            engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
            batch.Dispose();
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
            // A product shader is read when it is opened, not compiled: one that does not compose
            // opens, and the renderer reports it where it compiles it. Its materials hold it.
            RenderResourceInfo broken = engine.Graphics.OpenResource(new RenderResourceRequest("shaders/broken.wgsl"));
            Require(broken.Kind == RenderResourceKind.Shader, "a shader that does not compose still opens");
            broken.Handle.Dispose();
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

            // Contacts: every actor a volume overlaps with the floor, and a
            // blade turning through a held shield before its holder.
            SpatialEntityCollider Body(ulong entity, float x) =>
                new(entity, new(x - .25f, 0, -.25f), new(x + .25f, 2, .25f), 0, 0, true, false, false);
            var bodies = new[] { Body(7, .5f), Body(3, -.5f), Body(9, 0), Body(5, 4) };
            SpatialContactResult overlap = engine.Spatial.OverlapContacts(SpatialContactQueryRequest.Overlap(session,
                SpatialShape.Sphere(1), SpatialPose.At(new(0.5f, 0, 0.5f)), bodies, ignoredEntities: new ulong[] { 9 }));
            SpatialContact[] overlapped = overlap.Contacts.ToArray();
            Require(overlap.WorldContact && overlapped.Length == 3 && overlapped[0].Kind == SpatialHitKind.StaticMesh
                && overlapped[1].Entity == 3 && overlapped[2].Entity == 7,
                $"An overlap did not list the floor then every actor in it: {string.Join(", ", overlapped)}");
            var shield = new SpatialEntityPart(21, 0, SpatialShape.Box(new(.05f, .4f, .4f)), SpatialPose.At(new(.9f, 1, -.9f)), holder: 20);
            var holder = new SpatialEntityPart(20, 2, SpatialShape.Capsule(new(0, .5f, 0), new(0, 1.5f, 0), .3f), SpatialPose.At(new(.9f, 0, -1.6f)));
            SpatialContactResult swept = engine.Spatial.SweepContacts(SpatialContactQueryRequest.Sweep(session,
                SpatialShape.Capsule(Vector3.Zero, new(2, 0, 0), .05f),
                new(new(0, 1, 0), Quaternion.Identity), new(new(0, 1, 0), Quaternion.CreateFromAxisAngle(Vector3.UnitY, 1.5f)),
                parts: new[] { holder, shield }, excludeWorld: true));
            SpatialContact[] met = swept.Contacts.ToArray();
            Require(met.Length == 2 && met[0].Entity == 21 && met[0].Holder == 20 && met[1].Entity == 20 && met[1].Part == 2
                && met[0].Fraction < met[1].Fraction,
                $"A turning blade did not meet the shield before its holder: {string.Join(", ", met)}");
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
            engine.Ui.PublishProjection(new(stream, value));
            // The Engine numbers each published change, so an unchanged value
            // may be published again.
            engine.Ui.PublishProjection(new(stream, value));
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
            // Auto exposure left without a target aims at middle grey, as it
            // did before products could set one.
            AutoExposureRequest grey = new(Enabled: true, Speed: 1.5f, MinExposure: 0.25f, MaxExposure: 4);
            Require(grey.TargetLuminance == 0.18f, $"An omitted target was {grey.TargetLuminance}.");
            engine.CameraView.SetAutoExposure(grey);
            AutoExposureRequest set = new(true, 1.5f, 0.25f, 4, 0.03f);
            Require(set.TargetLuminance == 0.03f, $"A set target was {set.TargetLuminance}.");
            engine.CameraView.SetAutoExposure(set);
            ExpectRefusal(() => engine.CameraView.SetAutoExposure(grey with { TargetLuminance = 0 }), "CSHARP_AUTO_EXPOSURE");

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

        QueuedPersistence(host);
        RagdollFall(host);
        DeferredResidency(host);
        DeferredCollisionResidency(host);
        DeferredCollisionNavigation(host);
        SurfaceCrawler(host);
        SoundArrival(host);
        SurfaceNavigation(host);
        DeferredContentResidency(host);
        Bundles(library);
    }

    // Deferred saves return queued with their revision and ticket; Load and
    // the guards see the newest at once, a save that is not deferred lands
    // after them, and a cache store reads back what it wrote (#10911).
    private static void QueuedPersistence(EngineTestHost host)
    {
        host.Call(engine =>
        {
            using PersistenceStore store = engine.Persistence.OpenStore(new("queued"));
            for (ulong revision = 1; revision <= 3; revision++)
            {
                PersistenceSaveReceipt queued = engine.Persistence.Save(new(
                    store, "slot", PersistenceRevisionGuard.Any, 0, new byte[] { (byte)revision }, Deferred: true));
                Require(queued == new PersistenceSaveReceipt(PersistenceSaveOutcome.Queued, revision, revision), $"A deferred save answered {queued}.");
            }
            using (PersistenceBlob newest = engine.Persistence.Load(new(store, "slot")))
            {
                Require(engine.Persistence.DescribeBlob(newest).Revision == 3 && engine.Persistence.ReadBlobBytes(newest).Span[0] == 3,
                    "Load did not see the newest queued save.");
            }
            PersistenceSaveReceipt stale = engine.Persistence.Save(new(
                store, "slot", PersistenceRevisionGuard.Exact, 2, new byte[] { 9 }, Deferred: true));
            Require(stale == new PersistenceSaveReceipt(PersistenceSaveOutcome.RevisionConflict, 3), $"A stale guard answered {stale}.");
            PersistenceSaveReceipt saved = engine.Persistence.Save(new(store, "slot", PersistenceRevisionGuard.Exact, 3, new byte[] { 4 }));
            Require(saved == new PersistenceSaveReceipt(4), $"A save after queued ones answered {saved}.");
            PersistenceSaveReadout readout = engine.Persistence.ReadSaves(store);
            Require(readout == new PersistenceSaveReadout(3, 3, 0, 0), $"The queue read {readout}.");
            Require(engine.Persistence.ReadSaveFailureText(store) == "", "A store with no failure reported one.");

            using PersistenceStore cache = engine.Persistence.OpenStore(new("cache", Cache: true));
            engine.Persistence.Save(new(cache, "chunk", PersistenceRevisionGuard.Any, 0, new byte[8192], Deferred: true));
            using PersistenceBlob chunk = engine.Persistence.Load(new(cache, "chunk"));
            Require(engine.Persistence.DescribeBlob(chunk) == new PersistenceBlobInfo(true, 1, 8192), "A cache store did not read back its blob.");
        });
    }

    // A deferred static collision batch, a terrain window of about 1.6 M
    // triangles, returns from its call without baking it, reports how far
    // it is while it bakes, and is live once the readout says so; a batch
    // removing tiles while it bakes follows it (#10328).
    private static void DeferredCollisionResidency(EngineTestHost host)
    {
        const int Tiles = 49, Quads = 128;
        int tileVertices = (Quads + 1) * (Quads + 1), tileTriangles = 2 * Quads * Quads;
        var vertices = new Vector3[Tiles * tileVertices];
        var triangles = new Triangle[Tiles * tileTriangles];
        var assets = new StaticMeshAsset[Tiles];
        var instances = new StaticMeshInstance[Tiles];
        for (int tile = 0; tile < Tiles; tile++)
        {
            int firstVertex = tile * tileVertices, firstTriangle = tile * tileTriangles;
            for (int z = 0; z <= Quads; z++)
                for (int x = 0; x <= Quads; x++)
                    vertices[firstVertex + z * (Quads + 1) + x] = new(tile % 7 * Quads + x, 0, tile / 7 * Quads + z);
            for (int z = 0, next = firstTriangle; z < Quads; z++)
                for (int x = 0; x < Quads; x++)
                {
                    uint corner = (uint)(z * (Quads + 1) + x), row = Quads + 1;
                    triangles[next++] = new(corner, corner + row, corner + 1);
                    triangles[next++] = new(corner + 1, corner + row, corner + row + 1);
                }
            assets[tile] = new((ulong)tile + 1, default, (uint)firstVertex, (uint)tileVertices, (uint)firstTriangle, (uint)tileTriangles);
            instances[tile] = new((ulong)tile + 1, (ulong)tile + 1, new(Vector3.Zero, Quaternion.Identity, Vector3.One));
        }
        SpatialSession? sync = null, deferred = null;
        bool Hit(IEngineContext engine, SpatialSession session, int tile) => engine.Spatial.CastRay(new(session,
            new(tile % 7 * Quads + Quads / 2, 5, tile / 7 * Quads + Quads / 2), -Vector3.UnitY, 10, new(uint.MaxValue, uint.MaxValue),
            ReadOnlyMemory<SpatialEntityCollider>.Empty, ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<SpatialEntityCollider>.Empty)).Present;
        TimeSpan synchronous = default, deferredCall = default;
        ulong batch = 0, removal = 0;
        host.Call(engine =>
        {
            sync = engine.Spatial.CreateSession(new(1, 16, VoxelSurfaceMode.GreedyCubes));
            deferred = engine.Spatial.CreateSession(new(1, 16, VoxelSurfaceMode.GreedyCubes));
            var started = System.Diagnostics.Stopwatch.StartNew();
            engine.Spatial.ApplyCollisionResidency(new(sync, assets, vertices, triangles, instances,
                ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<ulong>.Empty));
            synchronous = started.Elapsed;
            started.Restart();
            CollisionReplaceReceipt receipt = engine.Spatial.ApplyCollisionResidency(new(deferred, assets, vertices, triangles,
                instances, ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<ulong>.Empty, true));
            deferredCall = started.Elapsed;
            batch = receipt.DeferredBatch;
            Require(batch == 1 && receipt.InstanceCount == 0 && !Hit(engine, deferred, 0),
                $"A deferred collision batch was live in its own call: {receipt}.");
            // Tiles 0 and 48 leave while the window bakes.
            removal = engine.Spatial.ApplyCollisionResidency(new(deferred, ReadOnlyMemory<StaticMeshAsset>.Empty,
                ReadOnlyMemory<Vector3>.Empty, ReadOnlyMemory<Triangle>.Empty, ReadOnlyMemory<StaticMeshInstance>.Empty,
                new ulong[] { 1, 49 }, new ulong[] { 1, 49 }, true)).DeferredBatch;
            CollisionResidencyReadout reading = engine.Spatial.ReadCollisionResidency(new(deferred));
            Require(reading.PendingBatches == 2 && reading.CompletedBatch == 0 && reading.BakedInstances <= reading.PlannedInstances,
                $"The batches in flight read {reading}.");
        });
        Require(deferredCall < synchronous / 2,
            $"A deferred batch's call took {deferredCall.TotalMilliseconds:0} ms, a synchronous one {synchronous.TotalMilliseconds:0} ms.");
        for (int call = 0; ; call++)
        {
            CollisionResidencyReadout reading = default;
            host.Call(engine => reading = engine.Spatial.ReadCollisionResidency(new(deferred!)));
            if (reading.CompletedBatch >= removal)
            {
                Require(reading.RefusedBatches == 0 && reading.PendingBatches == 0, $"The batches ended as {reading}.");
                break;
            }
            Require(reading.BakedInstances <= reading.PlannedInstances && call < 20_000, $"The batches never finished: {reading}.");
            System.Threading.Thread.Sleep(1);
        }
        host.Call(engine =>
        {
            for (int tile = 0; tile < Tiles; tile++)
                Require(Hit(engine, deferred!, tile) == (tile != 0 && tile != 48) && Hit(engine, sync!, tile),
                    $"Tile {tile}'s collision is not as its batches left it.");
            sync!.Dispose();
            deferred!.Dispose();
        });
    }

    // A collision-derived grid derived off the update returns from its call
    // pending, leaves the grid as it was until a later call installs it, and
    // then holds what a synchronous derivation of the same collision does
    // (#10808).
    // A crawler steered by script round a closed 1 m voxel room (inside x and
    // z -3..3, y 1..5): floor, up the +X wall, across the ceiling, down the -X
    // wall; it lets go of the ceiling on request and lands, and is lost when
    // the wall it holds is cut away.
    /// <summary>
    /// A noise in a closed room, heard through the room's wall by the session's acoustics: muffled
    /// beyond it, carried once a doorway is cut and the air graph shows it, and silent past its
    /// range; the acoustic space names only a live session.
    /// </summary>
    private static void SoundArrival(EngineTestHost host) => host.Call(engine =>
    {
        SpatialSession session = engine.Spatial.CreateSession(new(1, 8, VoxelSurfaceMode.GreedyCubes));
        var room = new List<VoxelEdit>();
        for (int x = -1; x <= 16; x++)
            for (int y = -1; y <= 4; y++)
                for (int z = -1; z <= 8; z++)
                    if (y is -1 or 4 || x is -1 or 16 || z is -1 or 8 || x == 8)
                        room.Add(new(VoxelEditKind.Set, new(x, y, z), 1));
        engine.Voxel.ApplyEdits(new(session, room.ToArray()));
        Vector3 source = new(4.5f, 1.5f, 3.5f);
        Vector3[] points = [new(4.5f, 1.5f, 6.5f), new(12.5f, 1.5f, 3.5f), new(60f, 1.5f, 3.5f)];
        AudioArrival[] walled = engine.Audio.QueryArrival(session, source, 30f, points).Arrivals.ToArray();
        Require(walled.Length == 3 && walled[0].Path == AudioArrivalPath.Open && walled[0].LowPassHz == 20000f,
            $"same side of the wall: {walled[0]}");
        Require(walled[1].Path == AudioArrivalPath.Occluded && walled[1].LowPassHz < 2000f && walled[1].Audible
            && walled[1].Decibels < walled[0].Decibels - 15f, $"through the wall: {walled[1]}");
        Require(walled[2].Path == AudioArrivalPath.Silent && !walled[2].Audible, $"out of range: {walled[2]}");
        var door = new List<VoxelEdit>();
        for (int y = 0; y <= 2; y++)
            for (int z = 6; z <= 7; z++)
                door.Add(new(VoxelEditKind.Clear, new(8, y, z), 0));
        engine.Voxel.ApplyEdits(new(session, door.ToArray()));
        // The session's air graph rebuilds where the doorway was cut off the
        // update; a query reads the latest one built.
        AudioArrivalResult opened = engine.Audio.QueryArrival(session, source, 30f, points);
        for (int attempt = 0; attempt < 500 && opened.Arrivals.Span[1].Path != AudioArrivalPath.Around; attempt++)
        {
            System.Threading.Thread.Sleep(4);
            opened = engine.Audio.QueryArrival(session, source, 30f, points);
        }
        AudioArrival carried = opened.Arrivals.Span[1];
        Require(carried.Path == AudioArrivalPath.Around && carried.Gain > walled[1].Gain && carried.PathMetres > 8f
            && carried.ArrivesFrom.Z > 4f && opened.CellsSearched > 0, $"round by the doorway: {carried}, {opened}");
        ExpectRefusal(() => engine.Audio.QueryArrival(session, source, 30f, new Vector3[65]), "CSHARP_AUDIO_ARRIVAL_POINTS");
        engine.Audio.SetAcousticSpace(session, 0.5f);
        engine.Audio.ClearAcousticSpace();
        session.Dispose();
        ExpectRefusal(() => engine.Audio.SetAcousticSpace(session), "CSHARP_SPATIAL_SESSION");
    });

    private static void SurfaceCrawler(EngineTestHost host) => host.Call(engine =>
    {
        using SpatialSession session = engine.Spatial.CreateSession(new(1, 8, VoxelSurfaceMode.GreedyCubes));
        var room = new List<VoxelEdit>();
        for (int x = -4; x <= 3; x++)
            for (int y = 0; y <= 5; y++)
                for (int z = -4; z <= 3; z++)
                    if (y == 0 || y == 5 || x == -4 || x == 3 || z == -4 || z == 3)
                        room.Add(new(VoxelEditKind.Set, new(x, y, z), 1));
        engine.Voxel.ApplyEdits(new(session, room.ToArray()));
        CrawlerConfig config = engine.Spatial.DefaultCrawlerConfig();
        Require(config.Radius == 0.25f && config.ContactSkin > 0, $"default crawler config: {config}");
        Vector3 position = new(0, 1.5f, 0.5f);
        CrawlerMotion motion = default;
        ulong sequence = 0;
        var transitions = new List<CrawlerTransition>();
        CrawlerStepReceipt Step(Vector3 direction, bool cling)
        {
            CrawlerStepReceipt receipt = engine.Spatial.ProposeCrawlerStep(new(session, position, motion, config,
                new(direction, cling, 1f / 60, ++sequence)));
            position = receipt.Position;
            motion = receipt.Motion;
            if (receipt.Transition != CrawlerTransition.None) transitions.Add(receipt.Transition);
            return receipt;
        }
        CrawlerStepReceipt Drive(Vector3 direction, bool cling, string what, Func<CrawlerStepReceipt, bool> done)
        {
            for (int i = 0; i < 400; i++)
            {
                CrawlerStepReceipt receipt = Step(direction, cling);
                if (done(receipt)) return receipt;
            }
            throw new InvalidOperationException($"crawler {what}: not done at {position}, {motion}; {string.Join(",", transitions)}");
        }
        bool On(CrawlerStepReceipt r, CrawlerSurface surface, Vector3 up) =>
            r.Surface == surface && Near(r.Motion.Up, up) && Near(Vector3.Transform(Vector3.UnitY, r.Rotation), up);

        Drive(Vector3.Zero, true, "lands", r => On(r, CrawlerSurface.Floor, Vector3.UnitY) && r.Motion.Velocity.Length() < 1e-3f);
        Require(MathF.Abs(position.Y - 1.27f) < 0.02f, $"rests its skin off the floor: {position}");
        CrawlerStepReceipt wall = Drive(Vector3.UnitX, true, "up the +X wall", r => On(r, CrawlerSurface.Wall, -Vector3.UnitX));
        Require(wall.Contact.Present && wall.Contact.SourceKind == CharacterCollisionSourceKind.VoxelChunk
            && Near(wall.Contact.Normal, -Vector3.UnitX), $"the wall contact: {wall.Contact}");
        Require(transitions.Contains(CrawlerTransition.Concave), "floor to wall is a concave edge");
        Drive(Vector3.UnitY, true, "onto the ceiling", r => On(r, CrawlerSurface.Ceiling, -Vector3.UnitY));
        Drive(-Vector3.UnitX, true, "across the ceiling", r => On(r, CrawlerSurface.Wall, Vector3.UnitX));
        Drive(-Vector3.UnitY, true, "down to the floor", r => On(r, CrawlerSurface.Floor, Vector3.UnitY));

        // Back up the wall to the ceiling, then let go.
        Drive(-Vector3.UnitX, true, "back up the -X wall", r => On(r, CrawlerSurface.Wall, Vector3.UnitX));
        Drive(Vector3.UnitY, true, "back onto the ceiling", r => On(r, CrawlerSurface.Ceiling, -Vector3.UnitY));
        for (int i = 0; i < 20; i++) Step(Vector3.UnitX, true);
        CrawlerStepReceipt letGo = Step(Vector3.Zero, false);
        Require(letGo.Transition == CrawlerTransition.Detached && letGo.Surface == CrawlerSurface.None && !letGo.Motion.Attached,
            $"letting go of the ceiling: {letGo}");
        CrawlerStepReceipt landed = Drive(Vector3.Zero, false, "falls and lands", r => r.Transition == CrawlerTransition.Landed);
        Require(On(landed, CrawlerSurface.Floor, Vector3.UnitY) && landed.Position.Y < 1.3f, $"landed on the floor: {landed}");

        // Up the +X wall, then cut it away under the crawler.
        Drive(new(1, 1, 0), true, "up the wall again", r => r.Surface == CrawlerSurface.Wall && r.Position.Y > 3);
        var cut = new List<VoxelEdit>();
        for (int y = 1; y <= 4; y++)
            for (int z = -2; z <= 2; z++)
                cut.Add(new(VoxelEditKind.Clear, new(3, y, z), 0));
        engine.Voxel.ApplyEdits(new(session, cut.ToArray()));
        CrawlerStepReceipt lost = Step(Vector3.Zero, true);
        Require(lost.Transition == CrawlerTransition.Lost && lost.Surface == CrawlerSurface.None, $"lost its wall: {lost}");
        ExpectRefusal(() => engine.Spatial.ProposeCrawlerStep(new(session, position, motion, config with { Radius = 0 },
            new(Vector3.Zero, true, 1f / 60, ++sequence))), "invalid-crawler-config");
    });

    /// <summary>
    /// A surface grid over a room whose floor and side walls a trench cuts in two, joined only by
    /// the ceiling: derived on a worker, it routes floor, wall, ceiling, wall, floor and a crawler
    /// follows the route; planar requests refuse it, the grid list names its kind, and the edge
    /// explanation names the corner a shelf over a lip blocks.
    /// </summary>
    private static void SurfaceNavigation(EngineTestHost host)
    {
        const ulong Grid = 21;
        SpatialSession? session = null;
        host.Call(engine =>
        {
            session = engine.Spatial.CreateSession(new(1, 8, VoxelSurfaceMode.GreedyCubes));
            var room = new List<VoxelEdit>();
            for (int x = -4; x <= 3; x++)
                for (int y = 0; y <= 5; y++)
                    for (int z = -4; z <= 3; z++)
                    {
                        bool inside = x >= -3 && x <= 2 && y >= 1 && y <= 4 && z >= -3 && z <= 2;
                        bool trench = x is -1 or 0 && y < 5;
                        if (!inside && !trench) room.Add(new(VoxelEditKind.Set, new(x, y, z), 1));
                    }
            engine.Voxel.ApplyEdits(new(session, room.ToArray()));
            SurfaceNavigationConfig config = engine.Spatial.DefaultSurfaceNavigationConfig() with { GridId = Grid };
            Require(config.CellSize == 1 && config.WallCost == 2 && config.CeilingCost == 3, $"default surface navigation config: {config}");
            SurfaceNavigationReplaceReceipt pending = engine.Spatial.ReplaceSurfaceNavigation(new(session, new(-3, 1, -3), new(3, 5, 3), config, true));
            Require(pending.Pending && pending.SurfaceCellCount == 0, $"a deferred surface grid: {pending}");
        });
        SurfaceNavigationReadout installed = default;
        for (int call = 0; ; call++)
        {
            host.Call(engine => installed = engine.Spatial.ReadSurfaceNavigation(new(session!, Grid)));
            if (!installed.Pending) break;
            Require(call < 20_000, "The deferred surface grid never installed.");
            System.Threading.Thread.Sleep(1);
        }
        host.Call(engine =>
        {
            Require(!installed.Refused && installed.Installed.SurfaceCellCount > 0 && installed.Installed.ComponentCount == 1,
                $"the installed surface grid: {installed}");
            SurfaceNavigationPathResult route = engine.Spatial.RequestSurfaceNavigationPath(new(session!, Grid,
                new(-2.5f, 1.27f, 0.5f), new(2.5f, 1.27f, 0.5f), 10_000));
            SurfaceWaypoint[] waypoints = route.Waypoints.ToArray();
            var runs = new List<CrawlerSurface>();
            foreach (SurfaceWaypoint waypoint in waypoints)
                if (runs.Count == 0 || runs[^1] != waypoint.Surface) runs.Add(waypoint.Surface);
            Require(route.Outcome == SurfaceNavigationPathOutcome.Reached
                    && runs.SequenceEqual(new[] { CrawlerSurface.Floor, CrawlerSurface.Wall, CrawlerSurface.Ceiling, CrawlerSurface.Wall, CrawlerSurface.Floor }),
                $"the route across the trench: {route.Outcome} {string.Join(",", runs)}");

            // Phase 1's movement follows it, leaning into a waypoint's surface round a concave corner.
            CrawlerConfig crawler = engine.Spatial.DefaultCrawlerConfig();
            Vector3 position = waypoints[0].Position;
            CrawlerMotion motion = default;
            var crossed = new List<CrawlerSurface>();
            int next = 1;
            for (ulong sequence = 1; sequence <= 3000 && next < waypoints.Length; sequence++)
            {
                Vector3 toward = waypoints[next].Position - position;
                if (toward.Length() < (next + 1 == waypoints.Length ? 0.1f : 0.35f)) { next++; continue; }
                Vector3 direction = Vector3.Normalize(toward), normal = waypoints[next].Normal;
                if (Vector3.Dot(normal, motion.Up) < 0.9f && Vector3.Dot(normal, position - waypoints[next].Position) >= -1e-3f)
                    direction -= normal;
                CrawlerStepReceipt step = engine.Spatial.ProposeCrawlerStep(new(session!, position, motion, crawler, new(direction, true, 1f / 60, sequence)));
                position = step.Position;
                motion = step.Motion;
                if (step.Surface != CrawlerSurface.None && (crossed.Count == 0 || crossed[^1] != step.Surface)) crossed.Add(step.Surface);
            }
            Require(next == waypoints.Length && crossed.SequenceEqual(runs), $"followed to {position}: {string.Join(",", crossed)}");

            ExpectRefusal(() => engine.Spatial.RequestNavigationPath(new(session!, new(-3, 1, 0), new(-2, 1, 0), 100, Grid)), "navigation-grid-is-surface");
            NavigationGridInfo listed = engine.Spatial.ReadNavigationGrids(new(session!)).ToArray().Single();
            Require(listed.GridId == Grid && listed.Kind == NavigationProjectionKind.CollisionSurface
                    && listed.WalkableCellCount == installed.Installed.SurfaceCellCount, $"the grid list: {listed}");
            engine.Spatial.ClearNavigation(new(session!, Grid));
            Require(engine.Spatial.RequestSurfaceNavigationPath(new(session!, Grid, waypoints[0].Position, waypoints[^1].Position, 100)).Outcome
                    == SurfaceNavigationPathOutcome.Unavailable, "a cleared surface grid is unavailable");
            session!.Dispose();

            // A step a metre high for x >= 0, and a shelf beside its lip that the ball cannot round.
            using SpatialSession ledge = engine.Spatial.CreateSession(new(1, 8, VoxelSurfaceMode.GreedyCubes));
            var voxels = new List<VoxelEdit>();
            for (int x = -6; x < 6; x++)
                for (int z = -3; z < 3; z++)
                {
                    voxels.Add(new(VoxelEditKind.Set, new(x, 0, z), 1));
                    if (x >= 0) voxels.Add(new(VoxelEditKind.Set, new(x, 1, z), 1));
                    if (x == -1) voxels.Add(new(VoxelEditKind.Set, new(x, 2, z), 1));
                }
            engine.Voxel.ApplyEdits(new(ledge, voxels.ToArray()));
            engine.Spatial.ReplaceSurfaceNavigation(new(ledge, new(-3, 0, -1), new(3, 4, 2), engine.Spatial.DefaultSurfaceNavigationConfig() with { GridId = Grid }, false));
            SurfaceCell top = new(0, 2, 0, SurfaceFace.PositiveY), side = new(-1, 1, 0, SurfaceFace.NegativeX);
            SurfaceNavigationCellReadout cell = engine.Spatial.ExplainSurfaceNavigationCell(new(ledge, Grid, top));
            Require(cell.Outcome == SurfaceCellOutcome.Surface && cell.Admitted && cell.Surface == CrawlerSurface.Floor
                    && cell.SurfaceContact.SourceKind == CharacterCollisionSourceKind.VoxelChunk, $"the lip's top: {cell}");
            SurfaceNavigationEdgeReadout corner = engine.Spatial.ExplainSurfaceNavigationEdge(new(ledge, Grid, top, side));
            Require(corner.Kind == SurfaceEdgeKind.Convex && corner.Outcome == SurfaceEdgeOutcome.Blocked && !corner.Admitted
                    && corner.CornerPresent && Near(corner.Corner, new(0, 2, 0.5f)) && corner.Blocker.Present && corner.Blocker.Point.Y >= 2 - 1e-4f,
                $"the blocked lip: {corner}");
            ExpectRefusal(() => engine.Spatial.ReplaceSurfaceNavigation(new(ledge, new(-3, 0, -1), new(3, 4, 2),
                engine.Spatial.DefaultSurfaceNavigationConfig() with { GridId = 0 }, false)), "invalid-surface-navigation-config");
        });
    }

    private static void DeferredCollisionNavigation(EngineTestHost host)
    {
        const int Quads = 96;
        var vertices = new Vector3[(Quads + 1) * (Quads + 1)];
        var triangles = new Triangle[2 * Quads * Quads];
        for (int z = 0; z <= Quads; z++)
            for (int x = 0; x <= Quads; x++)
                vertices[z * (Quads + 1) + x] = new(x, (x / 8 + z / 8) % 2 * 0.25f, z);
        for (int z = 0, next = 0; z < Quads; z++)
            for (int x = 0; x < Quads; x++)
            {
                uint corner = (uint)(z * (Quads + 1) + x), row = Quads + 1;
                triangles[next++] = new(corner, corner + row, corner + 1);
                triangles[next++] = new(corner + 1, corner + row, corner + row + 1);
            }
        SpatialSession? session = null;
        CollisionNavigationReplaceRequest Grid(IEngineContext engine, ulong grid) => new(session!, new(0, -1, 0),
            new(Quads, 3, Quads), engine.Spatial.DefaultCollisionNavigationConfig() with { GridId = grid, MaximumCells = Quads * Quads });
        host.Call(engine =>
        {
            session = engine.Spatial.CreateSession(new(1, 16, VoxelSurfaceMode.GreedyCubes));
            engine.Spatial.ApplyCollisionResidency(new(session, new StaticMeshAsset[] { new(1, default, 0, (uint)vertices.Length, 0, (uint)triangles.Length) },
                vertices, triangles, new StaticMeshInstance[] { new(1, 1, new(Vector3.Zero, Quaternion.Identity, Vector3.One)) },
                ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<ulong>.Empty));
            CollisionNavigationReplaceReceipt receipt = engine.Spatial.ReplaceCollisionNavigation(Grid(engine, 1) with { Deferred = true });
            CollisionNavigationReadout readout = engine.Spatial.ReadCollisionNavigation(new(session, 1));
            Require(receipt.Pending && receipt.WalkableCellCount == 0 && readout.Pending && readout.Installed.WalkableCellCount == 0
                    && !engine.Spatial.ReadNavigationProjection(new(session, 1)).Present,
                $"A deferred grid was live in its own call: {receipt}, {readout}.");
        });
        CollisionNavigationReadout installed = default;
        for (int call = 0; ; call++)
        {
            host.Call(engine => installed = engine.Spatial.ReadCollisionNavigation(new(session!, 1)));
            if (!installed.Pending) break;
            Require(call < 20_000, "The deferred grid never installed.");
            System.Threading.Thread.Sleep(1);
        }
        host.Call(engine =>
        {
            CollisionNavigationReplaceReceipt synchronous = engine.Spatial.ReplaceCollisionNavigation(Grid(engine, 2));
            Require(!installed.Refused && !installed.Installed.Pending && installed.Installed.NavigationRevision > 0
                    && installed.Installed.WalkableCellCount == synchronous.WalkableCellCount
                    && installed.Installed.ProjectionHash == synchronous.ProjectionHash
                    && installed.Installed.ComponentCount == synchronous.ComponentCount,
                $"The deferred grid {installed} is not the synchronous one {synchronous}.");
            Require(engine.Spatial.RequestNavigationPath(new(session!, new(1, 0, 1), new(Quads - 2, 0, Quads - 2), 100_000, 1)).Outcome
                    == NavigationPathOutcome.Reached, "The deferred grid does not route across the floor.");
            session!.Dispose();
        });
    }

    // A deferred spatial content change, two sites of about 295 k triangles
    // and 147 k navigation cells each, returns from its call before parsing,
    // baking or composing them, reports how far it is while a worker does,
    // and its collision and navigation are live once the readout says so; a
    // removal made while they are in flight follows it (#10361).
    private static void DeferredContentResidency(EngineTestHost host)
    {
        SpatialSession? sync = null, deferred = null;
        ContentReference? site = null;
        bool Hit(IEngineContext engine, SpatialSession session, float x) => engine.Spatial.CastRay(new(session,
            new(x, 5, SiteQuads / 2f), -Vector3.UnitY, 10, new(uint.MaxValue, uint.MaxValue),
            ReadOnlyMemory<SpatialEntityCollider>.Empty, ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<SpatialEntityCollider>.Empty)).Present;
        NavigationPathOutcome Path(IEngineContext engine, SpatialSession session, long goal) =>
            engine.Spatial.RequestNavigationPath(new(session, new(1, 0, SiteQuads / 2), new(goal, 0, SiteQuads / 2), 100_000)).Outcome;
        SpatialContentArtifactResidencyRequest Change(SpatialSession session, SpatialContentArtifactInstance[] placed,
            ulong[] removed, bool defer) => new(session, placed, removed, 7, 16, 1, defer);
        TimeSpan synchronous = default, deferredCall = default;
        ulong removal = 0;
        host.Call(engine =>
        {
            sync = engine.Spatial.CreateSession(new(1, 16, VoxelSurfaceMode.GreedyCubes));
            deferred = engine.Spatial.CreateSession(new(1, 16, VoxelSurfaceMode.GreedyCubes));
            site = engine.Content.OpenReference(new("spatial/site.rspatial"));
            SpatialContentArtifactInstance[] sites =
                [new(1, site, 0, 0, 0, 0, Vector3.Zero), new(2, site, SiteQuads + 8, 0, 0, 0, Vector3.Zero)];
            var started = System.Diagnostics.Stopwatch.StartNew();
            engine.Spatial.ApplyContentArtifactResidency(Change(sync, sites, [], false));
            synchronous = started.Elapsed;
            started.Restart();
            SpatialContentArtifactResidencyReceipt receipt =
                engine.Spatial.ApplyContentArtifactResidency(Change(deferred, sites, [], true));
            deferredCall = started.Elapsed;
            Require(receipt.DeferredBatch == 1 && receipt.InstanceCount == 0 && !Hit(engine, deferred, 100.5f)
                && !engine.Spatial.ReadNavigationProjection(new(deferred)).Present,
                $"A deferred placement was live in its own call: {receipt}.");
            // The second site leaves while the two are admitted.
            removal = engine.Spatial.ApplyContentArtifactResidency(Change(deferred, [], [2], true)).DeferredBatch;
            engine.Spatial.ApplyContentArtifactResidency(Change(sync, [], [2], false));
            CollisionResidencyReadout reading = engine.Spatial.ReadCollisionResidency(new(deferred));
            Require(reading.PendingBatches == 2 && reading.CompletedBatch == 0, $"The changes in flight read {reading}.");
        });
        Require(deferredCall < synchronous / 2,
            $"A deferred placement's call took {deferredCall.TotalMilliseconds:0} ms, a synchronous one {synchronous.TotalMilliseconds:0} ms.");
        for (int call = 0; ; call++)
        {
            CollisionResidencyReadout reading = default;
            host.Call(engine => reading = engine.Spatial.ReadCollisionResidency(new(deferred!)));
            if (reading.CompletedBatch >= removal)
            {
                Require(reading.RefusedBatches == 0 && reading.PendingBatches == 0, $"The changes ended as {reading}.");
                break;
            }
            Require(reading.BakedInstances <= reading.PlannedInstances && reading.ComposedCells <= reading.PlannedCells
                && call < 20_000, $"The changes never finished: {reading}.");
            System.Threading.Thread.Sleep(1);
        }
        host.Call(engine =>
        {
            NavigationProjectionReadout left = engine.Spatial.ReadNavigationProjection(new(deferred!));
            NavigationProjectionReadout expected = engine.Spatial.ReadNavigationProjection(new(sync!));
            Require(left.WalkableCellCount == SiteQuads * SiteQuads && left.ProjectionHash == expected.ProjectionHash,
                $"The navigation is not as the changes left it: {left}, synchronous {expected}.");
            Require(Hit(engine, deferred!, 100.5f) && !Hit(engine, deferred!, SiteQuads + 100.5f),
                "The collision is not as the changes left it.");
            Require(Path(engine, deferred!, SiteQuads - 2) == NavigationPathOutcome.Reached
                && Path(engine, deferred!, SiteQuads + 100) != NavigationPathOutcome.Reached,
                "A path does not cross the site that stayed, or crosses the one that left.");
            sync!.Dispose();
            deferred!.Dispose();
            site!.Dispose();
        });
    }

    private const int SiteQuads = 384;

    // A spatial content artifact in its binary form: a flat floor of
    // SiteQuads x SiteQuads one-metre cells, every cell walkable.
    private static byte[] SiteArtifact()
    {
        var bytes = new System.IO.MemoryStream();
        using var writer = new System.IO.BinaryWriter(bytes);
        writer.Write("RSPATIAL"u8);
        writer.Write(1u);
        writer.Write(1u);
        foreach (double bound in new double[] { 0, -1, 0, SiteQuads, 1, SiteQuads })
            writer.Write(bound);
        foreach (double setting in new[] { 1.0, 0.25, 45.0, 1.8, 0.05 })
            writer.Write(setting);
        foreach (string id in new[] { "mesh/site", "navigation/site" })
        {
            writer.Write((uint)id.Length);
            writer.Write(System.Text.Encoding.UTF8.GetBytes(id));
        }
        const int Row = SiteQuads + 1, Cells = SiteQuads * SiteQuads;
        writer.Write((ulong)(Row * Row));
        writer.Write((ulong)(2 * Cells));
        writer.Write((ulong)Cells);
        for (int z = 0; z < Row; z++)
            for (int x = 0; x < Row; x++)
            {
                writer.Write((double)x);
                writer.Write(0.0);
                writer.Write((double)z);
            }
        for (int z = 0; z < SiteQuads; z++)
            for (int x = 0; x < SiteQuads; x++)
            {
                uint corner = (uint)(z * Row + x);
                foreach (uint index in new[] { corner, corner + Row, corner + 1, corner + 1, corner + Row, corner + Row + 1 })
                    writer.Write(index);
            }
        // Cells by column, then row: their columns, rows, supports and the
        // walkable bits.
        for (int column = 0; column < SiteQuads; column++)
            for (int row = 0; row < SiteQuads; row++)
                writer.Write(column);
        for (int column = 0; column < SiteQuads; column++)
            for (int row = 0; row < SiteQuads; row++)
                writer.Write(row);
        for (int cell = 0; cell < Cells; cell++)
            writer.Write(0.0);
        for (int cell = 0; cell < Cells / 8; cell++)
            writer.Write((byte)0xFF);
        writer.Flush();
        return bytes.ToArray();
    }

    // A deferred residency change writes its voxels at once and installs its
    // meshes and colliders when a call begins after they are built: a
    // stream's chunks, one evicted and one admitted, evicted and admitted
    // again within a call while its rebuild is in flight, end as a
    // synchronous session's (#10327).
    private static void DeferredResidency(EngineTestHost host)
    {
        const uint Edge = 8;
        SpatialSession? sync = null, deferred = null;
        VoxelResidencyReceipt Apply(IEngineContext engine, SpatialSession session, bool defer,
            (long X, long Z, uint Seed)[] admitted, (long X, long Z)[] evicted)
        {
            int volume = (int)(Edge * Edge * Edge);
            var slots = new uint[admitted.Length * volume];
            var operations = new List<VoxelResidencyOperation>();
            for (int index = 0; index < admitted.Length; index++)
            {
                for (int cell = 0; cell < volume; cell++)
                    slots[index * volume + cell] = (cell * 7 + admitted[index].Seed * 13) % 5 == 0 ? 0u : 1u;
                operations.Add(new(VoxelResidencyOperationKind.Admit, new(admitted[index].X, 0, admitted[index].Z),
                    (uint)(index * volume), (uint)volume));
            }
            foreach ((long x, long z) in evicted)
                operations.Add(new(VoxelResidencyOperationKind.Evict, new(x, 0, z), 0, 0));
            return engine.Voxel.ApplyResidency(new(default, session, operations.ToArray(), slots, default, defer));
        }
        string Shown(IEngineContext engine, SpatialSession session)
        {
            var hits = new System.Text.StringBuilder();
            for (int x = 0; x < 3 * (int)Edge; x++)
                for (int z = 0; z < 2 * (int)Edge; z++)
                {
                    SpatialHit hit = engine.Spatial.CastRay(new(session, new(x + .5f, 20, z + .5f), -Vector3.UnitY, 30,
                        new(uint.MaxValue, uint.MaxValue), ReadOnlyMemory<SpatialEntityCollider>.Empty,
                        ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<SpatialEntityCollider>.Empty));
                    hits.Append(hit.Present ? hit.VoxelY : -1).Append(',');
                }
            VoxelSceneReadout scene = engine.Voxel.ReadScene(new(session));
            return $"{scene.AuthorityHash}:{scene.ColliderChunkCount}:{hits}";
        }
        // Calls begin until the deferred session's collision shows its voxels.
        void Built()
        {
            for (int call = 0; ; call++)
            {
                bool built = false;
                host.Call(engine =>
                {
                    VoxelSceneReadout scene = engine.Voxel.ReadScene(new(deferred!));
                    built = scene.CollisionRevision == scene.SourceRevision && scene.MeshRevision == scene.SourceRevision;
                });
                if (built) return;
                Require(call < 2000, "Deferred residency never installed.");
                System.Threading.Thread.Sleep(1);
            }
        }
        (long, long, uint)[] block = [(0, 0, 1), (0, 1, 2), (1, 0, 3), (1, 1, 4)];
        host.Call(engine =>
        {
            sync = engine.Spatial.CreateSession(new(1, Edge, VoxelSurfaceMode.GreedyCubes));
            deferred = engine.Spatial.CreateSession(new(1, Edge, VoxelSurfaceMode.GreedyCubes));
            Apply(engine, sync, false, block, []);
            VoxelResidencyReceipt receipt = Apply(engine, deferred, true, block, []);
            VoxelSceneReadout scene = engine.Voxel.ReadScene(new(deferred));
            Require(receipt.CollisionRevision == receipt.RevisionBefore && scene.SourceRevision == receipt.AcceptedRevision
                && scene.CollisionRevision == receipt.RevisionBefore && scene.ColliderChunkCount == 0,
                $"A deferred admission collided in its own call: {receipt}, {scene}.");
            Require(!engine.Spatial.CastRay(new(deferred, new(.5f, 20, .5f), -Vector3.UnitY, 30, new(uint.MaxValue, uint.MaxValue),
                    ReadOnlyMemory<SpatialEntityCollider>.Empty, ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<SpatialEntityCollider>.Empty)).Present,
                "A ray met rock whose collider was still building.");
        });
        Built();
        host.Call(engine =>
        {
            Require(Shown(engine, deferred!) == Shown(engine, sync!), "Deferred admission differs from a synchronous one.");
            foreach (SpatialSession session in new[] { sync!, deferred! })
            {
                bool defer = session == deferred;
                Apply(engine, session, defer, [(2, 0, 10)], [(0, 0)]);
                Apply(engine, session, defer, [], [(2, 0)]);
                Apply(engine, session, defer, [(2, 0, 11), (2, 1, 12)], []);
            }
        });
        Built();
        host.Call(engine =>
        {
            Require(Shown(engine, deferred!) == Shown(engine, sync!), "A chunk admitted again while it rebuilt differs.");
            sync!.Dispose();
            deferred!.Dispose();
        });
    }

    // Nothing is drawn, yet a character published in one call has a pose in
    // the next: the csharp-ragdoll fixture's fall (its bones, links and hit)
    // runs headless, falls, rests on the floor and reads back (#10223).
    private static void RagdollFall(EngineTestHost host)
    {
        const float FloorHalfSize = 10, TickSeconds = 1f / 60;
        const int Ticks = 480, HitBone = 1;
        RenderResource? resource = null;
        AnimationInstance? instance = null;
        host.Call(engine =>
        {
            resource = engine.Animation.OpenAnimatedMesh(new("models/body.glb"));
            Appearance body = engine.Animation.CreateAnimatedMeshAppearance(new(resource));
            engine.Graphics.PublishSnapshot(new AppearanceFact[]
            {
                new(9785, false, 0, new(Vector3.Zero, Quaternion.Identity, Vector3.One), body, true, RenderLayer.Scene),
            });
            instance = engine.Animation.CreateInstance(new(body, 9785));
            engine.Animation.SetPlayback(new(instance, AnimationPlaybackKind.Sample, "idle", AnimationLoopMode.Repeat, 1, 1, true, 0, false, 0.3f));
        });
        host.Call(engine =>
        {
            AnimationJointPoseResult drawn = engine.Animation.ReadJointPose(instance!);
            Require(drawn.Reported, "A published character had no pose without a renderer.");
            ReadOnlySpan<AnimationJointInfo> rig = engine.Animation.ReadJoints(resource!).Span;
            uint Joint(ReadOnlySpan<AnimationJointInfo> joints, string? id) =>
                id is null ? DynamicsRagdollBone.NoEndJoint : AnimationJoints.IndexOf(joints, id);
            (string Joint, string? End, float Length, DynamicsRagdollShape Shape, float Radius, float HalfDepth, float Mass)[] described =
            [
                ("Hips", "Spine", 0, DynamicsRagdollShape.Box, 0.26f, 0.16f, 12),
                ("Spine", "Neck", 0, DynamicsRagdollShape.Box, 0.3f, 0.18f, 24),
                ("Head", null, 0.5f, DynamicsRagdollShape.Capsule, 0.24f, 0, 6),
                ("LeftArm", "LeftForeArm", 0, DynamicsRagdollShape.Capsule, 0.09f, 0, 3),
                ("LeftForeArm", "LeftHand", 0, DynamicsRagdollShape.Capsule, 0.08f, 0, 2),
                ("RightArm", "RightForeArm", 0, DynamicsRagdollShape.Capsule, 0.09f, 0, 3),
                ("RightForeArm", "RightHand", 0, DynamicsRagdollShape.Capsule, 0.08f, 0, 2),
                ("LeftUpLeg", "LeftLeg", 0, DynamicsRagdollShape.Capsule, 0.11f, 0, 8),
                ("LeftLeg", "LeftFoot", 0, DynamicsRagdollShape.Capsule, 0.09f, 0, 4),
                ("RightUpLeg", "RightLeg", 0, DynamicsRagdollShape.Capsule, 0.11f, 0, 8),
                ("RightLeg", "RightFoot", 0, DynamicsRagdollShape.Capsule, 0.09f, 0, 4),
            ];
            var bones = new DynamicsRagdollBone[described.Length];
            for (int index = 0; index < bones.Length; index++)
            {
                var bone = described[index];
                bones[index] = new(Joint(rig, bone.Joint), Joint(rig, bone.End), bone.Length, bone.Shape, bone.Radius, bone.HalfDepth, bone.Mass, false);
            }
            DynamicsJointLimits elbow = DynamicsJointLimits.Hinge(0, 2.5f, 2), knee = DynamicsJointLimits.Hinge(0, 2.4f, 2);
            DynamicsRagdollLink[] links =
            [
                new(0, 1, Vector3.UnitY, DynamicsJointLimits.Cone(0.5f, -0.3f, 0.3f, 2)),
                new(1, 2, Vector3.UnitY, DynamicsJointLimits.Cone(0.6f, -0.6f, 0.6f, 2)),
                new(1, 3, Vector3.UnitY, DynamicsJointLimits.Cone(1.4f, -0.8f, 0.8f, 2)),
                new(3, 4, Vector3.UnitX, elbow),
                new(1, 5, Vector3.UnitY, DynamicsJointLimits.Cone(1.4f, -0.8f, 0.8f, 2)),
                new(5, 6, Vector3.UnitZ, elbow),
                new(0, 7, Vector3.UnitY, DynamicsJointLimits.Cone(1.2f, -0.4f, 0.4f, 2)),
                new(7, 8, Vector3.UnitX, knee),
                new(0, 9, Vector3.UnitY, DynamicsJointLimits.Cone(1.2f, -0.4f, 0.4f, 2)),
                new(9, 10, Vector3.UnitX, knee),
            ];
            SpatialSession spatial = engine.Spatial.CreateSession(new SpatialSessionConfig(1.0, 16, VoxelSurfaceMode.GreedyCubes));
            engine.Spatial.ReplaceCollision(new CollisionReplaceRequest(
                spatial,
                new[] { new StaticMeshAsset(1, 0, 4, 0, 2) },
                new[]
                {
                    new Vector3(-FloorHalfSize, 0, -FloorHalfSize),
                    new Vector3(FloorHalfSize, 0, -FloorHalfSize),
                    new Vector3(FloorHalfSize, 0, FloorHalfSize),
                    new Vector3(-FloorHalfSize, 0, FloorHalfSize),
                },
                new[] { new Triangle(0, 2, 1), new Triangle(0, 3, 2) },
                new[] { new StaticMeshInstance(1, 1, new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One)) }));
            DynamicsWorld world = engine.Dynamics.CreateWorld(new DynamicsWorldConfig(new Vector3(0, -9.81f, 0)));
            engine.Dynamics.BindWorldCollision(new DynamicsWorldCollisionBindingRequest(world, spatial));
            DynamicsRagdoll ragdoll = engine.Dynamics.CreateRagdoll(new(world, instance!, bones, links, 1, uint.MaxValue, 0.8f, 0, 0.05f, 0.3f, 1));
            Transform[] spawned = engine.Dynamics.ReadRagdoll(ragdoll).Bones.ToArray();
            Vector3 hips = drawn.Joints.Span[(int)Joint(rig, "Hips")].World.Translation;
            Require(Vector3.Distance(spawned[0].Translation, hips) < 1e-3f,
                $"The hips spawned at {spawned[0].Translation}, not where they were drawn ({hips}).");
            Transform chest = spawned[HitBone];
            engine.Dynamics.ApplyRagdollImpulse(new(ragdoll, HitBone,
                chest.Translation + Vector3.Transform(new Vector3(0.05f, 0.4f, 0.1f), chest.Rotation), new Vector3(0, 0, -110)));
            int? restedAt = null;
            float lowest = float.MaxValue;
            for (int tick = 1; tick <= Ticks; tick++)
            {
                engine.Dynamics.Step(new(world, TickSeconds, 1, ReadOnlyMemory<DynamicsAction>.Empty));
                DynamicsRagdollResult read = engine.Dynamics.ReadRagdoll(ragdoll);
                if (read.Resting) restedAt ??= tick;
                foreach (Transform bone in read.Bones.Span) lowest = MathF.Min(lowest, bone.Translation.Y);
            }
            Transform[] fallen = engine.Dynamics.ReadRagdoll(ragdoll).Bones.ToArray();
            Require(restedAt is not null, $"The ragdoll did not rest within {Ticks} steps.");
            Require(fallen[0].Translation.Y < spawned[0].Translation.Y - 0.4f,
                $"The hips did not fall: {spawned[0].Translation} to {fallen[0].Translation}.");
            Require(lowest > -0.05f, $"A bone's joint went through the floor: {lowest}.");
            ragdoll.Dispose();
            world.Dispose();
            spatial.Dispose();
        });
    }

    // One triangle: a content static mesh the Engine stages as an internal
    // resource of each appearance made from it.
    private const string TriangleMesh = """
        {"asset":"mesh/test","payload":{"layout":{"vertexCount":3,"indexCount":3,"indexWidth":"u32","attributes":[{"name":"position","components":3,"kind":"f32"},{"name":"normal","components":3,"kind":"f32"}]},"groups":[{"materialSlot":0,"start":0,"count":3}],"bounds":{"min":[0.0,0.0,0.0],"max":[1.0,1.0,0.0]},"source":{"kind":"inline","positions":[0.0,0.0,0.0,1.0,0.0,0.0,0.0,1.0,0.0],"normals":[0.0,0.0,1.0,0.0,0.0,1.0,0.0,0.0,1.0],"indices":[0,1,2]},"provenance":"staticAsset"},"materialSlots":[{"slot":0,"material":"material/test"}],"collision":{"kind":"visualOnly"}}
        """;

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
        // opens, and only reading such a file refuses.
        using (EngineTestHost host = Host(Inventory(("first.txt", 5), ("gone.txt", 3)), new()
        {
            ["rooms/first.txt"] = "hello"u8.ToArray(),
        }))
        {
            host.Call(engine =>
            {
                ProductContent content = new(ReadOnlyMemory<ProductContentFile>.Empty, engine.Content);
                using ProductContentBundle rooms = content.OpenBundle("rooms");
                Require(rooms.Entries.Length == 2, "an open lists the inventory without reading bodies");
                Require(rooms.ReadText("first.txt") == "hello", "a readable file reads beside an unreadable one");
                ExpectRefusal(() => rooms.ReadFile("gone.txt"), "PRODUCT_SOURCE_MISSING");
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

    private static bool Near(Vector3 a, Vector3 b) => Vector3.Distance(a, b) < 1e-4f;

    /// <summary>
    /// A GLB prop of two closed unit cubes on mesh nodes under one parent:
    /// the parent turned 90° about y and raised 1, the children 1 to either
    /// side of it, the second at half size. In the model the first cube's
    /// centre is (0, 1, 1) and the second's (0, 1, -1).
    /// </summary>
    private static byte[] TwoCubeProp()
    {
        var positions = new List<float>();
        var normals = new List<float>();
        var indices = new List<ushort>();
        (Vector3 Normal, Vector3 U, Vector3 V)[] faces =
        {
            (Vector3.UnitX, Vector3.UnitY, Vector3.UnitZ), (-Vector3.UnitX, Vector3.UnitZ, Vector3.UnitY),
            (Vector3.UnitY, Vector3.UnitZ, Vector3.UnitX), (-Vector3.UnitY, Vector3.UnitX, Vector3.UnitZ),
            (Vector3.UnitZ, Vector3.UnitX, Vector3.UnitY), (-Vector3.UnitZ, Vector3.UnitY, Vector3.UnitX),
        };
        foreach ((Vector3 normal, Vector3 u, Vector3 v) in faces)
        {
            ushort first = (ushort)(positions.Count / 3);
            foreach ((float a, float b) in new[] { (-1f, -1f), (1f, -1f), (1f, 1f), (-1f, 1f) })
            {
                Vector3 corner = .5f * (normal + u * a + v * b);
                positions.AddRange(new[] { corner.X, corner.Y, corner.Z });
                normals.AddRange(new[] { normal.X, normal.Y, normal.Z });
            }
            indices.AddRange(new[] { first, (ushort)(first + 1), (ushort)(first + 2), first, (ushort)(first + 2), (ushort)(first + 3) });
        }
        var bin = new List<byte>();
        foreach (float value in positions) bin.AddRange(BitConverter.GetBytes(value));
        foreach (float value in normals) bin.AddRange(BitConverter.GetBytes(value));
        foreach (ushort value in indices) bin.AddRange(BitConverter.GetBytes(value));
        while (bin.Count % 4 != 0) bin.Add(0);
        int vertexBytes = positions.Count * 4;
        string half = (MathF.Sqrt(2) / 2).ToString(System.Globalization.CultureInfo.InvariantCulture);
        string json = "{\"asset\":{\"version\":\"2.0\"},\"scene\":0,\"scenes\":[{\"nodes\":[0]}],"
            + $"\"nodes\":[{{\"children\":[1,2],\"rotation\":[0,{half},0,{half}],\"translation\":[0,1,0]}},"
            + "{\"mesh\":0,\"translation\":[-1,0,0]},{\"mesh\":0,\"translation\":[1,0,0],\"scale\":[0.5,0.5,0.5]}],"
            + "\"meshes\":[{\"primitives\":[{\"attributes\":{\"POSITION\":0,\"NORMAL\":1},\"indices\":2}]}],"
            + "\"accessors\":[{\"bufferView\":0,\"componentType\":5126,\"count\":24,\"type\":\"VEC3\",\"min\":[-0.5,-0.5,-0.5],\"max\":[0.5,0.5,0.5]},"
            + "{\"bufferView\":1,\"componentType\":5126,\"count\":24,\"type\":\"VEC3\"},"
            + "{\"bufferView\":2,\"componentType\":5123,\"count\":36,\"type\":\"SCALAR\"}],"
            + $"\"bufferViews\":[{{\"buffer\":0,\"byteOffset\":0,\"byteLength\":{vertexBytes}}},"
            + $"{{\"buffer\":0,\"byteOffset\":{vertexBytes},\"byteLength\":{vertexBytes}}},"
            + $"{{\"buffer\":0,\"byteOffset\":{2 * vertexBytes},\"byteLength\":{indices.Count * 2}}}],"
            + $"\"buffers\":[{{\"byteLength\":{bin.Count}}}]}}";
        var jsonBytes = new List<byte>(System.Text.Encoding.UTF8.GetBytes(json));
        while (jsonBytes.Count % 4 != 0) jsonBytes.Add((byte)' ');
        var glb = new List<byte>();
        glb.AddRange("glTF"u8.ToArray());
        glb.AddRange(BitConverter.GetBytes(2u));
        glb.AddRange(BitConverter.GetBytes((uint)(12 + 8 + jsonBytes.Count + 8 + bin.Count)));
        glb.AddRange(BitConverter.GetBytes((uint)jsonBytes.Count));
        glb.AddRange("JSON"u8.ToArray());
        glb.AddRange(jsonBytes);
        glb.AddRange(BitConverter.GetBytes((uint)bin.Count));
        glb.AddRange("BIN\0"u8.ToArray());
        glb.AddRange(bin);
        return glb.ToArray();
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
