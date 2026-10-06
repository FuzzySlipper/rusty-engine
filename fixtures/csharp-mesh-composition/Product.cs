using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpMeshComposition;

/// <summary>
/// A small ordinary product that owns the procedural effect's shape and input
/// policy. Graphics retains the copied mesh and realizes its presentation.
/// </summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const int RingSegments = 48;
    private const ulong PrimaryObjectId = 7_787_001;
    private const ulong EchoObjectId = 7_787_002;
    private const ulong EffectLightId = 7_787_003;
    private const ulong BackdropObjectId = 7_787_004;
    // A matte wall behind the ring that receives its shadow.
    private const float BackdropHalfSize = 4.0f, BackdropDepth = -1.2f;
    private static readonly Color BackdropColor = new(0.55f, 0.55f, 0.6f, 1.0f);
    // The echo ring as a viewmodel: camera-local, lower right, close.
    private static readonly Vector3 ViewmodelOffset = new(0.9f, -0.6f, -2.2f);
    private const float ViewmodelScale = 0.3f;

    private readonly IEngineContext _engine;
    private readonly Material _innerMaterial;
    private readonly Material _outerMaterial;
    private readonly Material _backdropMaterial;
    private readonly MeshResource _backdropMesh;
    private readonly Appearance _backdrop;
    private readonly Camera _camera;
    private CameraDescriptor _cameraDescriptor;
    private readonly Light _light;
    private LightDescriptor _lightDescriptor;
    private ShadowCasting _casting = ShadowCasting.Cast;
    private bool _viewmodel;
    private readonly UiStream _uiStream;
    private RingPresentation? _ring;
    private bool _alternatePulse;
    private uint _pulseCount;
    private uint _rebuildCount;
    private ulong _uiSequence;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        _innerMaterial = CreateMaterial(
            color: new Color(0.12f, 0.75f, 1.0f, 1.0f),
            emission: new Vector3(0.02f, 0.65f, 1.0f));
        _outerMaterial = CreateMaterial(
            color: new Color(1.0f, 0.16f, 0.68f, 1.0f),
            emission: new Vector3(1.0f, 0.03f, 0.32f));
        _backdropMaterial = CreateMaterial(BackdropColor, Vector3.Zero);
        _backdropMesh = _engine.Graphics.CreateMeshResource(BuildBackdropMesh());
        _backdrop = _engine.Graphics.CreateMeshAppearance(_backdropMesh);
        _camera = CreateCamera();
        _lightDescriptor = new LightDescriptor(
            LightKind.Point,
            new Vector3(0.5f, 0.75f, 1.0f),
            4.0f,
            true,
            new Vector3(0, 0, 2.5f),
            -Vector3.UnitZ,
            true,
            12.0f,
            2.0f,
            0,
            0,
            LightShadowIntent.Disabled);
        _light = _engine.Graphics.CreateLight(new LightRequest(EffectLightId, false, 0, _lightDescriptor));
        _uiStream = _engine.Ui.OpenStream(new UiStreamRequest(
            "mesh-composition.hud",
            "mesh-composition.ui.snapshot.v1"));

        CreateAndPublishRing();
    }

    public void Start() => PublishUi();

    public ProductUpdateResult Update(ProductUpdate update)
    {
        bool recreate = false;
        foreach (ProductInputEvent input in update.Input)
        {
            if (input.ValueKind != InputValueKind.Digital || input.X < 0.5f)
            {
                continue;
            }

            if (input.Intent.Span.SequenceEqual("mesh.pulse"u8))
            {
                _alternatePulse = !_alternatePulse;
                _pulseCount++;
                recreate = true;
            }
            else if (input.Intent.Span.SequenceEqual("mesh.recreate"u8))
            {
                recreate = true;
            }
        }

        if (recreate)
        {
            ReleaseRing();
            CreateAndPublishRing();
        }

        return ProductUpdateResult.None;
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    /// <summary>
    /// The ring casts (true) or does not cast (false) a shadow on the backdrop
    /// from the effect light, whose shadow this turns on with the renderer's.
    /// </summary>
    [DebugCommand("mesh.shadows")]
    public string Shadows(bool cast)
    {
        _casting = cast ? ShadowCasting.Cast : ShadowCasting.None;
        _lightDescriptor = _lightDescriptor with { ShadowIntent = LightShadowIntent.Requested };
        _engine.Graphics.UpdateLight(new LightUpdateRequest(_light, new LightRequest(EffectLightId, false, 0, _lightDescriptor)));
        _engine.RendererSettings.Set(_engine.RendererSettings.Read().Requested with { Shadows = true });
        PublishRing();
        return $"casting={_casting} shadows={_engine.RendererSettings.Read().Effective.Shadows}";
    }

    /// <summary>
    /// Draw the echo ring as a viewmodel (camera-local) with the viewmodel
    /// layer's own field of view in degrees (0 for the camera's).
    /// </summary>
    [DebugCommand("mesh.viewmodel")]
    public string Viewmodel(bool enabled, double fovYDegrees)
    {
        _viewmodel = enabled;
        _cameraDescriptor = _cameraDescriptor with { ViewmodelFovYDegrees = fovYDegrees };
        _engine.CameraView.UpdateCamera(new CameraUpdateRequest(_camera, _cameraDescriptor));
        PublishRing();
        return $"viewmodel={_viewmodel} fov={fovYDegrees}";
    }

    public void Dispose()
    {
        ReleaseRing();
        _engine.CameraView.ClearActiveCamera(new ClearActiveCameraRequest(0));
        _light.Dispose();
        _camera.Dispose();
        _backdrop.Dispose();
        _backdropMesh.Dispose();
        _backdropMaterial.Dispose();
        _outerMaterial.Dispose();
        _innerMaterial.Dispose();
        _uiStream.Dispose();
    }

    private Material CreateMaterial(Color color, Vector3 emission) => _engine.Graphics.CreateMaterial(
        new MaterialRequest(
            color,
            default,
            0.35f,
            new Color(1, 1, 1, 1),
            emission,
            2.6f,
            true));

    private Camera CreateCamera()
    {
        _cameraDescriptor = new CameraDescriptor(
            new CameraPose(new Vector3(0, 0, 6), 0, 0),
            CameraBasisMode.Explicit,
            new CameraBasis(-Vector3.UnitZ, Vector3.UnitX, Vector3.UnitY),
            new CameraProjection(CameraProjectionKind.Perspective, 42, 0, 0.1, 100),
            new CameraViewport(0, 0, 1, 1));
        Camera camera = _engine.CameraView.CreateCamera(_cameraDescriptor);
        _engine.CameraView.SetActiveCamera(camera);
        return camera;
    }

    private void CreateAndPublishRing()
    {
        MeshResource mesh = _engine.Graphics.CreateMeshResource(BuildRingMesh(_alternatePulse));
        Appearance primary = _engine.Graphics.CreateMeshAppearance(mesh);
        Appearance echo = _engine.Graphics.CreateMeshAppearance(mesh);
        _ring = new RingPresentation(mesh, primary, echo);
        _rebuildCount++;
        PublishRing();

        PresentationReadout readout = _engine.Graphics.ReadPresentation();
        Require(readout.RetainedObjectCount == 3, "mesh snapshot did not retain both ring appearances and the backdrop");
        Require(readout.AppearanceCount == 3, "one mesh resource did not retain two appearances beside the backdrop's");
        Require(readout.MaterialCount == 3, "mesh resources did not retain the ring's two material slots and the backdrop's");
        Require(readout.ResourceCount == 2, "mesh resource admission was not visible in presentation readout");
        PublishUi(readout);
    }

    /// <summary>The ring, its echo (in the scene or as a viewmodel) and the backdrop, with the ring's shadow casting.</summary>
    private void PublishRing()
    {
        RingPresentation? ring = _ring;
        if (ring is null)
        {
            return;
        }

        Transform echoTransform = _viewmodel
            ? new Transform(ViewmodelOffset, Quaternion.CreateFromAxisAngle(Vector3.UnitZ, 0.18f), new Vector3(ViewmodelScale))
            : new Transform(
                new Vector3(0, 0, -0.25f),
                Quaternion.CreateFromAxisAngle(Vector3.UnitZ, 0.18f),
                new Vector3(0.72f));
        _engine.Graphics.PublishSnapshot(
        [
            new AppearanceFact(
                PrimaryObjectId,
                false,
                0,
                new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One),
                ring.Primary,
                true,
                RenderLayer.Scene,
                _casting),
            new AppearanceFact(
                EchoObjectId,
                false,
                0,
                echoTransform,
                ring.Echo,
                true,
                _viewmodel ? RenderLayer.Viewmodel : RenderLayer.Scene,
                _casting),
            new AppearanceFact(
                BackdropObjectId,
                false,
                0,
                new Transform(new Vector3(0, 0, BackdropDepth), Quaternion.Identity, Vector3.One),
                _backdrop,
                true,
                RenderLayer.Scene),
        ]);
    }

    private void ReleaseRing()
    {
        RingPresentation? ring = _ring;
        if (ring is null)
        {
            return;
        }

        // Removal is visible before release. The Engine can then tear down both
        // appearances before this immutable resource releases its materials.
        _engine.Graphics.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        Require(_engine.Graphics.ReadPresentation().RetainedObjectCount == 0,
            "mesh release must first publish a snapshot without appearances");
        _ring = null;
        ring.Primary.Dispose();
        ring.Echo.Dispose();
        ring.Mesh.Dispose();

        PresentationReadout released = _engine.Graphics.ReadPresentation();
        Require(released.AppearanceCount == 1 && released.ResourceCount == 1
                && released.MaterialCount == 3,
            "mesh release did not retain its materials until the resource was released");
    }

    /// <summary>A square facing +Z, two triangles, in the backdrop material.</summary>
    private MeshResourceCreateRequest BuildBackdropMesh()
    {
        float h = BackdropHalfSize;
        Vector3[] positions = [new(-h, -h, 0), new(h, -h, 0), new(h, h, 0), new(-h, h, 0)];
        Vector3[] normals = [Vector3.UnitZ, Vector3.UnitZ, Vector3.UnitZ, Vector3.UnitZ];
        Vector2[] uvs = [new(0, 0), new(1, 0), new(1, 1), new(0, 1)];
        uint[] indices = [0, 1, 2, 0, 2, 3];
        return new MeshResourceCreateRequest(
            positions,
            normals,
            uvs,
            indices,
            new MeshGroup[] { new MeshGroup(0, 0, (uint)indices.Length) },
            new MeshMaterialBinding[] { new MeshMaterialBinding(0, _backdropMaterial) });
    }

    private MeshResourceCreateRequest BuildRingMesh(bool alternatePulse)
    {
        Vector3[] positions = new Vector3[RingSegments * 2];
        Vector3[] normals = new Vector3[positions.Length];
        Vector2[] uvs = new Vector2[positions.Length];
        uint[] indices = new uint[RingSegments * 6];
        const float innerRadius = 1.45f;
        const float baseOuterRadius = 1.78f;
        const float pulseAmplitude = 0.20f;

        for (int segment = 0; segment < RingSegments; segment++)
        {
            float angle = MathF.Tau * segment / RingSegments;
            float pulse = alternatePulse ? MathF.Sin(angle * 6) * pulseAmplitude : 0;
            Vector2 direction = new(MathF.Cos(angle), MathF.Sin(angle));
            int next = (segment + 1) % RingSegments;
            int vertex = segment * 2;
            positions[vertex] = new Vector3(direction * innerRadius, 0);
            positions[vertex + 1] = new Vector3(direction * (baseOuterRadius + pulse), 0);
            normals[vertex] = Vector3.UnitZ;
            normals[vertex + 1] = Vector3.UnitZ;
            uvs[vertex] = new Vector2(segment / (float)RingSegments, 0);
            uvs[vertex + 1] = new Vector2(segment / (float)RingSegments, 1);

            int index = segment * 6;
            uint currentInner = (uint)(segment * 2);
            uint currentOuter = currentInner + 1;
            uint nextInner = (uint)(next * 2);
            uint nextOuter = nextInner + 1;
            indices[index] = currentInner;
            indices[index + 1] = currentOuter;
            indices[index + 2] = nextOuter;
            indices[index + 3] = currentInner;
            indices[index + 4] = nextOuter;
            indices[index + 5] = nextInner;
        }

        uint halfIndexCount = (uint)(RingSegments / 2 * 6);
        return new MeshResourceCreateRequest(
            positions,
            normals,
            uvs,
            indices,
            new MeshGroup[]
            {
                new MeshGroup(0, 0, halfIndexCount),
                new MeshGroup(1, halfIndexCount, (uint)indices.Length - halfIndexCount),
            },
            new MeshMaterialBinding[]
            {
                new MeshMaterialBinding(0, _innerMaterial),
                new MeshMaterialBinding(1, _outerMaterial),
            });
    }

    private void PublishUi() => PublishUi(_engine.Graphics.ReadPresentation());

    private void PublishUi(PresentationReadout readout)
    {
        const string data = "pulsesshaperebuildsobjectsappearancesmaterialsresources";
        string shape = _alternatePulse ? "six-point pulse" : "calm circle";
        byte[] utf8 = System.Text.Encoding.UTF8.GetBytes(data + shape);
        uint shapeOffset = (uint)data.Length;
        StructuredValueNode[] nodes =
        [
            new(StructuredValueKind.Object, 0, 0, 0, 0, 0, 0, 0, 7),
            new(StructuredValueKind.Number, 0, _pulseCount, 0, 6, 0, 0, 0, 0),
            new(StructuredValueKind.String, 0, 0, 6, 5, shapeOffset, (uint)shape.Length, 0, 0),
            new(StructuredValueKind.Number, 0, _rebuildCount, 11, 8, 0, 0, 0, 0),
            new(StructuredValueKind.Number, 0, readout.RetainedObjectCount, 19, 7, 0, 0, 0, 0),
            new(StructuredValueKind.Number, 0, readout.AppearanceCount, 26, 11, 0, 0, 0, 0),
            new(StructuredValueKind.Number, 0, readout.MaterialCount, 37, 9, 0, 0, 0, 0),
            new(StructuredValueKind.Number, 0, readout.ResourceCount, 46, 9, 0, 0, 0, 0),
        ];
        _engine.Ui.PublishProjection(new UiProjection(
            _uiStream,
            ++_uiSequence,
            new UiValue(nodes, new uint[] { 1, 2, 3, 4, 5, 6, 7 }, 0, utf8)));
    }

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }

    private sealed record RingPresentation(MeshResource Mesh, Appearance Primary, Appearance Echo);
}
