using System.Numerics;
using Rusty.Engine;

namespace CsharpMeshComposition;

/// <summary>
/// A small ordinary product that owns the procedural effect's shape and input
/// policy. Graphics retains the copied mesh and realizes its presentation.
/// </summary>
public sealed class Product : IEngineProduct
{
    private const int RingSegments = 48;
    private const ulong PrimaryObjectId = 7_787_001;
    private const ulong EchoObjectId = 7_787_002;
    private const ulong EffectLightId = 7_787_003;

    private readonly IEngineContext _engine;
    private readonly Material _innerMaterial;
    private readonly Material _outerMaterial;
    private readonly Camera _camera;
    private readonly Light _light;
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
        _camera = CreateCamera();
        _light = _engine.Graphics.CreateLight(new LightRequest(
            EffectLightId,
            false,
            0,
            new LightDescriptor(
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
                LightShadowIntent.Disabled)));
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

    public void Dispose()
    {
        ReleaseRing();
        _engine.CameraView.ClearActiveCamera(new ClearActiveCameraRequest(0));
        _light.Dispose();
        _camera.Dispose();
        _outerMaterial.Dispose();
        _innerMaterial.Dispose();
        _uiStream.Dispose();
    }

    private Material CreateMaterial(Color color, Vector3 emission) => _engine.Graphics.CreateMaterial(
        new MaterialRequest(
            color,
            new RenderResourceHandle(0),
            0.35f,
            new Color(1, 1, 1, 1),
            emission,
            2.6f,
            true));

    private Camera CreateCamera()
    {
        Camera camera = _engine.CameraView.CreateCamera(new CameraDescriptor(
            new CameraPose(new Vector3(0, 0, 6), 0, 0),
            CameraBasisMode.Explicit,
            new CameraBasis(-Vector3.UnitZ, Vector3.UnitX, Vector3.UnitY),
            new CameraProjection(CameraProjectionKind.Perspective, 42, 0, 0.1, 100),
            new CameraViewport(0, 0, 1, 1)));
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

        _engine.Graphics.PublishSnapshot(
        [
            new AppearanceFact(
                PrimaryObjectId,
                false,
                0,
                new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One),
                primary,
                true,
                RenderLayer.Scene),
            new AppearanceFact(
                EchoObjectId,
                false,
                0,
                new Transform(
                    new Vector3(0, 0, -0.25f),
                    Quaternion.CreateFromAxisAngle(Vector3.UnitZ, 0.18f),
                    new Vector3(0.72f)),
                echo,
                true,
                RenderLayer.Scene),
        ]);

        PresentationReadout readout = _engine.Graphics.ReadPresentation();
        Require(readout.RetainedObjectCount == 2, "mesh snapshot did not retain both appearances");
        Require(readout.AppearanceCount == 2, "one mesh resource did not retain two appearances");
        Require(readout.MaterialCount == 2, "mesh resource did not retain both material slots");
        Require(readout.ResourceCount == 1, "mesh resource admission was not visible in presentation readout");
        PublishUi(readout);
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
        ring.Primary.Dispose();
        ring.Echo.Dispose();
        ring.Mesh.Dispose();
        _ring = null;

        PresentationReadout released = _engine.Graphics.ReadPresentation();
        Require(released.AppearanceCount == 0 && released.ResourceCount == 0
                && released.MaterialCount == 2,
            "mesh release did not retain its materials until the resource was released");
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
