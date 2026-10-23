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

        RagdollFall(host);
        DeferredResidency(host);
        Bundles(library);
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
                bones[index] = new(Joint(rig, bone.Joint), Joint(rig, bone.End), bone.Length, bone.Shape, bone.Radius, bone.HalfDepth, bone.Mass);
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
