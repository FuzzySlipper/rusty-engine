using System.Numerics;
using System.Text.Json;
using System.Text.Json.Serialization;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpWorldStreaming;

public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const float StepSeconds = 1f / 60f;
    private const uint ChunkEdge = 8;
    private const uint Variant = 7, InitialOrientation = 1, EditedOrientation = 2;
    private const int ProofSteps = 60, VisibleSteps = 120, VariantCellIndex = 2;
    private const ulong CharacterVisualId = 1;
    private const float SwimSpeed = 3, SwimAcceleration = 10, WaterDrag = 2;
    private const float WaterGravity = 1, WaterBuoyancy = 1, ClimbSpeed = 2, ClimbReach = .5f;
    private const float FlightSpeed = 2, FlightAcceleration = 10, HoverTolerance = .001f;
    private const float CameraFov = 60, CameraNear = .05f, CameraFar = 100;
    private static readonly Vector3 StartPosition = new(-2,1,0), ClimbTop = new(-2,4,0);
    private static readonly Vector3 WaterMinimum = new(-5,-5,-5), WaterMaximum = new(5,5,5);
    private static readonly Vector3 CameraEye = new(8,7,12), CameraTarget = new(1,1,0), CharacterSize = new(.6f,1.8f,.6f);
    private static readonly Color CharacterColor = new(1,.8f,.1f,1), BaseColor = new(.1f,.6f,.9f,1), VariantColor = new(.1f,1,.15f,1);
    private static readonly VoxelAddress VariantAddress = new(VariantCellIndex,0,0);
    private readonly IEngineContext engine;
    private readonly SpatialSession spatial;
    private readonly SpatialSession movementScene;
    private readonly Camera camera;
    private readonly Appearance character;
    private readonly Material baseMaterial;
    private readonly Material variantMaterial;
    private readonly CharacterControllerConfig config;
    private VoxelScenePresentation? presentation;
    private VoxelPreparationRequest preparation;
    private string streaming = "pending";
    private string scenario = "idle";
    private int steps;
    private ulong sequence;
    private Vector3 position = StartPosition;
    private CharacterMotion motion;
    private CharacterStepReceipt lastStep;
    private CharacterMovementRequest movement;
    private bool stateProof;
    private bool modeProof;
    private uint pendingObservations;
    private readonly List<IDisposable> owners = [];

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        spatial = engine.Spatial.CreateSession(new(1, ChunkEdge, VoxelSurfaceMode.GreedyCubes));
        movementScene = engine.Spatial.CreateSession(new(1, ChunkEdge, VoxelSurfaceMode.GreedyCubes));
        config = engine.Spatial.DefaultCharacterControllerConfig();
        character = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube, false, CharacterColor));
        baseMaterial = MakeMaterial(BaseColor);
        variantMaterial = MakeMaterial(VariantColor);
        CameraQueries.TryLookAtPose(CameraEye, CameraTarget, 0, out CameraPose pose);
        camera = engine.CameraView.CreateCamera(new(pose, CameraBasisMode.Derived, default, new(CameraProjectionKind.Perspective,CameraFov,0,CameraNear,CameraFar), CameraViewports.Full));
        engine.CameraView.SetActiveCamera(camera);
        owners.AddRange([spatial, movementScene, character, baseMaterial, variantMaterial, camera]);
    }
    private Material MakeMaterial(Color color) => engine.Graphics.CreateMaterial(new(color, default(RenderResourceReference), 1, new Color(1,1,1,1), Vector3.Zero, 0, false));
    public void Start()
    {
        ExerciseModes();
        uint[] slots = new uint[ChunkEdge * ChunkEdge * ChunkEdge];
        uint[] states = new uint[slots.Length];
        slots[0] = slots[VariantCellIndex] = 1;
        states[VariantCellIndex] = VoxelCellState.Encode(InitialOrientation, Variant);
        VoxelResidencyOperation[] operations = [new(VoxelResidencyOperationKind.Admit, new(0,0,0), 0, 0, (uint)slots.Length)];
        VoxelPreparationReceipt started = engine.Voxel.StartResidencyPreparation(new(states, spatial, 0, VoxelResidencyHistoryPolicy.ResetToPublishedAuthority, operations, slots));
        preparation = new(spatial, started.Preparation);
        Array.Clear(slots); Array.Clear(states); // The worker must own copied input.
        StartMovement("swim");
    }
    private void ExerciseModes()
    {
        foreach (string mode in new[] { "swim", "climb", "fly" })
        {
            StartMovement(mode);
            for (int index=0; index<ProofSteps; index++) AdvanceMovement();
            if (mode == "swim") Require(lastStep.Movement.HeadSubmerged && Math.Abs(position.Y-StartPosition.Y)<HoverTolerance, "submerged hover");
            if (mode == "climb") Require(lastStep.Movement.ClimbAttached && position.Y>StartPosition.Y+1, "climb travel");
            if (mode == "fly") Require(position.Y>StartPosition.Y+1, "flight travel");
            movement = default;
            float before = position.Y;
            motion = motion with { ControlledVelocity = Vector3.Zero };
            AdvanceMovement();
            Require(lastStep.Movement.Mode == CharacterMovementMode.Walking && position.Y<before, "mode release restores gravity");
        }
        modeProof = true;
    }
    [DebugCommand("movement.start")]
    public string StartMovement(string mode)
    {
        scenario = mode;
        position = StartPosition;
        motion = new(Vector3.Zero,Vector3.Zero,false,CharacterStance.Standing,0,0,0,false,0,Vector3.Zero,Vector3.Zero,Quaternion.Identity,Vector3.Zero,position.Y,position.Y,0,0);
        movement = mode switch {
            "swim" => new(CharacterMovementMode.Swimming,0,SwimSpeed,SwimAcceleration,WaterDrag,WaterMinimum,WaterMaximum,WaterGravity,WaterBuoyancy,0),
            "climb" => new(CharacterMovementMode.Climbing,1,ClimbSpeed,0,0,StartPosition,ClimbTop,0,0,ClimbReach),
            "fly" => new(CharacterMovementMode.Flying,1,FlightSpeed,FlightAcceleration,0,default,default,0,0,0),
            _ => throw new ArgumentException("swim, climb or fly",nameof(mode))
        };
        steps = 0;
        return Inspect();
    }
    private void AdvanceMovement()
    {
        var command = new CharacterControllerCommand(movement,Vector2.Zero,0,false,false,false,Vector3.Zero,Vector3.Zero,StepSeconds,++sequence);
        lastStep = engine.Spatial.ProposeCharacterStep(new(movementScene,position,motion,default,ReadOnlyMemory<CharacterObstacle>.Empty,ReadOnlyMemory<CharacterMeshInstance>.Empty,config,command));
        position = lastStep.Transform.Translation;
        motion = lastStep.Motion;
        steps++;
    }
    public ProductUpdateResult Update(ProductUpdate update)
    {
        if (streaming == "pending")
        {
            var polled = engine.Voxel.PollResidencyPreparation(preparation);
            if (polled.Status == VoxelPreparationStatus.Pending) pendingObservations++;
            else
            {
                Require(engine.Voxel.ReadScene(new(spatial)).SolidVoxelCount == 0, "prepare published early");
                Require(engine.Voxel.CommitResidencyPreparation(preparation).Status == VoxelPreparationStatus.Committed, "commit status");
                ExerciseState();
                presentation = engine.VoxelScenePresentation.ProjectSceneDirectional(new(spatial, new VoxelSceneMaterialBinding[] { new(1,baseMaterial) }, new VoxelSceneFaceMaterialBinding[] { new(Variant,1,SpatialFace.PosY,variantMaterial), new(Variant,1,SpatialFace.PosZ,variantMaterial) }));
                streaming = "committed";
            }
        }
        for (uint admitted=0; admitted<update.Facts.AdmittedStepCount && steps<VisibleSteps; admitted++) AdvanceMovement();
        Publish();
        return ProductUpdateResult.None;
    }
    private void ExerciseState()
    {
        var address = VariantAddress;
        uint expected = VoxelCellState.Encode(InitialOrientation,Variant);
        Require(engine.Voxel.Read(new(spatial,address)).State == expected,"residency state");
        ulong revision = engine.Voxel.ReadScene(new(spatial)).SourceRevision;
        engine.Voxel.ApplyEdits(new(spatial,revision,new VoxelEdit[] { new(VoxelCellState.Encode(EditedOrientation,Variant),VoxelEditKind.Set,address,1) }));
        engine.Voxel.Undo(new(spatial));
        Require(engine.Voxel.Read(new(spatial,address)).State == expected,"state undo");
        engine.Voxel.Redo(new(spatial));
        ReadOnlyMemory<byte> saved = engine.Voxel.ExportHistory(new(spatial));
        engine.Voxel.Undo(new(spatial));
        engine.Voxel.RestoreHistory(new(spatial,saved));
        Require(engine.Voxel.Read(new(spatial,address)).State == VoxelCellState.Encode(EditedOrientation,Variant),"state restore");
        stateProof = true;
    }
    private void Publish() => engine.Graphics.PublishSnapshot(new AppearanceFact[] { new(CharacterVisualId,false,0,new(position,Quaternion.Identity,CharacterSize),character,true,RenderLayer.Scene) });
    [DebugCommand("streaming.inspect")]
    public string Inspect() => JsonSerializer.Serialize(new StreamingProof(streaming, modeProof, stateProof, pendingObservations, scenario, steps, new[] {position.X,position.Y,position.Z}, lastStep.Movement.Mode.ToString(), lastStep.Movement.HeadSubmerged, lastStep.Movement.ClimbAttached), ProofJsonContext.Default.StreamingProof);
    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    private static void Require(bool condition,string message) { if (!condition) throw new InvalidOperationException(message); }
    public void Attach() { if (presentation is not null) engine.VoxelScenePresentation.RefreshScene(presentation); Publish(); }
    public void Pause() { }
    public void Resume() { }
    public void Restart()
    {
        if (streaming == "pending" && preparation.Preparation != 0) { engine.Voxel.CancelResidencyPreparation(preparation); streaming = "cancelled"; }
        StartMovement("swim");
    }
    public void Shutdown() { }
    public void Dispose() { if (streaming == "pending" && preparation.Preparation != 0) engine.Voxel.CancelResidencyPreparation(preparation); presentation?.Dispose(); foreach(var owner in owners.AsEnumerable().Reverse()) owner.Dispose(); }
}

internal sealed record StreamingProof(string Streaming, bool ModeProof, bool StateProof,
    uint PendingObservations, string Scenario, int Steps, float[] Position, string Mode,
    bool HeadSubmerged, bool ClimbAttached);

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(StreamingProof))]
internal partial class ProofJsonContext : JsonSerializerContext { }
