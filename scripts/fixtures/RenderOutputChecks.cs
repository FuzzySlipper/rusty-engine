using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Numerics;
using System.Text;
using Rusty.Engine;
using Rusty.Engine.Implicit;

// Engine-owned packaged consumer: exercises the public safe API, including the
// real renderer and normal GLB import. No provider types or downstream checkout.
internal sealed class RenderOutputChecks
{
    private readonly IEngineContext _engine;
    private readonly string _directory;
    private readonly Camera _camera;
    private readonly List<(string Name, RenderOutput Output)> _jobs = new();
    private readonly List<IDisposable> _owners = new();
    private bool _finished;
    private RenderOutput _missing;
    private int _batch;
    private readonly bool _reopen;
    private const ulong StaticObject = 101;
    private const ulong GeneratedObject = 102;
    private const ulong AnimatedObject = 103;

    public RenderOutputChecks(IEngineContext engine)
    {
        _engine = engine;
        _directory = Environment.GetEnvironmentVariable("RUSTY_OUTPUT_TEST_DIR")
            ?? throw new InvalidOperationException("Missing output fixture directory.");
        Directory.CreateDirectory(_directory);
        _reopen = Environment.GetEnvironmentVariable("RUSTY_OUTPUT_REOPEN") == "1";
        _camera = engine.CameraView.CreateCamera(new(
            new(new(0, 1.5f, 4), 0, 0), CameraBasisMode.Derived, default,
            new(CameraProjectionKind.Orthographic, 55, 4, .01, 100), new(0, 0, 1, 1)));
        engine.CameraView.SetActiveCamera(_camera);
        Appearance source = OpenGlb(_reopen ? "content/export-static.glb" : "content/static.glb");
        _owners.Add(source);
        Appearance generated;
        if (_reopen)
            generated = OpenGlb("content/export-generated.glb");
        else
        {
            Material material = engine.Graphics.CreateMaterial(new(new(.2f, .8f, .3f, 1), default, 1, new(1, 1, 1, 1), default, 0, false));
            _owners.Add(material);
            using ImplicitRecipe recipe = new(engine.ImplicitSurfaces);
            ImplicitNode shape = recipe.Sphere(new(0, 1, 0), .65f);
            MeshResource mesh = engine.ImplicitSurfaces.Generate(new ImplicitGenerateRequest(
                recipe.Field, shape, new(-1, 0, -1), new(1, 2, 1), .2f,
                0, 1, material, ReadOnlyMemory<ImplicitMaterialRegion>.Empty));
            _owners.Add(mesh);
            generated = engine.Graphics.CreateMeshAppearance(mesh);
            // Field disposal precedes capture and export requests below.
        }
        _owners.Add(generated);
        RenderResource skin = engine.Animation.OpenAnimatedMesh(new(_reopen ? "content/export-animated.glb" : "content/animated.glb"));
        _owners.Add(skin);
        Appearance animated = engine.Animation.CreateAnimatedMeshAppearance(new(skin));
        _owners.Add(animated);
        Transform identity = new(Vector3.Zero, Quaternion.Identity, Vector3.One);
        Appearance child = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, new(.8f, .2f, .1f, 1)));
        _owners.Add(child);
        Transform edited = _reopen ? identity : new(new(.2f, 0, 0), Quaternion.CreateFromAxisAngle(Vector3.UnitY, .2f), Vector3.One);
        var facts = new List<AppearanceFact> {

            new(StaticObject, false, 0, edited, source, true, RenderLayer.Scene),
            new(GeneratedObject, false, 0, edited, generated, true, RenderLayer.Scene),
            new(AnimatedObject, false, 0, identity, animated, true, RenderLayer.Scene),
        };
        if (!_reopen) facts.Add(new(104, true, GeneratedObject,
            new(new(.5f, 1.3f, 0), Quaternion.Identity, new(.3f)), child, true, RenderLayer.Scene));
        engine.Graphics.PublishSnapshot(facts.ToArray());
        if (!_reopen) {
            QueueCapture("selected-child", 104);
            _jobs.Add(("export-child.glb", engine.RenderOutput.ExportSceneGlb(new(104, false))));
        }
        QueueCapture("static", StaticObject);
        QueueCapture("generated", GeneratedObject);
        _jobs.Add(("antialias.png", engine.RenderOutput.CaptureImage(Request(GeneratedObject) with { Samples = 4 })));
        _jobs.Add(("dim.png", engine.RenderOutput.CaptureImage(Request(GeneratedObject) with { Exposure = .5f })));
        _jobs.Add(("aces.png", engine.RenderOutput.CaptureImage(Request(GeneratedObject) with { ToneMapping = CaptureToneMapping.AcesFilmic })));
        _jobs.Add(("background.png", engine.RenderOutput.CaptureImage(Request(GeneratedObject) with { Background = new(.25f, .5f, .75f, .5f) })));
        QueueCapture("pose-start", AnimatedObject, 0);
        QueueCapture("pose-middle", AnimatedObject, .5);
        QueueCapture("pose-repeat", AnimatedObject, .5);
        if (!_reopen)
        {
            _jobs.Add(("export-static.glb", engine.RenderOutput.ExportSceneGlb(new(StaticObject, false))));
            _jobs.Add(("export-generated.glb", engine.RenderOutput.ExportSceneGlb(new(GeneratedObject, false))));
            _jobs.Add(("export-animated.glb", engine.RenderOutput.ExportSceneGlb(new(AnimatedObject, true))));
        }
        _missing = engine.RenderOutput.CaptureImage(Request(999999));
        using RenderOutput cancelled = engine.RenderOutput.CaptureImage(Request(GeneratedObject));
        engine.RenderOutput.Cancel(cancelled);
        if (engine.RenderOutput.Read(cancelled).State != RenderOutputState.Cancelled)
            throw new InvalidOperationException("Cancelled output remained pending.");
    }
    private Appearance OpenGlb(string path)
    {
        RenderResource resource = _engine.Animation.OpenAnimatedMesh(new(path));
        _owners.Add(resource);
        return _engine.Animation.CreateAnimatedMeshAppearance(new(resource));
    }
    private CaptureImageRequest Request(ulong source, double? pose = null) => new(
        source, _camera, 96, 80, new(0, 0, 0, 0), false, 1, CaptureToneMapping.None, 0,
        pose.HasValue ? AnimatedObject : 0, pose.HasValue ? "run" : "", pose ?? 0);
    private void QueueCapture(string name, ulong source, double? pose = null) =>
        _jobs.Add(($"{name}.png", _engine.RenderOutput.CaptureImage(Request(source, pose))));
    public void Tick()
    {
        if (_finished) return;
        if (_missing != null)
        {
            if (_engine.RenderOutput.Read(_missing).State != RenderOutputState.Failed
                || _engine.RenderOutput.ReadDiagnostic(_missing).Length == 0)
                throw new InvalidOperationException("Missing capture source did not fail its output job.");
            _missing.Dispose();
            _missing = null;
        }
        foreach ((string name, RenderOutput output) in _jobs)
        {
            RenderOutputReadout status = _engine.RenderOutput.Read(output);
            if (status.State == RenderOutputState.Failed)
                throw new InvalidOperationException($"{name}: {Encoding.UTF8.GetString(_engine.RenderOutput.ReadDiagnostic(output).Span)}");
            if (status.State != RenderOutputState.Completed) return;
        }
        foreach ((string name, RenderOutput output) in _jobs)
        {
            byte[] bytes = _engine.RenderOutput.ReadBytes(output).ToArray();
            if (bytes.Length < 16) throw new InvalidOperationException($"Empty output {name}");
            File.WriteAllBytes(Path.Combine(_directory, name), bytes);
            output.Dispose();
        }
        _jobs.Clear();
        if (_batch++ == 0)
        {
            byte[] middle = File.ReadAllBytes(Path.Combine(_directory, "pose-middle.png"));
            if (!middle.SequenceEqual(File.ReadAllBytes(Path.Combine(_directory, "pose-repeat.png"))))
                throw new InvalidOperationException("Exact pose capture changed across repeated jobs.");
            if (middle.SequenceEqual(File.ReadAllBytes(Path.Combine(_directory, "pose-start.png"))))
                throw new InvalidOperationException("Different sampled poses produced identical output.");
            QueueCapture("batch-reuse", GeneratedObject);
            return;
        }
        _engine.Graphics.PublishSnapshot([]);
        for (int i = _owners.Count - 1; i >= 0; --i) _owners[i].Dispose();
        _camera.Dispose();
        File.WriteAllText(Path.Combine(_directory, "complete"), _reopen ? "reopened" : "exported");
        _finished = true;
    }
}
