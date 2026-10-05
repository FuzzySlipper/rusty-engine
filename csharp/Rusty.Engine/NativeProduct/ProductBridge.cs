using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using Rusty.Engine.Debugging;

namespace Rusty.Engine.NativeProduct;

internal sealed class EngineContext : IEngineContext
{
    internal EngineContext(NativeEngineApi native)
    {
        Input = new InputServiceImplementation(native.input);
        GameplayTime = new GameplayTimeServiceImplementation(native.gameplay_time);
        Diagnostics = new DiagnosticsServiceImplementation(native.diagnostics);
        Audio = new AudioServiceImplementation(native.audio);
        Video = new VideoServiceImplementation(native.video);
        RenderOutput = new RenderOutputServiceImplementation(native.render_output);
        Dynamics = new DynamicsServiceImplementation(native.dynamics);
        Motion = new MotionServiceImplementation(native.motion);
        Kinematic = new KinematicServiceImplementation(native.kinematic);
        Spatial = new SpatialServiceImplementation(native.spatial);
        Perception = new PerceptionServiceImplementation(native.perception);
        WorldOrigin = new WorldOriginServiceImplementation(native.world_origin);
        Voxel = new VoxelServiceImplementation(native.voxel);
        VoxelContent = new VoxelContentServiceImplementation(native.voxel_content);
        VoxelScenePresentation = new VoxelScenePresentationServiceImplementation(native.voxel_scene_presentation);
        Content = new ContentServiceImplementation(native.content);
        AuthoredContent = new AuthoredContentServiceImplementation(native.authored_content);
        Graphics = new GraphicsServiceImplementation(native.graphics);
        ImplicitSurfaces = new ImplicitSurfacesServiceImplementation(native.implicit_surfaces, native.graphics);
        Presentation = new PresentationServiceImplementation(native.presentation);
        Animation = new AnimationServiceImplementation(native.animation, native.graphics);
        CameraView = new CameraViewServiceImplementation(native.camera_view);
        Random = new RngServiceImplementation(native.rng);
        Persistence = new PersistenceServiceImplementation(native.persistence);
        Http = new HttpServiceImplementation(native.http);
        Ui = new UiServiceImplementation(native.ui);
    }

    public IInputService Input { get; }
    public IGameplayTimeService GameplayTime { get; }
    public IDiagnosticsService Diagnostics { get; }
    public IAudioService Audio { get; }
    public IVideoService Video { get; }
    public IRenderOutputService RenderOutput { get; }
    public IDynamicsService Dynamics { get; }
    public IMotionService Motion { get; }
    public IKinematicService Kinematic { get; }
    public ISpatialService Spatial { get; }
    public IPerceptionService Perception { get; }
    public IWorldOriginService WorldOrigin { get; }
    public IVoxelService Voxel { get; }
    public IVoxelContentService VoxelContent { get; }
    public IVoxelScenePresentationService VoxelScenePresentation { get; }
    public IContentService Content { get; }
    public IAuthoredContentService AuthoredContent { get; }
    public IGraphicsService Graphics { get; }
    public IImplicitSurfacesService ImplicitSurfaces { get; }
    public IPresentationService Presentation { get; }
    public IAnimationService Animation { get; }
    public ICameraViewService CameraView { get; }
    public IRandomService Random { get; }
    public IPersistenceService Persistence { get; }
    public IHttpService Http { get; }
    public IUiService Ui { get; }
}

internal sealed class ProductLifetime
{
    private IEngineProduct? _product;
    private Exception? _lastCallError;
    private readonly IDebugCommandCatalog _debugCatalog;
    internal ProductLifetime(IEngineProduct product, IDebugCommandCatalog debugCatalog, ProductDebugExecutionContext debugging) { _product = product; _debugCatalog = debugCatalog; Debugging = debugging; }
    internal IEngineProduct Product => _product ?? throw new ObjectDisposedException(nameof(ProductLifetime));
    internal IDebugCommandCatalog DebugCatalog => _product is null ? throw new ObjectDisposedException(nameof(ProductLifetime)) : _debugCatalog;
    internal ProductDebugExecutionContext Debugging { get; }
    internal void RecordCallError(Exception error) => _lastCallError = error;
    internal void ClearCallError() => _lastCallError = null;
    internal Exception? TakeCallError()
    {
        Exception? error = _lastCallError;
        _lastCallError = null;
        return error;
    }
    internal void Dispose()
    {
        _lastCallError = null;
        try
        {
            System.Threading.Interlocked.Exchange(ref _product, null)?.Dispose();
        }
        catch (Exception exception)
        {
            // Destroy has no error result, so the process stream is the only record.
            Console.Error.WriteLine($"rusty: product Dispose threw {ProductFault.Summary(exception)}");
        }
    }
}

/// <summary>One bounded line naming a product exception: its type, the first line of
/// its message and the first stack frame outside the SDK and the runtime libraries.</summary>
internal static class ProductFault
{
    private const int MaximumLength = 600;

    internal static string Summary(Exception exception)
    {
        string message = exception.Message;
        int end = message.IndexOfAny(['\r', '\n']);
        if (end >= 0) message = message[..end];
        string? frame = ProductFrame(exception.StackTrace);
        string line = frame is null
            ? $"{exception.GetType().Name}: {message}"
            : $"{exception.GetType().Name}: {message} (at {frame})";
        return line.Length <= MaximumLength ? line : string.Concat(line.AsSpan(0, MaximumLength - 3), "...");
    }

    private static string? ProductFrame(string? stackTrace)
    {
        if (stackTrace is null) return null;
        foreach (string entry in stackTrace.Split('\n'))
        {
            string frame = entry.Trim();
            if (!frame.StartsWith("at ", StringComparison.Ordinal)) continue;
            frame = frame[3..];
            if (frame.StartsWith("Rusty.Engine.", StringComparison.Ordinal)
                || frame.StartsWith("System.", StringComparison.Ordinal)
                || frame.StartsWith("Microsoft.", StringComparison.Ordinal)) continue;
            return frame;
        }
        return null;
    }
}

internal sealed class ProductDebugExecutionContext : DebugExecutionContext
{
    internal void RecordStarted() => RecordLifecycleState(ProductLifecycleState.Running, clearLatestUpdate: false);
    internal void RecordPaused() => RecordLifecycleState(ProductLifecycleState.Paused, clearLatestUpdate: false);
    internal void RecordResumed() => RecordLifecycleState(ProductLifecycleState.Running, clearLatestUpdate: false);
    internal void RecordRestarted() => RecordLifecycleState(ProductLifecycleState.Running, clearLatestUpdate: true);
    internal void RecordShutdown() => RecordLifecycleState(ProductLifecycleState.Shutdown, clearLatestUpdate: false);
    internal void RecordUpdated(ProductUpdateFacts facts) => RecordUpdate(facts);
    internal void ApplyCommittedRuntime(ProductRuntimeFacts facts) => RecordCommittedRuntime(facts);
}

/// <summary>
/// The Engine side of the product bind entrypoint. The SDK's product generator
/// emits the product's <c>rusty_product_bind_v1</c> export, which passes the
/// product constructor and debug catalog here. Products never call this directly.
/// </summary>
[EditorBrowsable(EditorBrowsableState.Never)]
public static unsafe class ProductBridge
{
    private readonly struct AbiIdentityStorage
    {
        internal AbiIdentityStorage(byte* pointer, nuint length) { Pointer = pointer; Length = length; }
        internal byte* Pointer { get; }
        internal nuint Length { get; }
    }

    private static readonly AbiIdentityStorage AbiIdentity = CreateAbiIdentity();
    private static readonly NativeProductApi* Api = CreateApi();
    // One product is bound per load: a CoreCLR load context or a NativeAOT module.
    private static Func<ProductCreateContext, IEngineProduct>? s_createProduct;
    private static Func<IEngineProduct, IDebugCommandCatalog>? s_createDebugCatalog;

    private static AbiIdentityStorage CreateAbiIdentity()
    {
        byte[] source = Encoding.UTF8.GetBytes(NativeProductAbiIdentity.SdkBuildIdentity);
        byte* copy = (byte*)NativeMemory.Alloc((nuint)source.Length);
        fixed (byte* input = source) Buffer.MemoryCopy(input, copy, source.Length, source.Length);
        return new AbiIdentityStorage(copy, (nuint)source.Length);
    }

    private static NativeProductApi* CreateApi()
    {
        NativeProductApi* api = (NativeProductApi*)NativeMemory.Alloc((nuint)sizeof(NativeProductApi));
        *api = new NativeProductApi
        {
            create = &Create,
            start = &Start,
            update = &Update,
            pause = &Pause,
            resume = &Resume,
            restart = &Restart,
            shutdown = &Shutdown,
            destroy = &Destroy,
            complete_timeline = &CompleteTimeline,
            paused_intents = &PausedIntents,
            execute_debug = &ExecuteDebug,
            describe_debug = &DescribeDebug,
            release_debug_result = &ReleaseDebugResult,
            observe_runtime = &ObserveRuntime,
            read_call_error = &ReadCallError,
            release_call_error = &ReleaseCallError,
        };
        return api;
    }

    /// <summary>Completes the host handshake for the product's generated bind export.</summary>
    /// <param name="host">The host's <c>NativeProductAbiHandshakeV1</c>.</param>
    /// <param name="product">The product handshake this call fills.</param>
    public static int Bind(
        nint host,
        nint product,
        Func<ProductCreateContext, IEngineProduct> createProduct,
        Func<IEngineProduct, IDebugCommandCatalog> createDebugCatalog)
    {
        if (host == 0 || product == 0 || createProduct is null || createDebugCatalog is null) return 2;
        s_createProduct = createProduct;
        s_createDebugCatalog = createDebugCatalog;
        *(NativeProductAbiHandshakeV1*)product = new NativeProductAbiHandshakeV1
        {
            protocol_version = NativeProductAbiIdentity.ProtocolVersion,
            engine_api_size = (nuint)sizeof(NativeEngineApi),
            product_api_size = (nuint)sizeof(NativeProductApi),
            fingerprint = NativeProductAbiIdentity.Fingerprint(),
            build_identity = new NativeUtf8Slice { bytes = AbiIdentity.Pointer, len = AbiIdentity.Length },
            product_api = Api,
        };
        return 1;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int Create(NativeProductCreateArgs* args, void** handle, NativeProductCallError* error)
    {
        ProductLifetime? lifetime = null;
        try
        {
            *error = default;
            if (args is null || handle is null || (args->content_len != 0 && args->content is null) || (args->input.context_len != 0 && args->input.context is null) || (args->input.direct_intents_len != 0 && args->input.direct_intents is null) || (args->input.physical_mappings_len != 0 && args->input.physical_mappings is null)) return 2;
            ProductInputConfiguration input = CopyInputConfiguration(args->input);
            ProductDebugExecutionContext debugging = new();
            EngineContext engine = new(args->engine);
            ProductContent content = new(CopyContent(args->content, args->content_len), engine.Content);
            Func<ProductCreateContext, IEngineProduct> createProduct = s_createProduct ?? throw new InvalidOperationException("The product was created before it was bound.");
            IEngineProduct product = createProduct(new ProductCreateContext(engine, content, input, debugging));
            lifetime = new ProductLifetime(product, s_createDebugCatalog!(product), debugging);
            *handle = (void*)GCHandle.ToIntPtr(GCHandle.Alloc(lifetime));
            return 1;
        }
        catch (Exception exception)
        {
            try { SetProductCallError(error, exception); }
            catch { }
            try { lifetime?.Dispose(); }
            catch { }
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int Start(void* handle) => Invoke(handle, static lifetime => { lifetime.Product.Start(); lifetime.Debugging.RecordStarted(); });

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int Update(void* handle, NativeProductUpdateArgs* args, NativeProductUpdateResult* result)
    {
        try
        {
            if (args is null || result is null || (args->event_count != 0 && args->events is null)) return 2;
            *result = NativeProductUpdateResult.NativeProductUpdateResult_None;
            ProductLifetime lifetime = Get(handle);
            ProductUpdateFacts facts = NativeConversions.FromNative(args->facts);
            ProductUpdateResult productResult = lifetime.Product.Update(new ProductUpdate(facts, CopyInput(args->events, args->event_count)));
            *result = productResult switch
            {
                ProductUpdateResult.None => NativeProductUpdateResult.NativeProductUpdateResult_None,
                ProductUpdateResult.ReportFault => NativeProductUpdateResult.NativeProductUpdateResult_ReportFault,
                _ => throw new ArgumentOutOfRangeException(nameof(productResult)),
            };
            lifetime.Debugging.RecordUpdated(facts);
            return 1;
        }
        catch (Exception exception)
        {
            RecordCallError(handle, exception);
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int Pause(void* handle) => Invoke(handle, static lifetime => { lifetime.Product.Pause(); lifetime.Debugging.RecordPaused(); });

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int Resume(void* handle) => Invoke(handle, static lifetime => { lifetime.Product.Resume(); lifetime.Debugging.RecordResumed(); });

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int Restart(void* handle) => Invoke(handle, static lifetime => { lifetime.Product.Restart(); lifetime.Debugging.RecordRestarted(); });

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int Shutdown(void* handle) => Invoke(handle, static lifetime => { lifetime.Product.Shutdown(); lifetime.Debugging.RecordShutdown(); });

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int CompleteTimeline(void* handle, NativeProductTimelineCompletion* completion, byte* accepted)
    {
        try
        {
            if (completion is null || accepted is null
                || (completion->correlation.len != 0 && completion->correlation.bytes is null)
                || (completion->outcome_data.len != 0 && completion->outcome_data.bytes is null)
                || (completion->provenance_correlation.len != 0 && completion->provenance_correlation.bytes is null)
                || (completion->provenance_detail.len != 0 && completion->provenance_detail.bytes is null)) return 2;
            *accepted = 0;
            ProductTimelineCompletion value = new(
                completion->ticket,
                new ProductTimelineBinding(completion->instance_id, completion->generation, completion->control_revision),
                CopyBytes(completion->correlation.bytes, completion->correlation.len),
                NativeConversions.FromNative(completion->outcome),
                CopyOptionalBytes(completion->outcome_data.bytes, completion->outcome_data.len),
                CopyBytes(completion->provenance_correlation.bytes, completion->provenance_correlation.len),
                CopyOptionalBytes(completion->provenance_detail.bytes, completion->provenance_detail.len));
            *accepted = Get(handle).Product.CompleteTimeline(value) ? (byte)1 : (byte)0;
            return 1;
        }
        catch (Exception exception)
        {
            RecordCallError(handle, exception);
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int PausedIntents(void* handle, NativeInputEvent* events, nuint count)
    {
        try
        {
            if (count != 0 && events is null) return 2;
            Get(handle).Product.HandlePausedIntents(CopyInput(events, count));
            return 1;
        }
        catch (Exception exception)
        {
            RecordCallError(handle, exception);
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void ObserveRuntime(void* handle, NativeProductRuntimeFacts* facts)
    {
        try
        {
            if (handle is null || facts is null) return;
            Get(handle).Debugging.ApplyCommittedRuntime(NativeConversions.FromNative(*facts));
        }
        catch { }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int ExecuteDebug(void* handle, NativeUtf8Slice* command, NativeProductDebugResult* result)
    {
        try
        {
            if (command is null || result is null || (command->len != 0 && command->bytes is null)) return 2;
            *result = default;
            string commandLine = StrictUtf8.GetString(new ReadOnlySpan<byte>(command->bytes, checked((int)command->len)));
            DebugCommandResult commandResult = Get(handle).DebugCatalog.Execute(commandLine);
            return SetDebugResult(result, commandResult.Succeeded, commandResult.Message ?? string.Empty);
        }
        catch { return 99; }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int DescribeDebug(void* handle, NativeProductDebugResult* result)
    {
        try
        {
            if (result is null) return 2;
            *result = default;
            return SetDebugResult(result, true, EncodeDebugCatalog(Get(handle).DebugCatalog.Commands));
        }
        catch { return 99; }
    }

    private static int SetDebugResult(NativeProductDebugResult* result, bool succeeded, string message)
    {
        byte[] bytes = StrictUtf8.GetBytes(message);
        byte* owned = bytes.Length == 0 ? null : (byte*)NativeMemory.Alloc((nuint)bytes.Length);
        if (bytes.Length != 0 && owned is null) return 99;
        if (bytes.Length != 0) bytes.CopyTo(new Span<byte>(owned, bytes.Length));
        result->succeeded = succeeded ? (byte)1 : (byte)0;
        result->message = new NativeUtf8Slice { bytes = owned, len = (nuint)bytes.Length };
        return 1;
    }

    // Descriptor data is generated product catalog output only. The
    // host treats this JSON as read-only help/completion data and
    // never uses it for invocation or method discovery.
    private static string EncodeDebugCatalog(IReadOnlyList<DebugCommandDescriptor> commands)
    {
        StringBuilder output = new();
        output.Append("{\"available\":true,\"commands\":[");
        for (int index = 0; index < commands.Count; index++)
        {
            if (index != 0) output.Append(',');
            DebugCommandDescriptor command = commands[index];
            output.Append("{\"name\":");
            AppendJsonString(output, command.Name);
            output.Append(",\"description\":");
            AppendJsonString(output, command.Description);
            output.Append(",\"parameters\":[");
            for (int parameterIndex = 0; parameterIndex < command.Parameters.Count; parameterIndex++)
            {
                if (parameterIndex != 0) output.Append(',');
                DebugCommandParameterDescriptor parameter = command.Parameters[parameterIndex];
                output.Append("{\"name\":");
                AppendJsonString(output, parameter.Name);
                output.Append(",\"type\":");
                AppendJsonString(output, parameter.TypeName);
                output.Append('}');
            }
            output.Append("]}");
        }
        return output.Append("]}").ToString();
    }

    private static void AppendJsonString(StringBuilder output, string value)
    {
        output.Append('"');
        foreach (char character in value)
        {
            switch (character)
            {
                case '"': output.Append("\\\""); break;
                case '\\': output.Append("\\\\"); break;
                case '\b': output.Append("\\b"); break;
                case '\f': output.Append("\\f"); break;
                case '\n': output.Append("\\n"); break;
                case '\r': output.Append("\\r"); break;
                case '\t': output.Append("\\t"); break;
                default:
                    if (character < ' ') output.Append("\\u").Append(((int)character).ToString("x4", System.Globalization.CultureInfo.InvariantCulture));
                    else output.Append(character);
                    break;
            }
        }
        output.Append('"');
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void ReleaseDebugResult(void* handle, NativeProductDebugResult result)
    {
        try
        {
            if (handle is null) return;
            NativeMemory.Free(result.message.bytes);
        }
        catch { }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int ReadCallError(void* handle, NativeProductCallError* result)
    {
        try
        {
            if (result is null) return 2;
            *result = default;
            Exception? error = Get(handle).TakeCallError();
            return error is null ? 1 : SetProductCallError(result, error);
        }
        catch { return 99; }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void ReleaseCallError(void* _, NativeProductCallError result)
    {
        try
        {
            NativeMemory.Free(result.service.bytes);
            NativeMemory.Free(result.operation.bytes);
            NativeMemory.Free(result.message.bytes);
        }
        catch { }
    }

    private static int SetProductCallError(NativeProductCallError* result, Exception exception)
    {
        if (result is null) return 2;
        *result = default;
        string service = "CSharpProduct";
        string operation = "Callback";
        int status = 99;
        // The first line is the bounded summary the host prints; the rest keeps
        // the complete exception text, including its stack trace.
        string message = ProductFault.Summary(exception) + Environment.NewLine + exception;
        if (exception is EngineCallException engineError)
        {
            service = engineError.Service;
            operation = engineError.Operation;
            status = engineError.Status;
            message = ProductFault.Summary(exception) + Environment.NewLine + DescribeEngineError(engineError) + Environment.NewLine + engineError.StackTrace;
        }
        byte* serviceBytes = null;
        byte* operationBytes = null;
        byte* messageBytes = null;
        try
        {
            serviceBytes = AllocateProductErrorText(service, out nuint serviceLength);
            operationBytes = AllocateProductErrorText(operation, out nuint operationLength);
            messageBytes = AllocateProductErrorText(message, out nuint messageLength);
            *result = new NativeProductCallError
            {
                service = new NativeUtf8Slice { bytes = serviceBytes, len = serviceLength },
                operation = new NativeUtf8Slice { bytes = operationBytes, len = operationLength },
                status = status,
                message = new NativeUtf8Slice { bytes = messageBytes, len = messageLength },
            };
            return 1;
        }
        catch
        {
            NativeMemory.Free(serviceBytes);
            NativeMemory.Free(operationBytes);
            NativeMemory.Free(messageBytes);
            *result = default;
            return 99;
        }
    }

    private static string DescribeEngineError(EngineCallException error)
    {
        if (error.Diagnostics.IsEmpty) return error.Message;
        StringBuilder message = new(error.Message);
        foreach (EngineDiagnostic diagnostic in error.Diagnostics.Span)
        {
            message.Append(": ").Append(diagnostic.Code).Append(": ").Append(diagnostic.Message);
            if (!string.IsNullOrEmpty(diagnostic.Source)) message.Append(" [").Append(diagnostic.Source).Append(']');
        }
        return message.ToString();
    }

    private static byte* AllocateProductErrorText(string value, out nuint length)
    {
        // Replacing, not strict: a lone surrogate must not lose the product's error.
        byte[] bytes = Encoding.UTF8.GetBytes(value);
        length = (nuint)bytes.Length;
        if (bytes.Length == 0) return null;
        byte* allocated = (byte*)NativeMemory.Alloc((nuint)bytes.Length);
        if (allocated is null) throw new OutOfMemoryException();
        bytes.CopyTo(new Span<byte>(allocated, bytes.Length));
        return allocated;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void Destroy(void* handle)
    {
        if (handle is null) return;
        GCHandle reference = GCHandle.FromIntPtr((nint)handle);
        try
        {
            if (reference.Target is ProductLifetime lifetime) lifetime.Dispose();
        }
        catch { }
        finally
        {
            try { reference.Free(); }
            catch { }
        }
    }

    private static int Invoke(void* handle, Action<ProductLifetime> action)
    {
        try
        {
            ProductLifetime lifetime = Get(handle);
            action(lifetime);
            lifetime.ClearCallError();
            return 1;
        }
        catch (Exception exception)
        {
            RecordCallError(handle, exception);
            return 99;
        }
    }

    private static void RecordCallError(void* handle, Exception exception)
    {
        try
        {
            if (handle is not null) Get(handle).RecordCallError(exception);
        }
        catch { }
    }

    private static ProductLifetime Get(void* handle)
    {
        if (handle is null) throw new ArgumentNullException(nameof(handle));
        return (ProductLifetime)GCHandle.FromIntPtr((nint)handle).Target!;
    }

    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    private static ProductContentFile[] CopyContent(NativeContentFile* source, nuint count)
    {
        ProductContentFile[] files = new ProductContentFile[checked((int)count)];
        for (nuint index = 0; index < count; index++)
        {
            NativeContentFile file = source[index];
            files[checked((int)index)] = new ProductContentFile(CopyBytes(file.path, file.path_len), CopyBytes(file.bytes, file.bytes_len));
        }
        return files;
    }

    private static ProductInputEvent[] CopyInput(NativeInputEvent* source, nuint count)
    {
        ProductInputEvent[] events = new ProductInputEvent[checked((int)count)];
        for (nuint index = 0; index < count; index++)
        {
            NativeInputEvent input = source[index];
            events[checked((int)index)] = new ProductInputEvent(
                NativeConversions.FromNative(input.kind),
                NativeConversions.FromNative(input.edge),
                NativeConversions.FromNative(input.device),
                NativeConversions.FromNative(input.channel),
                NativeConversions.FromNative(input.axis),
                NativeConversions.FromNative(input.keyboard),
                NativeConversions.FromNative(input.pointer_button),
                NativeConversions.FromNative(input.controller_button),
                NativeConversions.FromNative(input.controller_axis),
                NativeConversions.FromNative(input.clear_reason),
                NativeConversions.FromNative(input.value_kind),
                NativeConversions.FromNative(input.phase),
                NativeConversions.FromNative(input.provenance),
                new InputBinding(input.binding.instance_id, input.binding.generation, input.binding.control_revision),
                new InputSequence(input.sequence.value),
                new InputContext(CopyBytes(input.context, input.context_len)),
                input.x,
                input.y,
                CopyBytes(input.label, input.label_len),
                CopyBytes(input.mapping_id, input.mapping_id_len),
                CopyBytes(input.intent, input.intent_len),
                CopyBytes(input.payload_contract, input.payload_contract_len),
                CopyBytes(input.payload_data, input.payload_data_len));
        }
        return events;
    }

    private static ProductInputConfiguration CopyInputConfiguration(NativeInputConfiguration input)
    {
        ProductInputDescriptor[] descriptors = new ProductInputDescriptor[checked((int)input.direct_intents_len)];
        for (nuint index = 0; index < input.direct_intents_len; index++)
        {
            NativeInputDescriptor descriptor = input.direct_intents[index];
            descriptors[checked((int)index)] = new ProductInputDescriptor(
                CopyBytes(descriptor.id, descriptor.id_len),
                NativeConversions.FromNative(descriptor.value_kind),
                CopyBytes(descriptor.payload_contract, descriptor.payload_contract_len));
        }
        ProductInputMapping[] mappings = new ProductInputMapping[checked((int)input.physical_mappings_len)];
        for (nuint index = 0; index < input.physical_mappings_len; index++)
        {
            NativeInputMapping mapping = input.physical_mappings[index];
            mappings[checked((int)index)] = new ProductInputMapping(
                CopyBytes(mapping.id, mapping.id_len),
                CopyBytes(mapping.intent, mapping.intent_len),
                NativeConversions.FromNative(mapping.trigger_kind),
                NativeConversions.FromNative(mapping.edge),
                NativeConversions.FromNative(mapping.axis),
                NativeConversions.FromNative(mapping.keyboard),
                NativeConversions.FromNative(mapping.pointer_button),
                NativeConversions.FromNative(mapping.controller_button),
                NativeConversions.FromNative(mapping.controller_axis),
                CopyKeyboardControls(mapping.chord, mapping.chord_len),
                new InputContext(CopyBytes(mapping.context, mapping.context_len)));
        }
        return new ProductInputConfiguration(
            new InputBinding(input.binding.instance_id, input.binding.generation, input.binding.control_revision),
            new InputContext(CopyBytes(input.context, input.context_len)),
            descriptors,
            mappings,
            NativeConversions.FromNative(input.cursor_mode));
    }

    private static ReadOnlyMemory<KeyboardControl> CopyKeyboardControls(NativeKeyboardControl* source, nuint count)
    {
        if (count != 0 && source is null) throw new ArgumentException("nonempty native keyboard chord has no storage");
        KeyboardControl[] controls = new KeyboardControl[checked((int)count)];
        for (nuint index = 0; index < count; index++) controls[checked((int)index)] = NativeConversions.FromNative(source[index]);
        return controls;
    }

    private static ReadOnlyMemory<byte> CopyBytes(byte* source, nuint length)
    {
        if (length == 0) return ReadOnlyMemory<byte>.Empty;
        if (source is null) throw new ArgumentException("nonempty native byte slice has no storage");
        return new ReadOnlySpan<byte>(source, checked((int)length)).ToArray();
    }

    private static ReadOnlyMemory<byte>? CopyOptionalBytes(byte* source, nuint length)
    {
        if (source is null && length == 0) return null;
        return CopyBytes(source, length);
    }
}
