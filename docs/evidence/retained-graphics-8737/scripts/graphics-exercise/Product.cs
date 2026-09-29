using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Numerics;
using Rusty.Engine;

namespace GraphicsExercise;

// #8737 evidence product. It keeps 4,000 unchanged mesh instances, 100 more
// on a mesh it later releases, an animated body and 50 cubes that move every
// update. Each update publishes the graphics changes, then:
// - update 100 moves cube 2 under cube 1, and update 110 moves it back;
// - update 150 refuses a missing joint, then attaches a sword to RightHand;
// - update 200 hides 10 mesh instances, update 250 removes 100;
// - update 300 removes the released mesh's objects and disposes the mesh.
// Update 400 prints the facts and GRAPHICS_EXERCISE_PASSED or the failures.
//
// EXERCISE_PUBLISH=changes publishes only what changed (PublishChanges);
// EXERCISE_PUBLISH=snapshot republishes every object (PublishSnapshot).
// Built with OLD_GRAPHICS_API it runs the snapshot path against the
// pre-#8737 SDK, which has PublishAttachedSnapshot but no PublishChanges.
public sealed class Product : IEngineProduct
{
    private const int Bulk = 4_000;
    private const int Released = 100;
    private const int Moving = 50;
    private const int Hidden = 10;
    private const int Removed = 100;
    private const ulong FirstBulk = 1_000, FirstReleased = 10_000, FirstMoving = 1;
    private const ulong BodyId = 20_000, SwordId = 20_001;
    private const long ReparentUpdate = 100, RestoreUpdate = 110, AttachUpdate = 150;
    private const long HideUpdate = 200, RemoveUpdate = 250, ReleaseUpdate = 300, EndUpdate = 400;
    private const long SteadyFrom = 20;
    private const string Joint = "RightHand";

    private readonly IEngineContext _engine;
    private readonly Material _material;
    private readonly MeshResource _mesh;
    private MeshResource? _releasedMesh;
    private readonly Appearance _meshAppearance;
    private Appearance? _releasedAppearance;
    private readonly Appearance _cube;
    private readonly RenderResource _bodyResource;
    private readonly Appearance _body;
    private readonly Camera _camera;
    private readonly bool _snapshot;
    private readonly SortedDictionary<ulong, AppearanceFact> _facts = new();
    // Only the last fact per object is published.
    private readonly Dictionary<ulong, AppearanceFact> _changed = new();
    private readonly List<ulong> _removed = new();
    private readonly List<double> _publishUs = new();
    private readonly List<double> _eventUs = new();
    private readonly List<string> _failures = new();
    private bool _attached;
    private string _missingJoint = "not attempted";
    private long _updates;
    private bool _reported;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
#if OLD_GRAPHICS_API
        _snapshot = true;
#else
        _snapshot = Environment.GetEnvironmentVariable("EXERCISE_PUBLISH") == "snapshot";
#endif
        CameraQueries.TryLookAtPose(new(40, 30, -30), new(40, 0, 20), 0, out CameraPose pose);
        _camera = _engine.CameraView.CreateCamera(new(pose, CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Perspective, 60, 0, .05f, 500), CameraViewports.Full));
        _engine.CameraView.SetActiveCamera(_camera);
        _material = _engine.Graphics.CreateMaterial(new MaterialRequest(
            new Color(0.4f, 0.7f, 0.9f, 1), default, 1, new Color(1, 1, 1, 1), default, 0, false,
            MaterialAlphaMode.Opaque, 0.5f));
        _mesh = _engine.Graphics.CreateMeshResource(Pyramid(1));
        _meshAppearance = _engine.Graphics.CreateMeshAppearance(_mesh);
        _releasedMesh = _engine.Graphics.CreateMeshResource(Pyramid(2));
        _releasedAppearance = _engine.Graphics.CreateMeshAppearance(_releasedMesh);
        _cube = _engine.Graphics.CreatePrimitive(new PrimitiveAppearanceRequest(
            PrimitiveGeometry.Cube, false, new Color(1, 0.5f, 0.2f, 1)));
        using (ContentReference source = _engine.Content.OpenReference(new ContentOpenRequest("body.glb")))
        {
            _bodyResource = _engine.Animation.OpenAnimatedMeshFromContent(new AnimationContentRequest(source));
        }
        _body = _engine.Animation.CreateAnimatedMeshAppearance(new AnimatedMeshAppearanceRequest(_bodyResource));

        for (int index = 0; index < Bulk; index++)
            Put(FirstBulk + (ulong)index, null, Grid(index, 0), _meshAppearance);
        for (int index = 0; index < Released; index++)
            Put(FirstReleased + (ulong)index, null, Grid(index, 60), _releasedAppearance);
        Put(BodyId, null, At(new Vector3(-5, 0, -5)), _body);
        Put(SwordId, BodyId, At(Vector3.Zero, 0.2f), _cube);
        for (int index = 0; index < Moving; index++)
            Put(FirstMoving + (ulong)index, null, Mover(index), _cube);
        Publish();
    }

    public void Start() { }
    public void Attach() { }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        _updates++;
        if (_updates > EndUpdate)
            return ProductUpdateResult.None;
        for (int index = 0; index < Moving; index++)
        {
            ulong id = FirstMoving + (ulong)index;
            Put(id, _facts[id].HasParentObject ? _facts[id].ParentObjectId : null, Mover(index), _cube);
        }
        bool eventUpdate = true;
        switch (_updates)
        {
            case ReparentUpdate:
                Put(FirstMoving + 1, FirstMoving, At(new Vector3(0, 2, 0)), _cube);
                break;
            case RestoreUpdate:
                Put(FirstMoving + 1, null, Mover(1), _cube);
                break;
            case AttachUpdate:
                RefuseMissingJoint();
                _attached = true;
                Put(SwordId, BodyId, At(Vector3.Zero, 0.2f), _cube);
                break;
            case HideUpdate:
                for (int index = 0; index < Hidden; index++)
                {
                    ulong id = FirstBulk + (ulong)index;
                    Put(id, null, Grid(index, 0), _meshAppearance, visible: false);
                }
                break;
            case RemoveUpdate:
                for (int index = 0; index < Removed; index++)
                    Remove(FirstBulk + (ulong)(Bulk - 1 - index));
                break;
            case ReleaseUpdate:
                for (int index = 0; index < Released; index++)
                    Remove(FirstReleased + (ulong)index);
                break;
            default:
                eventUpdate = false;
                break;
        }
        // Moving cube 2 under cube 1 keeps it there until update 110.
        if (_updates > ReparentUpdate && _updates < RestoreUpdate)
            Put(FirstMoving + 1, FirstMoving, At(new Vector3(0, 2, 0)), _cube);

        double us = Publish();
        if (eventUpdate)
            _eventUs.Add(us);
        else if (_updates >= SteadyFrom)
            _publishUs.Add(us);

        if (_updates == ReleaseUpdate)
        {
            _releasedAppearance!.Dispose();
            _releasedAppearance = null;
            _releasedMesh!.Dispose();
            _releasedMesh = null;
        }
        if (_updates == EndUpdate)
            Report();
        return ProductUpdateResult.None;
    }

    private void RefuseMissingJoint()
    {
        PresentationReadout before = _engine.Graphics.ReadPresentation();
        try
        {
#if OLD_GRAPHICS_API
            _engine.Graphics.PublishAttachedSnapshot(new(_facts.Values.ToArray(),
                new MeshJointAttachment[] { new(SwordId, "MissingHand8737") }));
#else
            _engine.Graphics.PublishChanges(new(new[] { _facts[SwordId] }, ReadOnlyMemory<ulong>.Empty,
                new MeshJointAttachment[] { new(SwordId, "MissingHand8737") }));
#endif
            _failures.Add("missing joint accepted");
        }
        catch (EngineCallException error)
        {
            _missingJoint = string.Join(";", error.Diagnostics.ToArray().Select(d => d.Message));
            Require(_missingJoint.Contains("MissingHand8737"), $"missing joint diagnostic: {_missingJoint}");
        }
        Require(_engine.Graphics.ReadPresentation() == before, "a refused joint changed the scene");
    }

    private double Publish()
    {
        AppearanceFact[] all = _snapshot ? _facts.Values.ToArray() : Array.Empty<AppearanceFact>();
        AppearanceFact[] changed = _changed.Values.ToArray();
        ulong[] removed = _removed.ToArray();
        bool attach = _attached && (_snapshot || _changed.ContainsKey(SwordId));
        MeshJointAttachment[] attachments = attach
            ? new MeshJointAttachment[] { new(SwordId, Joint) }
            : Array.Empty<MeshJointAttachment>();
        _changed.Clear();
        _removed.Clear();
        long started = Stopwatch.GetTimestamp();
#if OLD_GRAPHICS_API
        if (attach)
            _engine.Graphics.PublishAttachedSnapshot(new(all, attachments));
        else
            _engine.Graphics.PublishSnapshot(all);
#else
        if (_snapshot && !(attach && _updates == AttachUpdate))
            _engine.Graphics.PublishSnapshot(all);
        else if (_snapshot)
            _engine.Graphics.PublishChanges(new(all, ReadOnlyMemory<ulong>.Empty, attachments));
        else
            _engine.Graphics.PublishChanges(new(changed, removed, attachments));
#endif
        return Stopwatch.GetElapsedTime(started).TotalMicroseconds;
    }

    private void Report()
    {
        PresentationReadout presentation = _engine.Graphics.ReadPresentation();
        int expected = Bulk - Removed + Moving + 2;
        Require(presentation.RetainedObjectCount == expected,
            $"{presentation.RetainedObjectCount} retained objects, expected {expected}");
        Require(_releasedMesh is null, "released mesh still open");
        double[] sorted = _publishUs.ToArray();
        Array.Sort(sorted);
        double Percentile(double p) => sorted[(int)Math.Round((sorted.Length - 1) * p)];
        Console.WriteLine(
            $"GRAPHICS_FACTS {{\"mode\":\"{(_snapshot ? "snapshot" : "changes")}\",\"updates\":{_updates}," +
            $"\"publishUsP50\":{Percentile(0.5):F1},\"publishUsP95\":{Percentile(0.95):F1},\"publishUsMax\":{sorted[^1]:F1}," +
            $"\"eventUs\":[{string.Join(",", _eventUs.Select(us => us.ToString("F1")))}]," +
            $"\"retainedObjects\":{presentation.RetainedObjectCount},\"resources\":{presentation.ResourceCount}," +
            $"\"missingJoint\":\"{_missingJoint.Replace("\"", "'")}\"}}");
        Console.WriteLine(_failures.Count == 0
            ? "GRAPHICS_EXERCISE_PASSED"
            : "GRAPHICS_EXERCISE_FAILED " + string.Join("; ", _failures));
        _reported = true;
    }

    private void Put(ulong id, ulong? parent, Transform transform, Appearance appearance, bool visible = true)
    {
        AppearanceFact fact = new(id, parent.HasValue, parent ?? 0, transform, appearance, visible, RenderLayer.Scene);
        _facts[id] = fact;
        _changed[id] = fact;
    }

    private void Remove(ulong id)
    {
        _facts.Remove(id);
        _removed.Add(id);
    }

    private Transform Mover(int index)
    {
        float phase = _updates * 0.05f + index;
        return At(new Vector3(index % 10 * 3 + MathF.Sin(phase), 1, -10 - index / 10 * 3 + MathF.Cos(phase)), 0.5f);
    }

    private static Transform Grid(int index, float offset) =>
        At(new Vector3(index % 80, 0, offset + index / 80), 0.8f);

    private static Transform At(Vector3 position, float scale = 1) =>
        new(position, Quaternion.Identity, new Vector3(scale));

    private void Require(bool condition, string failure)
    {
        if (!condition) _failures.Add(failure);
    }

    private MeshResourceCreateRequest Pyramid(float height)
    {
        Vector3[] positions = { new(-0.5f, 0, -0.5f), new(0.5f, 0, -0.5f), new(0.5f, 0, 0.5f), new(-0.5f, 0, 0.5f), new(0, height, 0) };
        Vector3[] normals = positions.Select(p => Vector3.Normalize(p + Vector3.UnitY)).ToArray();
        Vector2[] uvs = positions.Select(p => new Vector2(p.X + 0.5f, p.Z + 0.5f)).ToArray();
        uint[] indices = { 0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4, 0, 2, 1, 0, 3, 2 };
        return new MeshResourceCreateRequest(positions, normals, uvs, indices,
            new[] { new MeshGroup(0, 0, (uint)indices.Length) },
            new[] { new MeshMaterialBinding(0, _material) });
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public bool CompleteTimeline(ProductTimelineCompletion completion) => false;
    public void Dispose()
    {
        if (!_reported) Console.WriteLine("GRAPHICS_EXERCISE_INCOMPLETE");
    }
}
