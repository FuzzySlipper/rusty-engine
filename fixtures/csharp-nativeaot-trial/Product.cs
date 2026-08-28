using System.Numerics;
using Rusty.Engine;

namespace CsharpNativeAotTrial;

// This project owns only trusted product meaning. The composition project
// receives the generated native ABI and exports as internal source.
public sealed class Product : IEngineProduct
{
    private readonly IEngineContext _engine;
    private readonly Rng _rng;
    private readonly Rng _forkedRng;
    private readonly SpatialSession _spatial;
    private readonly VoxelChunkLease _voxelLease;
    private readonly UiStreamHandle _uiStream;
    private readonly Appearance _appearance;
    private readonly Material _material;
    private readonly Camera _camera;
    private readonly MechanicsCatalog _mechanicsCatalog;
    private readonly MechanicsEntity _mechanicsEntity;
    private readonly PersistenceStore _persistenceStore;
    private int _turns;
    private float _x;
    private bool _started;
    private bool _paused;
    private bool _shutdown;
    private ulong _uiSequence;
    private ulong _lastRandom;
    private LookState _look;
    private bool _physicalMappingConfigured;
    private InputBinding? _mappedBinding;
    private int _mappedHeldTurns;
    private bool _mappingReleasePending;
    private bool _mappingReleaseVerified;
    private bool _updateFactsSeen;
    private ulong _lastUpdateGeneration;
    private ulong _lastUpdateControlRevision;

    public Product(ProductCreateContext context)
    {
        _engine = context.Engine;
        Require(context.Input.Context.Value.Span.SequenceEqual("gameplay.default"u8), "input context did not reach Product.Game");
        bool payloadIntentFound = false;
        foreach (ProductInputDescriptor descriptor in context.Input.DirectIntents.Span)
        {
            if (!descriptor.Id.Span.SequenceEqual("runtime.exercise"u8))
            {
                continue;
            }
            Require(descriptor.ValueKind == InputValueKind.ProductPayload, "input descriptor value kind did not reach Product.Game");
            Require(descriptor.PayloadContract.Span.SequenceEqual("runtime.exercise.payload"u8), "input descriptor contract did not reach Product.Game");
            payloadIntentFound = true;
        }
        Require(payloadIntentFound, "configured payload input descriptor did not reach Product.Game");
        bool mappingIntentFound = false;
        foreach (ProductInputDescriptor descriptor in context.Input.DirectIntents.Span)
        {
            if (!descriptor.Id.Span.SequenceEqual("runtime.exercise.move"u8))
            {
                continue;
            }
            Require(descriptor.ValueKind == InputValueKind.Digital, "physical mapping intent descriptor value kind did not reach Product.Game");
            mappingIntentFound = true;
        }
        foreach (ProductInputMapping mapping in context.Input.PhysicalMappings.Span)
        {
            if (!mapping.Id.Span.SequenceEqual("runtime.exercise.move"u8))
            {
                continue;
            }
            Require(mappingIntentFound, "physical mapping arrived without its typed intent descriptor");
            Require(mapping.Intent.Span.SequenceEqual("runtime.exercise.move"u8), "physical mapping intent did not reach Product.Game");
            Require(mapping.TriggerKind == InputTriggerKind.Key, "physical mapping trigger kind did not reach Product.Game");
            Require(mapping.Edge == InputEdge.Held, "physical mapping edge did not reach Product.Game");
            Require(mapping.Keyboard == KeyboardControl.KeyW, "physical mapping keyboard control did not reach Product.Game");
            Require(mapping.Context.Value.IsEmpty, "unscoped physical mapping unexpectedly carried a context");
            _physicalMappingConfigured = true;
        }
        _mechanicsCatalog = _engine.Mechanics.CreateCatalog(new MechanicsCatalogCreateRequest("nativeaot_trial"));
        _engine.Mechanics.DefineStat(new MechanicsStatDefinitionRequest(_mechanicsCatalog, "strength", 0, 100));
        _engine.Mechanics.DefineTrack(new MechanicsTrackDefinitionRequest(_mechanicsCatalog, "stamina", 0, MechanicsTrackMaximumKind.Stat, 0, "strength"));
        _engine.Mechanics.DefineTrack(new MechanicsTrackDefinitionRequest(_mechanicsCatalog, "focus", 0, MechanicsTrackMaximumKind.Fixed, 10, string.Empty));
        _engine.Mechanics.DefineContribution(new MechanicsContributionDefinitionRequest(_mechanicsCatalog, "trial_bonus", 0, "strength", MechanicsContributionKind.Add, 2, 0, 0, "trial_bonus", MechanicsStackingPolicy.Sum));
        _engine.Mechanics.AdmitCatalog(_mechanicsCatalog);
        _mechanicsEntity = _engine.Mechanics.BindEntity(new MechanicsEntityBindRequest(_mechanicsCatalog, 41, "trial_actor"));
        _engine.Mechanics.SetInitialStat(new MechanicsInitialStatRequest(_mechanicsEntity, "strength", 10));
        _engine.Mechanics.SetInitialTrack(new MechanicsInitialTrackRequest(_mechanicsEntity, "stamina", 12));
        _engine.Mechanics.SetInitialTrack(new MechanicsInitialTrackRequest(_mechanicsEntity, "focus", 10));
        _engine.Mechanics.BindIntrinsicSource(new MechanicsIntrinsicSourceRequest(_mechanicsEntity, "trial_bonus_instance", "trial_bonus"));
        MechanicsEntityReceipt mechanicsReceipt = _engine.Mechanics.CommitEntity(_mechanicsEntity);
        MechanicsStatEvaluationLeaseReceipt strength = _engine.Mechanics.EvaluateStat(new MechanicsStatOperationRequest(
            _mechanicsEntity,
            "strength",
            "trial_evaluate",
            ReadOnlyMemory<MechanicsRequestSource>.Empty));
        if (strength.Value != 12)
        {
            throw new InvalidOperationException("exact mechanics contribution did not apply");
        }
        MechanicsTrackMutationLeaseReceipt staminaSpend = _engine.Mechanics.SpendTrack(new MechanicsTrackMutationRequest(_mechanicsEntity, "trial_spend", "trial_spend_source", "stamina", 2, MechanicsRevisionGuard.Exact, mechanicsReceipt.TracksRevision));
        if (staminaSpend.After != 10)
        {
            throw new InvalidOperationException("exact mechanics spend did not preserve the track bound");
        }
        _persistenceStore = _engine.Persistence.OpenStore(new PersistenceOpenRequest(
            Path.Combine(Path.GetTempPath(), "rusty-engine-nativeaot-lease-fixture")));
        const string leaseKey = "fixtures/café";
        byte[] leasePayload = [0x00, 0xC3, 0xA9, 0xFF];
        _engine.Persistence.Save(new PersistenceSaveRequest(
            _persistenceStore,
            leaseKey,
            1,
            PersistenceRevisionGuard.Any,
            0,
            leasePayload));
        using (PersistenceBlob loaded = _engine.Persistence.Load(new PersistenceLoadRequest(_persistenceStore, leaseKey)))
        {
            Require(_engine.Persistence.ReadBlobBytes(loaded).Span.SequenceEqual(leasePayload),
                "native byte lease did not copy and release its payload");
        }
        using (RulesPackage rulesPackage = _engine.Rules.AdmitPackage(new RulesPackageAdmitRequest(
            """
            {"kind":"rusty.gameplay-rules.package","schemaVersion":1,"domain":"fixture","package":"nativeaot","version":1,"dependencies":[],"sources":[],"provenance":[],"payload":{"machines":[{"output":10}]}}
            """u8.ToArray())))
        {
            RulesPackageReadoutLeaseReceipt packageReadout = _engine.Rules.ReadPackage(rulesPackage);
            RulesPackageReadoutRow parent = packageReadout.Packages.Span[0];
            RulesResolvedPackageSetLeaseReceipt resolved = _engine.Rules.ResolvePackages(
                new RulesResolvePackagesRequest(new RulesPackage[] { rulesPackage }));
            Require(resolved.Packages.Length == 1
                && resolved.Packages.Span[0].Package == "nativeaot"
                && resolved.Aggregate.DependencyCount == 0,
                "rules resolution did not copy deterministic package facts");
            ReadOnlyMemory<RulesPayloadSelectionRow> selected = _engine.Rules.SelectPayload(
                new RulesSelectPayloadRequest(
                    rulesPackage,
                    parent.Fingerprint,
                    new RulesPayloadPathSegment[] {
                        new RulesPayloadPathSegment(RulesPayloadPathSegmentKind.Field, "machines", 0),
                        new RulesPayloadPathSegment(RulesPayloadPathSegmentKind.Index, string.Empty, 0),
                        new RulesPayloadPathSegment(RulesPayloadPathSegmentKind.Field, "output", 0),
                    }));
            Require(selected.Length == 1 && selected.Span[0].ParentFingerprint == parent.Fingerprint
                && selected.Span[0].CanonicalBytes.Span.SequenceEqual("10"u8),
                "rules payload selection did not copy the requested field/index subtree");
            ExpectEngineFailure(() => _engine.Rules.SelectPayload(new RulesSelectPayloadRequest(
                rulesPackage,
                new string('0', 64),
                new RulesPayloadPathSegment[] {
                    new RulesPayloadPathSegment(RulesPayloadPathSegmentKind.Field, "machines", 0),
                })));
        }
        _appearance = _engine.Appearance.CreatePrimitive(new PrimitiveAppearanceRequest(PrimitiveGeometry.Cube, false, new Color(0.25f, 0.75f, 1.0f, 1.0f)));
        Material createdMaterial = _engine.Appearance.CreateMaterial(new MaterialRequest(
            new Color(0.25f, 0.75f, 1.0f, 1.0f),
            new RenderResourceHandle(0),
            0.5f,
            new Color(1, 1, 1, 1),
            Vector3.Zero,
            0,
            false));
        _engine.Appearance.UpdateMaterial(new MaterialUpdateRequest(createdMaterial, new MaterialRequest(
            new Color(0.5f, 0.5f, 1, 1), new RenderResourceHandle(0), 0.25f, new Color(1, 1, 1, 1), Vector3.Zero, 0, true)));
        _material = _engine.Appearance.ReplaceMaterial(new MaterialUpdateRequest(createdMaterial, new MaterialRequest(
            new Color(1, 1, 1, 1), new RenderResourceHandle(0), 1, new Color(1, 1, 1, 1), Vector3.Zero, 0, false)));
        createdMaterial.Dispose();
        CameraDescriptor initialCamera = new(
            new CameraPose(new Vector3(0, 1, 3), 0, 0),
            CameraBasisMode.Explicit,
            new CameraBasis(new Vector3(0, 0, -1), Vector3.UnitX, Vector3.UnitY),
            new CameraProjection(CameraProjectionKind.Perspective, 65, 0, 0.1, 100),
            new CameraViewport(0, 0, 1, 1));
        Camera createdCamera = _engine.CameraView.CreateCamera(initialCamera);
        _engine.CameraView.SetActiveCamera(createdCamera);
        _engine.CameraView.UpdateCamera(new CameraUpdateRequest(
            createdCamera,
            initialCamera with { Pose = new CameraPose(new Vector3(0, 1, 3), 5, 15) }));
        _camera = _engine.CameraView.ReplaceCamera(new CameraReplaceRequest(
            createdCamera,
            initialCamera with { Projection = new CameraProjection(CameraProjectionKind.Orthographic, 0, 8, 0.1, 100) }));
        createdCamera.Dispose(); // replaced camera is an Engine-recognized tombstone.
        _engine.CameraView.SetActiveCamera(_camera);
        KeyedRngReceipt keyed = _engine.Random.DrawKeyed(new KeyedRngRequest(17, "nativeaot-trial", "create", -10, 10));
        if (keyed != _engine.Random.DrawKeyed(new KeyedRngRequest(17, "nativeaot-trial", "create", -10, 10)))
        {
            throw new InvalidOperationException("keyed random sequence changed during creation");
        }
        _rng = _engine.Random.CreateScoped(new ScopedRngCreateRequest(17, "nativeaot-trial"));
        _forkedRng = _engine.Random.ForkScoped(new ScopedRngForkRequest(_rng, "child"));
        _lastRandom = _engine.Random.NextU64(_forkedRng).Value;
        _uiStream = _engine.Ui.OpenStream(new UiStreamRequest("nativeaot-trial", "nativeaot.trial.hud"));
        _spatial = _engine.Spatial.CreateSession(new SpatialSessionConfig(1.0, 16, 0));
        VoxelSceneReadout initialVoxelScene = _engine.Voxel.ReadScene(new VoxelSceneReadRequest(_spatial));
        Require(initialVoxelScene.Present && initialVoxelScene.ChunkSize == 16 && initialVoxelScene.SourceRevision == 0,
            "voxel scene facts did not reach C#");
        VoxelAddress exercisedVoxel = new(4, 0, 4);
        VoxelEditReceipt voxelEdit = _engine.Voxel.ApplyEdits(new VoxelEditTransaction(
            _spatial,
            initialVoxelScene.SourceRevision,
            new[] { new VoxelEdit(VoxelEditKind.Set, exercisedVoxel, 3) }));
        Require(voxelEdit.AcceptedRevision == 1 && voxelEdit.ChangedVoxels == 1 && voxelEdit.CollisionRevision == 1
            && voxelEdit.NavigationRevision == 1 && voxelEdit.MeshRevision == 1,
            "voxel edit did not publish coherent projection revisions");
        VoxelReadout exercisedReadout = _engine.Voxel.Read(new VoxelReadRequest(_spatial, exercisedVoxel));
        Require(exercisedReadout.Present && exercisedReadout.MaterialSlot == 3,
            "voxel material readout did not preserve the accepted edit");
        SpatialProjectionReadout sharedVoxelProjection = _engine.Spatial.ReadProjection(
            new SpatialProjectionReadRequest(_spatial));
        Require(sharedVoxelProjection.SourceRevision == voxelEdit.AcceptedRevision
            && sharedVoxelProjection.AuthorityHash == voxelEdit.AuthorityHash,
            "spatial projection did not observe the canonical voxel authority");
        ExpectEngineFailure(() => _engine.Voxel.ApplyEdits(new VoxelEditTransaction(
            _spatial,
            initialVoxelScene.SourceRevision,
            new[] { new VoxelEdit(VoxelEditKind.Clear, exercisedVoxel, 0) })));
        Require(_engine.Voxel.Read(new VoxelReadRequest(_spatial, exercisedVoxel)).MaterialSlot == 3,
            "rejected stale voxel edit changed canonical state");
        VoxelHistoryCursorReadout voxelCursor = _engine.Voxel.ReadHistoryCursor(
            new VoxelHistoryCursorReadRequest(_spatial));
        Require(voxelCursor.EntryCount == 1 && voxelCursor.UndoDepth == 1,
            "voxel history cursor did not retain the accepted transaction");
        VoxelHistoryEntryReadout voxelEntry = _engine.Voxel.ReadHistoryEntryAt(
            new VoxelHistoryEntryAtRequest(_spatial, 0));
        VoxelHistoryDeltaReadout voxelDelta = _engine.Voxel.ReadHistoryDeltaAt(
            new VoxelHistoryDeltaAtRequest(_spatial, 0, 0));
        Require(voxelEntry.Present && voxelEntry.DeltaCount == 1 && voxelDelta.Present
            && voxelDelta.Address == exercisedVoxel && !voxelDelta.BeforeMaterialPresent
            && voxelDelta.AfterMaterialPresent && voxelDelta.AfterMaterial == 3,
            "bounded voxel history readouts did not describe the accepted delta");
        VoxelHistoryReceipt voxelUndo = _engine.Voxel.Undo(new VoxelHistoryActionRequest(_spatial));
        Require(voxelUndo.Applied && !_engine.Voxel.Read(new VoxelReadRequest(_spatial, exercisedVoxel)).Present,
            "voxel undo did not restore the prior authority");
        VoxelHistoryReceipt voxelRedo = _engine.Voxel.Redo(new VoxelHistoryActionRequest(_spatial));
        Require(voxelRedo.Applied && _engine.Voxel.Read(new VoxelReadRequest(_spatial, exercisedVoxel)).MaterialSlot == 3,
            "voxel redo did not restore the accepted authority");
        VoxelChunkReadout exercisedChunk = _engine.Voxel.ReadChunk(new VoxelChunkReadRequest(
            _spatial,
            new VoxelChunkIdentity(0, 0, 0)));
        Require(exercisedChunk.Present && exercisedChunk.SolidVoxelCount == 1,
            "voxel chunk readout did not describe the edited resident chunk");
        _voxelLease = _engine.Voxel.AcquireChunkLease(new VoxelChunkLeaseRequest(
            _spatial,
            exercisedChunk.Chunk));
        VoxelChunkLeaseReadout leaseReadout = _engine.Voxel.ReadChunkLease(
            new VoxelChunkLeaseReadRequest(_voxelLease));
        Require(leaseReadout.Present && leaseReadout.Chunk == exercisedChunk.Chunk
            && leaseReadout.AcquiredContentHash == exercisedChunk.ContentHash,
            "voxel chunk lease did not retain exact owner evidence");
        NavigationReplaceReceipt hostNavigation = _engine.Spatial.ReplaceNavigation(new NavigationReplaceRequest(
            _spatial,
            new PlanarNavConfig(1, 1.0, 16, 0),
            new[] { new PlanarNavCell(0, 0, 0), new PlanarNavCell(1, 0, 0) }));
        NavigationProjectionReadout hostProjection = _engine.Spatial.ReadNavigationProjection(
            new NavigationProjectionReadRequest(_spatial));
        Require(hostProjection.Present && hostProjection.Kind == NavigationProjectionKind.HostWalkableCells && hostProjection.NavigationRevision == hostNavigation.NavigationRevision,
            "host navigation projection facts did not reach C#");
        NavigationPathReadout hostPath = _engine.Spatial.RequestNavigationPath(new NavigationPathRequest(
            _spatial, new PlanarNavCell(0, 0, 0), new PlanarNavCell(1, 0, 0), 16));
        NavigationPathCellAtReceipt hostPathCell = _engine.Spatial.ReadNavigationPathCellAt(
            new NavigationPathCellAtRequest(_spatial, 1));
        Require(hostPath.Outcome == NavigationPathOutcome.Reached && hostPath.PathLen == 2 && hostPathCell.Present && hostPathCell.Cell == new PlanarNavCell(1, 0, 0),
            "bounded indexed host navigation path did not reach C#");
        NavigationStepReceipt hostStep = _engine.Spatial.ProposeNavigationStep(new NavigationStepRequest(
            _spatial,
            new Vector3(0.5f),
            new Vector3(1.5f, 0.5f, 0.5f),
            0.5f,
            16));
        Require(hostStep.Outcome == NavigationPathOutcome.Reached, "typed navigation step outcome did not reach C#");
        NavigationReplaceReceipt voxelNavigation = _engine.Spatial.ReplaceVoxelNavigation(new NavigationVoxelReplaceRequest(
            _spatial,
            new PlanarNavConfig(2, 1.0, 16, 1),
            1,
            true,
            new[] { new PlanarNavCell(0, 0, 0), new PlanarNavCell(1, 0, 0), new PlanarNavCell(2, 0, 0) }));
        NavigationProjectionReadout voxelProjection = _engine.Spatial.ReadNavigationProjection(
            new NavigationProjectionReadRequest(_spatial));
        Require(voxelProjection.Present && voxelProjection.Kind == NavigationProjectionKind.VoxelDerived && voxelProjection.NavigationRevision == voxelNavigation.NavigationRevision,
            "voxel-derived navigation projection facts did not reach C#");
        NavigationPathReadout voxelPath = _engine.Spatial.RequestNavigationPath(new NavigationPathRequest(
            _spatial, new PlanarNavCell(0, 1, 0), new PlanarNavCell(2, 1, 0), 16));
        Require(voxelPath.Outcome == NavigationPathOutcome.Reached && voxelPath.PathLen == 3,
            "voxel-derived planar navigation path did not reach C#");
        NavigationPathReadout volumetricPath = _engine.Spatial.RequestVolumetricNavigationPath(new NavigationVolumetricPathRequest(
            _spatial,
            new PlanarNavCell(0, 1, 0),
            new PlanarNavCell(2, 1, 0),
            16,
            new NavigationVolumetricConfig(1, 1, 1, NavigationVolumetricNeighborSet.Planar4, NavigationVolumetricVerticalPolicy.DisallowVertical, NavigationVolumetricTraversalRule.EmptyCells)));
        Require(volumetricPath.Outcome == NavigationPathOutcome.Reached && volumetricPath.PathLen == 3,
            "bounded volumetric navigation path did not reach C#");
        _engine.Spatial.ClearNavigation(new NavigationClearRequest(_spatial));
        NavigationProjectionReadout clearedNavigation = _engine.Spatial.ReadNavigationProjection(
            new NavigationProjectionReadRequest(_spatial));
        Require(!clearedNavigation.Present && clearedNavigation.NavigationRevision > voxelNavigation.NavigationRevision,
            "navigation clear did not invalidate the retained source");
        ExerciseCharacterController();
        ExerciseLook();
        ExerciseDynamics();
    }

    public void Start()
    {
        _started = true;
        _paused = false;
        PublishPresentation();
        PresentationReadout presentation = _engine.Appearance.ReadPresentation();
        Require(presentation.RetainedObjectCount == 1 && presentation.AppearanceCount == 1 && presentation.MaterialCount == 1, "appearance readout did not report retained Engine presentation facts");
    }

    public void Update(ProductUpdate update)
    {
        if (!_started || _paused || _shutdown)
        {
            throw new InvalidOperationException("the product is not accepting updates");
        }
        ProductUpdateFacts facts = update.Facts;
        Require(facts.LifecycleState == ProductLifecycleState.Running, "lifecycle facts did not identify a running update");
        Require(facts.Generation != 0 && facts.ControlRevision != 0, "lifecycle facts did not carry the current identity");
        Require(facts.AdmittedStepCount != 0, "update facts did not carry admitted simulation steps");
        Require(facts.SimulationStep + facts.AdmittedStepCount >= facts.SimulationStep, "simulation step facts overflowed");
        if (facts.Mode == ProductTurnKind.Realtime)
        {
            Require(facts.ObservedHostTimeNanoseconds != 0, "realtime update facts did not carry host observation");
            Require(facts.FixedStepHz != 0 && facts.FixedDeltaSeconds > 0, "realtime update facts did not carry fixed-step timing");
        }
        else
        {
            Require(facts.ObservedHostTimeNanoseconds == 0 && facts.FixedStepHz == 0 && facts.FixedDeltaSeconds == 0, "non-realtime update facts carried realtime-only timing");
        }
        _updateFactsSeen = true;
        _lastUpdateGeneration = facts.Generation;
        _lastUpdateControlRevision = facts.ControlRevision;
        bool releaseObservedThisTurn = false;
        bool mappedHeldThisTurn = false;
        foreach (ProductInputEvent input in update.Input)
        {
            if (input.Kind == InputEventKind.Key && input.Keyboard == KeyboardControl.KeyW && input.Edge == InputEdge.Pressed)
            {
                Require(input.Kind == InputEventKind.Key, "typed input event kind did not reach Product.Game");
                Require(input.Edge == InputEdge.Pressed, "typed input edge did not reach Product.Game");
                Require(input.Device == InputDevice.Keyboard, "typed input device did not reach Product.Game");
                Require(input.Channel == InputChannel.Key, "typed input channel did not reach Product.Game");
                Require(input.Context.Value.Span.SequenceEqual("gameplay.default"u8), "input event context did not reach Product.Game");
                Require(input.Sequence.Value != 0, "input event sequence did not reach Product.Game");
                _x += 1.0f;
            }
            if (input.Kind == InputEventKind.Key && input.Keyboard == KeyboardControl.KeyW && input.Edge == InputEdge.Released && _physicalMappingConfigured)
            {
                Require(input.Device == InputDevice.Keyboard && input.Channel == InputChannel.Key, "typed release input device or channel did not reach Product.Game");
                Require(input.Context.Value.Span.SequenceEqual("gameplay.default"u8), "typed release input context did not reach Product.Game");
                Require(input.Sequence.Value != 0, "typed release input sequence did not reach Product.Game");
            }
            if (input.Kind == InputEventKind.Key && input.Edge == InputEdge.Released && _physicalMappingConfigured)
            {
                Require(_mappedHeldTurns >= 2, "physical mapping did not produce Held envelopes across admitted turns before release");
                releaseObservedThisTurn = true;
            }
            if (input.Kind == InputEventKind.MappedDigital && _physicalMappingConfigured)
            {
                Require(input.MappingId.Span.SequenceEqual("runtime.exercise.move"u8), "mapped input identity did not reach Product.Game");
                Require(input.Intent.Span.SequenceEqual("runtime.exercise.move"u8), "mapped input intent did not reach Product.Game");
                if (_mappedBinding is InputBinding mappedBinding)
                {
                    Require(input.Binding == mappedBinding, "mapped input binding changed during one held sequence");
                }
                else
                {
                    _mappedBinding = input.Binding;
                }
                Require(input.Phase == InputPhase.Held && input.Edge == InputEdge.Held, "mapped input did not retain its Held phase");
                Require(input.Provenance == InputProvenance.Physical, "mapped input provenance did not retain physical origin");
                Require(input.ValueKind == InputValueKind.Digital && input.X == 1.0f, "mapped input digital value did not reach Product.Game");
                Require(input.Sequence.Value != 0, "mapped input sequence did not reach Product.Game");
                Require(!_mappingReleasePending && !releaseObservedThisTurn, "stale Held mapping survived a release");
                mappedHeldThisTurn = true;
                _mappedHeldTurns++;
            }
            if (input.Kind == InputEventKind.PointerDelta)
            {
                _look = _engine.Look.Integrate(new LookRequest(_look, new Vector2(input.X, input.Y), LookConfig())).After;
            }
            if (input.Kind == InputEventKind.DirectProductPayload)
            {
                Require(input.PayloadContract.Span.SequenceEqual("runtime.exercise.payload"u8), "payload contract did not reach Product.Game");
                Require(input.PayloadData.Span.SequenceEqual("{\"exercise\":true}"u8), "payload data did not reach Product.Game");
            }
        }
        using (StandardExactDefinition exact = _engine.StandardExact.Admit(new StandardExactAdmitRequest(
            "fixture",
            "nativeaot-exact",
            1,
            "trial.damage",
            "trial-source",
            "rules/trial.exact",
            false,
            0,
            false,
            0,
            new StandardExactRole[] { new("self", 0, 0) },
            Array.Empty<StandardExactCapability>(),
            new StandardExactNode[] {
                new(StandardExactNodeKind.Literal, 7, StandardExactInputKind.Parameter, string.Empty, string.Empty, 0, 0, 0, 0, 0, 0, 0),
            },
            Array.Empty<uint>(),
            0)))
        {
            StandardExactReadoutLeaseReceipt exactReadout = _engine.StandardExact.ReadDefinition(exact);
            ReadOnlyMemory<StandardExactEvaluationRow> exactResult = _engine.StandardExact.Evaluate(
                new StandardExactEvaluateRequest(exact, Array.Empty<StandardExactEvidence>()));
            Require(exactReadout.Definitions.Length == 1
                && exactReadout.Definitions.Span[0].Family == "exact"
                && exactReadout.Roles.Length == 1
                && exactReadout.Roles.Span[0].CapabilitiesLen == 0
                && exactResult.Length == 1
                && exactResult.Span[0].Value == 7
                && exactResult.Span[0].WorkUsed == 1,
                "StandardExact definition did not retain canonical identity, empty role, and measured evaluation work");
        }
        if (_mappingReleasePending && !releaseObservedThisTurn)
        {
            Require(!mappedHeldThisTurn, "released physical mapping remained Held on the next admitted turn");
            _mappingReleasePending = false;
            _mappingReleaseVerified = true;
        }
        if (releaseObservedThisTurn)
        {
            _mappingReleasePending = true;
        }
        _turns++;
        _lastRandom = _engine.Random.NextBoundedU32(new ScopedRngBoundedRequest(_rng, 100)).Value;
        PublishPresentation();
    }

    public void Pause() => _paused = true;
    public void Resume() => _paused = false;
    public void Shutdown()
    {
        Require(_updateFactsSeen, "typed lifecycle update facts never reached Product.Game");
        Require(_lastUpdateGeneration != 0 && _lastUpdateControlRevision != 0, "typed lifecycle update identity was not retained");
        if (_physicalMappingConfigured)
        {
            Require(_mappedHeldTurns >= 2, "physical mapping exercise did not reach two admitted Held turns");
            Require(_mappingReleaseVerified, "physical mapping release was not observed to end Held delivery");
        }
        // Release the retained Engine projection before owner disposal.
        _engine.Appearance.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
        _shutdown = true;
    }

    public void Dispose()
    {
        _ = _engine.Random.NextBool(_rng);
        _forkedRng.Dispose();
        _rng.Dispose();
        _camera.Dispose();
        _appearance.Dispose();
        _material.Dispose();
        _voxelLease.Dispose();
        _spatial.Dispose();
        _mechanicsEntity.Dispose();
        _mechanicsCatalog.Dispose();
        _persistenceStore.Dispose();
    }

    private static LookConfig LookConfig() => new(0.01f, 0.01f, -1.4f, 1.4f, 1.0f, false, false, true);

    private void ExerciseCharacterController()
    {
        CharacterControllerConfig config = _engine.Spatial.DefaultCharacterControllerConfig();
        config = config with
        {
            Jump = config.Jump with { BufferSeconds = 0.15f, CoyoteSeconds = 0.1f },
            Surface = config.Surface with { MaximumStepHeight = 0.35f, FloorSnapDistance = 0.2f },
            ExternalMotion = config.ExternalMotion with { ImpulseScale = 0.75f, DynamicImpulseFactor = 0.5f },
        };
        CharacterMotion motion = new(
            Vector3.Zero, Vector3.Zero, false, CharacterStance.Standing, 0, 0, 0, false, 0,
            Vector3.Zero, Vector3.Zero, Quaternion.Identity, Vector3.Zero, 3.0f, 3.0f, 0, 0);
        CharacterSupport noSupport = new(false, CharacterSupportLifecycle.Active, 0, new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One));
        CharacterControllerCommand firstCommand = new(
            Vector2.Zero, 0, false, false, false, Vector3.Zero, Vector3.Zero, 1.0f / 60.0f, 1);
        CharacterStepReceipt first = _engine.Spatial.ProposeCharacterStep(new CharacterStepRequest(
            _spatial, new Vector3(0, 3, 0), motion, noSupport, config, firstCommand));
        CharacterControllerCommand secondCommand = new(
            new Vector2(0, 1), 0, true, true, false, new Vector3(0.1f, 0, 0), new Vector3(0, 0.5f, 0), 1.0f / 60.0f, 2);
        CharacterStepReceipt second = _engine.Spatial.ProposeCharacterStep(new CharacterStepRequest(
            _spatial, first.Transform.Translation, first.Motion, noSupport, config, secondCommand));
        Require(second.Generation > first.Generation && second.RevisionAfter > second.RevisionBefore,
            "character proposal did not return Engine publication revisions");
        Require(second.Motion.LastCommandSequence == 2 && second.Motion.CollisionWorldHash != 0,
            "character continuity did not remain product-held across proposals");
        CharacterControllerReadout readout = _engine.Spatial.ReadCharacterController(new CharacterControllerReadRequest(_spatial));
        Require(readout.Present && readout.CommandSequence == 2 && readout.Generation == second.Generation,
            "character session readout did not describe the latest proposal");
        CharacterContactAtReceipt contact = _engine.Spatial.ReadCharacterContactAt(new CharacterContactAtRequest(_spatial, 0));
        Require(!contact.Present || contact.Contact.Present, "indexed character contact readout was incoherent");
        CharacterDynamicImpulseAtReceipt impulse = _engine.Spatial.ReadCharacterDynamicImpulseAt(new CharacterDynamicImpulseAtRequest(_spatial, 0));
        Require(!impulse.Present || impulse.Proposal.Entity != 0, "indexed dynamic impulse readout was incoherent");
    }

    private void ExerciseLook()
    {
        LookConfig config = new(1.0f, 1.0f, -0.5f, 0.5f, 2.0f, true, false, true);
        LookState initial = new(0.25f, -0.25f);
        LookRequest request = new(initial, new Vector2(0.75f, 1.0f), config);
        Require(_engine.Look.Diagnose(request) == LookDiagnostic.Accepted, "look rejected a valid request");
        LookReceipt integrated = _engine.Look.Integrate(request);
        Require(integrated.Before == initial, "look receipt omitted the accumulated state before integration");
        Require(MathF.Abs(integrated.After.YawRadians + 0.5f) < 0.0001f, "look horizontal inversion did not reach Engine");
        Require(integrated.After.PitchRadians == config.MaximumPitchRadians, "look pitch clamp did not reach Engine");
        Require(MathF.Abs(integrated.Forward.Length() - 1.0f) < 0.0001f, "look forward basis was not normalized");
        Require(MathF.Abs(integrated.Right.Length() - 1.0f) < 0.0001f, "look right basis was not normalized");
        Require(MathF.Abs(integrated.Up.Length() - 1.0f) < 0.0001f, "look up basis was not normalized");

        LookReceipt rebased = _engine.Look.Rebase(new LookRebaseRequest(
            integrated.After,
            new LookState(MathF.Tau + 0.5f, 2.0f),
            config));
        Require(MathF.Abs(rebased.After.YawRadians - 0.5f) < 0.0001f, "look rebase did not normalize wrapped yaw");
        Require(rebased.After.PitchRadians == config.MaximumPitchRadians, "look rebase did not clamp pitch");
        LookReceipt reset = _engine.Look.Reset(new LookResetRequest(rebased.After));
        Require(reset.Before == rebased.After && reset.After == default, "look reset did not preserve receipt or reset state");

        LookRequest rejected = new(initial, new Vector2(3.0f, 0.0f), config);
        Require(_engine.Look.Diagnose(rejected) == LookDiagnostic.DeltaLimitExceeded, "look diagnostic lost delta-limit cause");
        ExpectEngineFailure(() => _engine.Look.Integrate(rejected));
    }

    private void ExerciseDynamics()
    {
        const uint oneStep = 1;
        const uint rejectedStepCount = 256;
        const float oneSixtiethSecond = 1.0f / 60.0f;
        DynamicsWorld world = _engine.Dynamics.CreateWorld(new DynamicsWorldConfig(Vector3.Zero));
        DynamicsBody body = _engine.Dynamics.CreateBody(new DynamicsCreateBodyRequest(
            world,
            new DynamicsBodyConfig(
                new Transform(new Vector3(0.0f, 2.0f, 0.0f), Quaternion.Identity, Vector3.One),
                new Vector3(0.5f),
                2.0f,
                new AxisLocks(false, false, false, false, false, false),
                0.0f)));
        DynamicsReadout initial = _engine.Dynamics.Read(new DynamicsReadRequest(body));
        Require(initial.MassProperties.PrincipalInertia.X > 0.0f, "Engine did not provide cuboid inertia");
        _ = _engine.Dynamics.Step(new DynamicsStepRequest(
            world,
            oneSixtiethSecond,
            oneStep,
            new[] { new DynamicsAction(body, new Vector3(2.0f, 0.0f, 0.0f), new Vector3(0.0f, 0.0f, 1.0f), Vector3.Zero, Vector3.Zero, true) }));
        DynamicsReadout driven = _engine.Dynamics.Read(new DynamicsReadRequest(body));
        Require(driven.LinearVelocity.X > 0.0f && driven.AngularVelocity.Z > 0.0f, "force and torque did not reach Engine dynamics");
        ExpectEngineFailure(() => _engine.Dynamics.Step(new DynamicsStepRequest(world, oneSixtiethSecond, rejectedStepCount, ReadOnlyMemory<DynamicsAction>.Empty)));
        Require(_engine.Dynamics.Read(new DynamicsReadRequest(body)).Equals(driven), "rejected step partially published dynamics state");
        _engine.Dynamics.Reset(new DynamicsResetRequest(
            body,
            new Transform(new Vector3(3.0f, 2.0f, 0.0f), Quaternion.Identity, Vector3.One),
            Vector3.Zero,
            Vector3.Zero,
            false));
        Require(_engine.Dynamics.Read(new DynamicsReadRequest(body)).Transform.Translation.X == 3.0f, "reset did not publish the requested pose");
        DynamicsBody replacement = _engine.Dynamics.ReplaceBody(new DynamicsReplaceBodyRequest(
            body,
            new DynamicsBodyConfig(
                new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One),
                new Vector3(1.0f, 0.5f, 0.25f),
                3.0f,
                new AxisLocks(false, true, false, true, false, true),
                0.0f)));
        ExpectEngineFailure(() => _engine.Dynamics.Read(new DynamicsReadRequest(body)));
        body.Dispose(); // old owner is a known tombstone after successful replacement.
        replacement.Dispose(); // body-first disposal order.

        DynamicsBodyProperties configured = new(
            4.0f,
            Vector3.Zero,
            Vector3.Zero,
            new AxisLocks(true, false, false, false, true, false),
            0.2f,
            0.3f,
            0.5f,
            0.8f,
            0.4f,
            0x0000_0002u,
            0x0000_0004u,
            true,
            false,
            true);
        DynamicsBody configuredBody = _engine.Dynamics.CreateCuboidBody(new DynamicsCreateCuboidBodyRequest(
            world,
            new DynamicsCuboidBodyConfig(
                new Transform(new Vector3(1.0f, 3.0f, 0.0f), Quaternion.Identity, Vector3.One),
                new Vector3(0.25f, 0.5f, 0.75f),
                configured)));
        _engine.Dynamics.UpdateBody(new DynamicsUpdateBodyRequest(configuredBody, configured with { Sleeping = true }));
        Require(_engine.Dynamics.Read(new DynamicsReadRequest(configuredBody)).Sleeping, "full body properties did not preserve sleep state");
        DynamicsWorldReadout configuredWorld = _engine.Dynamics.ReadWorld(new DynamicsWorldReadRequest(world));
        Require(configuredWorld.BodyCount == 1 && configuredWorld.EntityRevision > 0, "world receipt did not report retained-body revision");
        DynamicsBodyAtReceipt configuredAt = _engine.Dynamics.ReadBodyAt(new DynamicsBodyAtRequest(world, 0));
        Require(configuredAt.Present && configuredAt.Body.Value != 0 && configuredAt.Readout.MassProperties.Mass == 4.0f, "bounded body enumeration lost Engine-owned readout");
        DynamicsBody capsule = _engine.Dynamics.CreateCapsuleBody(new DynamicsCreateCapsuleBodyRequest(
            world,
            new DynamicsCapsuleBodyConfig(
                new Transform(new Vector3(3.0f, 3.0f, 0.0f), Quaternion.Identity, Vector3.One),
                0.75f,
                0.25f,
                configured)));
        Require(!_engine.Dynamics.Read(new DynamicsReadRequest(capsule)).MassProperties.Available, "capsule incorrectly claimed the 7219 inertia readout");
        DynamicsBody sphereReplacement = _engine.Dynamics.ReplaceSphereBody(new DynamicsReplaceSphereBodyRequest(
            configuredBody,
            new DynamicsSphereBodyPropertiesConfig(
                new Transform(new Vector3(1.0f, 4.0f, 0.0f), Quaternion.Identity, Vector3.One),
                0.5f,
                configured)));
        ExpectEngineFailure(() => _engine.Dynamics.Read(new DynamicsReadRequest(configuredBody)));
        configuredBody.Dispose();
        sphereReplacement.Dispose();
        capsule.Dispose();
        world.Dispose();

        _engine.Spatial.ReplaceCollision(new CollisionReplaceRequest(
            _spatial,
            new[] { new StaticMeshAsset(1, 0, 4, 0, 2) },
            new[]
            {
                new Vector3(-10.0f, 0.0f, -10.0f),
                new Vector3(10.0f, 0.0f, -10.0f),
                new Vector3(10.0f, 0.0f, 10.0f),
                new Vector3(-10.0f, 0.0f, 10.0f),
            },
            new[] { new Triangle(0, 1, 2), new Triangle(0, 2, 3) },
            new[] { new StaticMeshInstance(1, 1, new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One)) }));
        DynamicsWorld sphereWorld = _engine.Dynamics.CreateWorld(new DynamicsWorldConfig(Vector3.Zero));
        _engine.Dynamics.BindWorldCollision(new DynamicsWorldCollisionBindingRequest(sphereWorld, _spatial));
        DynamicsBody sphere = _engine.Dynamics.CreateSphereBody(new DynamicsCreateSphereBodyRequest(
            sphereWorld,
            new DynamicsSphereBodyConfig(
                new Transform(new Vector3(0.0f, 0.4f, 0.0f), Quaternion.Identity, Vector3.One),
                0.5f,
                2.0f,
                new AxisLocks(false, false, false, false, false, false),
                0.0f)));
        DynamicsReadout sphereInitial = _engine.Dynamics.Read(new DynamicsReadRequest(sphere));
        Require(
            sphereInitial.MassProperties.PrincipalInertia.X == sphereInitial.MassProperties.PrincipalInertia.Y
            && sphereInitial.MassProperties.PrincipalInertia.Y == sphereInitial.MassProperties.PrincipalInertia.Z,
            "Engine did not provide sphere inertia");
        _ = _engine.Dynamics.Step(new DynamicsStepRequest(sphereWorld, oneSixtiethSecond, oneStep, ReadOnlyMemory<DynamicsAction>.Empty));
        DynamicsReadout sphereContact = _engine.Dynamics.Read(new DynamicsReadRequest(sphere));
        Require(
            sphereContact.ContactCount > 0
            && sphereContact.FirstContact.Present
            && sphereContact.FirstContact.Environment,
            "sphere contact was not projected from Engine dynamics");
        DynamicsContactAtReceipt indexedContact = _engine.Dynamics.ReadContactAt(new DynamicsContactAtRequest(sphereWorld, 0));
        Require(indexedContact.Present && indexedContact.Environment && indexedContact.First.Value != 0 && indexedContact.Second.Value == 0, "bounded world contact receipt lost Engine ownership facts");
        ExpectEngineFailure(() => _engine.Dynamics.Step(new DynamicsStepRequest(sphereWorld, oneSixtiethSecond, rejectedStepCount, ReadOnlyMemory<DynamicsAction>.Empty)));
        Require(_engine.Dynamics.Read(new DynamicsReadRequest(sphere)).Equals(sphereContact), "rejected step partially published contact facts");
        sphere.Dispose();
        sphereWorld.Dispose();

        DynamicsWorld parentFirstWorld = _engine.Dynamics.CreateWorld(new DynamicsWorldConfig(Vector3.Zero));
        DynamicsBody parentFirstBody = _engine.Dynamics.CreateBody(new DynamicsCreateBodyRequest(
            parentFirstWorld,
            new DynamicsBodyConfig(new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One), new Vector3(0.5f), 1.0f, new AxisLocks(false, false, false, false, false, false), 0.0f)));
        parentFirstWorld.Dispose(); // world disposal tombstones children.
        parentFirstBody.Dispose(); // parent-first disposal order is safe and idempotent.
    }

    private static void ExpectEngineFailure(Action action)
    {
        try
        {
            action();
        }
        catch (EngineCallException)
        {
            return;
        }
        throw new InvalidOperationException("expected an Engine call failure");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }

    private void PublishPresentation()
    {
        PublishAppearanceSnapshot();
        _engine.Ui.PublishProjection(new UiProjection(_uiStream, ++_uiSequence, UiValue()));
    }

    private UiValue UiValue()
    {
        StructuredValueNode[] nodes =
        [
            new(5, 0, 0, 0, 0, 0, 0, 0, 3),
            new(2, 0, _turns, 0, 5, 0, 0, 0, 0),
            new(2, 0, _look.YawRadians, 5, 3, 0, 0, 0, 0),
            new(2, 0, _x, 8, 1, 0, 0, 0, 0),
        ];
        return new UiValue(nodes, new uint[] { 1, 2, 3 }, 0, "turnsyawx"u8.ToArray());
    }

    private void PublishAppearanceSnapshot()
    {
        if (_turns >= 2)
        {
            _engine.Appearance.PublishSnapshot(ReadOnlySpan<AppearanceFact>.Empty);
            return;
        }
        _engine.Appearance.PublishSnapshot(
        [
            new AppearanceFact(41, new Transform(new Vector3(_x, 0, 0), Quaternion.Identity, Vector3.One), _appearance, true, RenderLayer.Viewmodel),
        ]);
    }
}
