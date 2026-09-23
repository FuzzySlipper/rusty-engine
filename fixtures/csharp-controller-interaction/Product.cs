using System.Numerics;
using System.Text;
using System.Text.Json;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.Interaction;
using Rusty.Engine.Input;

namespace CsharpControllerInteraction;

/// <summary>Ordinary package-consuming gameplay: Engine mechanisms, local bindings and chest rules.</summary>
public sealed class Product : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule, IWorldInteractionScene
{
    private const float Reach = 2.7f, Acquire = .35f, Release = .5f, QueryDistance = 15, EyeHeight = .65f;
    private const uint UiRootNode = 0;
    private const string UiKeys = "opentitlecontents";
    private static readonly string[] ContainerContents = ["Brass compass and survey note.", "Field ration and signal flare.", "Occluded test crate contents."];
    private readonly IEngineContext engine;
    private readonly SpatialSession spatial;
    private readonly Camera camera;
    private readonly WorldInteraction interaction;
    private readonly InteractionDebugModule interactionDebug;
    private readonly UiStream uiStream;
    private readonly List<(ulong Id, Appearance Appearance, Transform Transform)> visuals = new();
    private readonly List<CharacterObstacle> obstacles = new();
    private readonly List<SpatialEntityCollider> colliders = new();
    private readonly Vector3[] points = [new(-.65f,1.2f,-5), new(.65f,1.2f,-5), new(4,1.2f,-6)];
    private readonly bool[] opened = new bool[3];
    private readonly ulong[] revisions = [1,1,1];
    private Vector3 position = new(0,.95f,0);
    private LookState look;
    private CharacterMotion motion;
    private readonly CharacterControllerConfig characterConfig;
    private ulong sequence;
    private ulong sceneRevision;
    private ulong uiSequence;
    private bool locked;
    private string viewpoint = "free";
    private int uses;
    private int openPanelChest = -1;
    private InteractionReason lastUse = InteractionReason.NoCandidate;
    private readonly FpsInput input = new(FpsInputConfig.Standard);
    private readonly Appearance focusedAppearance;
    private readonly Appearance openedAppearance;

    public ProductUpdateResult Update(ProductUpdate update)
    {
        float dt = (float)(update.Facts.FixedDeltaSeconds * update.Facts.AdmittedStepCount);
        FpsInputFrame frame = input.Consume(update.Input, dt);
        if (frame.Movement != Vector2.Zero || frame.PointerDelta != Vector2.Zero || frame.ControllerLookRadians != Vector2.Zero)
            viewpoint = "free";
        look = input.IntegrateLook(look,frame).After;
        if (input.Physical.Pressed(KeyboardControl.KeyK)) { locked = !locked; revisions[0]++; sceneRevision++; }
        if (input.Physical.Pressed(KeyboardControl.Escape)) CloseContainerPanel();
        for (uint admitted=0; admitted<update.Facts.AdmittedStepCount; admitted++)
        {
            bool jump = frame.JumpPressed && admitted==0;
            // Engine owns contact/support solving; the product supplies movement policy and current obstacles.
            var command = new CharacterControllerCommand(frame.Movement,look.YawRadians,jump,frame.JumpHeld,frame.CrouchHeld,Vector3.Zero,Vector3.Zero,(float)update.Facts.FixedDeltaSeconds,++sequence);
            var step = engine.Spatial.ProposeCharacterStep(new(spatial,position,motion,Support(),obstacles.ToArray(),ReadOnlyMemory<CharacterMeshInstance>.Empty,characterConfig,command));
            position = step.Transform.Translation;
            motion = step.Motion;
        }
        int cycle = input.Physical.Pressed(KeyboardControl.KeyQ) || input.Physical.Pressed(ControllerButton.Button5) ? 1 : 0;
        sceneRevision++;
        interaction.Update(cycle);
        if (frame.UsePressed) lastUse = interaction.UseFocused().Reason;
        Publish();
        return ProductUpdateResult.None;
    }

    public Product(ProductCreateContext context)
    {
        engine = context.Engine;
        uiStream = engine.Ui.OpenStream(new UiStreamRequest("controller-interaction", "controller-interaction.panel.v1"));
        spatial = engine.Spatial.CreateSession(new SpatialSessionConfig(.25,16,VoxelSurfaceMode.GreedyCubes));
        characterConfig = engine.Spatial.DefaultCharacterControllerConfig();
        motion = new(Vector3.Zero,Vector3.Zero,false,CharacterStance.Standing,0,0,0,false,0,Vector3.Zero,Vector3.Zero,Quaternion.Identity,Vector3.Zero,position.Y,position.Y,0,0);
        AddBox(100,new(0,-.5f,-4),new(24,1,24),new(.2f,.23f,.27f,1));
        AddBox(101,new(3,1.5f,-4),new(2,3,.3f),new(.35f,.35f,.4f,1));
        for(int i=0;i<points.Length;i++) AddBox((ulong)i+11,points[i],new(.9f,.8f,.8f),new(.65f,.35f,.1f,1));
        focusedAppearance = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube,false,new Color(.1f,.9f,.2f,1)));
        openedAppearance = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube,false,new Color(.1f,.3f,.9f,1)));
        camera = engine.CameraView.CreateCamera(CameraDescriptor());
        engine.CameraView.SetActiveCamera(camera);
        interaction = new WorldInteraction(this);
        interactionDebug = new InteractionDebugModule(interaction);
        Publish();
    }

    private CharacterSupport Support() => motion.SupportEntityPresent
        ? new(true,CharacterSupportLifecycle.Active,motion.SupportEntity,obstacles.First(x=>x.Entity==motion.SupportEntity).Transform) : default;
    private void AddBox(ulong id, Vector3 center, Vector3 size, Color color)
    {
        Appearance appearance = engine.Graphics.CreatePrimitive(new(PrimitiveGeometry.Cube,false,color));
        visuals.Add((id,appearance,new(center,Quaternion.Identity,size)));
        obstacles.Add(new(id,new(center,Quaternion.Identity,Vector3.One),-size/2,size/2,true,Vector3.Zero,Vector3.Zero));
        colliders.Add(new(id,center-size/2,center+size/2,uint.MaxValue,uint.MaxValue,true,true,false));
    }
    private CameraDescriptor CameraDescriptor() => new(new(position+Vector3.UnitY*EyeHeight,float.RadiansToDegrees(look.PitchRadians),float.RadiansToDegrees(look.YawRadians)),CameraBasisMode.Derived,default,new(CameraProjectionKind.Perspective,70,0,.05,100),CameraViewports.Full);
    private Vector3 Forward => Look.Rebase(new(look,look,new(1,1,-1.5f,1.5f,MathF.PI,false,false,true))).Forward;
    private InteractionQuery Query(Vector3 origin,Vector3 direction) => new(origin,direction,Acquire,Release,QueryDistance,QueryDistance,DistanceOrigin:position+Vector3.UnitY*EyeHeight);
    private InteractionQuery Reticle => Query(position+Vector3.UnitY*EyeHeight,Forward);
    private InteractionCandidate[] Candidates()
    {
        InteractionCandidate[] result = new InteractionCandidate[3];
        for(int i=0;i<result.Length;i++)
        {
            var visibility = InteractionVisibilityQuery.Cast(engine.Spatial,spatial,position+Vector3.UnitY*EyeHeight,points[i],new(uint.MaxValue,uint.MaxValue),colliders.ToArray(),new ulong[]{(ulong)i+11});
            result[i] = new(new((ulong)i+11,revisions[i]),i==0?"left chest":i==1?"right chest":"behind wall",points[i],Reach,visibility,opened[i]?InteractionAvailability.Unavailable:i==0&&locked?InteractionAvailability.Locked:InteractionAvailability.Available);
        }
        return result;
    }
    public InteractionSceneSnapshot ReadInteraction() => new(Reticle,Candidates(),$"scene-{sceneRevision}","open container");
    public InteractionActionResult UseInteraction(InteractionTarget target)
    {
        int index = checked((int)target.Id - 11);
        if ((uint)index >= opened.Length) return new(false,"That target is not a container.");
        if (target.Revision != revisions[index]) return new(false,"The container changed; inspect it again.");
        if (opened[index]) return new(false,"The container is already open.");
        if (index == 0 && locked) return new(false,"The left chest is locked.");
        opened[index] = true;
        revisions[index]++;
        openPanelChest = index;
        uses++;
        sceneRevision++;
        PublishContainerPanel();
        return new(true,$"Opened {ContainerTitle(index)}.");
    }
    private void Publish()
    {
        engine.Graphics.PublishSnapshot(visuals.Select(v=>new AppearanceFact(v.Id,false,0,v.Transform,v.Id is >=11 and <=13 ? (opened[(int)v.Id-11] ? openedAppearance : interaction.Focus.Selected?.Id==v.Id ? focusedAppearance : v.Appearance) : v.Appearance,true,RenderLayer.Scene)).ToArray());
        engine.CameraView.UpdateCamera(new(camera,CameraDescriptor()));
        PublishContainerPanel();
    }
    private void PublishContainerPanel()
    {
        engine.Ui.PublishProjection(new UiProjection(uiStream,++uiSequence,UiValue()));
    }

    private void CloseContainerPanel()
    {
        if (openPanelChest < 0) return;
        openPanelChest = -1;
        sceneRevision++;
    }
    private static string ContainerTitle(int index) => index switch { 0 => "Left chest", 1 => "Right chest", 2 => "Behind-wall chest", _ => "Container" };
    private UiValue UiValue()
    {
        bool open = openPanelChest >= 0;
        string title = open ? ContainerTitle(openPanelChest) : "Container";
        string contents = open ? ContainerContents[openPanelChest] : string.Empty;
        uint titleOffset = (uint)Encoding.UTF8.GetByteCount(UiKeys);
        uint contentsOffset = titleOffset + (uint)Encoding.UTF8.GetByteCount(title);
        byte[] utf8 = Encoding.UTF8.GetBytes(UiKeys + title + contents);
        StructuredValueNode[] nodes =
        [
            new(StructuredValueKind.Object,0,0,0,0,0,0,0,3),
            new(StructuredValueKind.Bool,open ? 1u : 0u,0,0,4,0,0,0,0),
            new(StructuredValueKind.String,0,0,4,5,titleOffset,(uint)Encoding.UTF8.GetByteCount(title),0,0),
            new(StructuredValueKind.String,0,0,9,8,contentsOffset,(uint)Encoding.UTF8.GetByteCount(contents),0,0),
        ];
        return new UiValue(nodes,new uint[]{1,2,3},UiRootNode,utf8);
    }
    [DebugCommand("viewpoint.visit",Description="Visit a product-owned inspection pose: entrance, near, or side. This explicitly moves the player; it is not ordinary-input evidence.")]
    public string Visit(string name)
    {
        Vector3 destination = name switch {
            "entrance" => new(0,.92f,0),
            "near" => new(0,.92f,-2.6f),
            "side" => new(-2,.92f,-2),
            _ => throw new ArgumentException("Unknown viewpoint; choose entrance, near, or side.",nameof(name)),
        };
        if (!CameraQueries.TryLookAtPose(destination+Vector3.UnitY*EyeHeight,new(0,1.2f,-5),0,out var pose))
            throw new InvalidOperationException("Viewpoint does not define a look direction.");
        position = destination;
        motion = new(Vector3.Zero,Vector3.Zero,false,CharacterStance.Standing,0,0,0,false,0,Vector3.Zero,Vector3.Zero,Quaternion.Identity,Vector3.Zero,position.Y,position.Y,0,0);
        look = new((float)double.DegreesToRadians(pose.YawDegrees),(float)double.DegreesToRadians(pose.PitchDegrees));
        input.Physical.Clear();
        interaction.Focus.Clear();
        viewpoint = name;
        sceneRevision++;
        Publish();
        return Observe();
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) { registrar.Register(this); registrar.Register(interactionDebug); }
    [DebugCommand("interaction.query",Description="Read-only reticle candidates; semantic targeting enabled, look assistance disabled.")]
    public string Observe() => Format(interaction.Focus.Observe(Candidates(),Reticle),"reticle");
    [DebugCommand("interaction.cursor",Description="Read-only free-cursor query: viewport-local bottom-left normalized x/y and explicit viewport width/height aspect. Does not turn camera or change focus.")]
    public string Cursor(float x,float y,double aspect)
    {
        CameraRay ray=CameraQueries.Ray(CameraDescriptor(),aspect,new(x,y));
        return Format(interaction.Focus.Observe(Candidates(),Query(ray.Origin,ray.Direction)),"free-cursor");
    }
    private string Format(InteractionReadout result,string mode) => JsonSerializer.Serialize(new {
        mode, viewpoint, semanticTargeting=true,lookAssistance=false,position=new[]{position.X,position.Y,position.Z},yaw=look.YawRadians,pitch=look.PitchRadians,
        selected=result.Selected?.Id,reason=result.Reason.ToString(),uses,lastUse=lastUse.ToString(),
        candidates=result.Candidates.Select(x=>new { id=x.Candidate.Target.Id,revision=x.Candidate.Target.Revision,label=x.Candidate.Label,
            point=new[]{x.Candidate.Point.X,x.Candidate.Point.Y,x.Candidate.Point.Z},distance=x.Distance,angleRadians=x.AngleRadians,
            visibility=x.Candidate.Visibility.ToString(),route=x.Candidate.Route.ToString(),reason=x.Reason.ToString(),selected=x.Selected }) });
    public void Start() { }
    public void Pause() { input.Physical.Clear(); interaction.Focus.Clear(); }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }
    public void Dispose() { engine.Graphics.PublishSnapshot([]); uiStream.Dispose(); camera.Dispose(); focusedAppearance.Dispose(); openedAppearance.Dispose(); foreach(var v in visuals)v.Appearance.Dispose(); spatial.Dispose(); }
}
