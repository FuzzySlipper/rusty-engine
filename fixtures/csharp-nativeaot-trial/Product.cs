using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Rusty.Engine.Native;

// This fixture implements one trusted product. All ABI layouts and the direct
// Engine function table are generated from Rust; this file owns only product
// lifecycle, state, input meaning, and calls to the generated Engine API.
public static unsafe class Product
{
    private sealed class State
    {
        public int Turns;
        public float X;
        public bool Started;
        public bool Paused;
        public bool Shutdown;
        public ulong UiSequence;
        public NativeUiStreamHandle UiStream;
        public NativeSpatialSessionHandle Spatial;
        public NativeAppearanceHandle Appearance;
        public NativeRngHandle Rng;
        public NativeRngHandle ForkedRng;
        public ulong LastRandom;
        public NativeLookState Look;
        public NativeEngineApi Engine;
    }

    [UnmanagedCallersOnly(EntryPoint = "rusty_product_bind", CallConvs = [typeof(CallConvCdecl)])]
    public static int Bind(NativeProductApi* api)
    {
        if (api is null)
        {
            return 2;
        }
        api->create = &Create;
        api->start = &Start;
        api->turn = &Turn;
        api->pause = &Pause;
        api->resume = &Resume;
        api->shutdown = &Shutdown;
        api->destroy = &Destroy;
        return 1;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    public static int Create(NativeProductCreateArgs* args, void** handle)
    {
        try
        {
            if (args is null || handle is null || (args->content_len != 0 && args->content is null))
            {
                return 2;
            }
            for (nuint index = 0; index < args->content_len; index++)
            {
                NativeContentFile file = args->content[index];
                if ((file.path_len != 0 && file.path is null) ||
                    (file.bytes_len != 0 && file.bytes is null))
                {
                    return 3;
                }
            }

            var state = new State { Engine = args->engine };
            state.Appearance = new EngineApi(state.Engine).Appearance.CreatePrimitive(
                new NativePrimitiveAppearanceRequest
                {
                    geometry = 1,
                    color = new NativeColor { r = 0.25f, g = 0.75f, b = 1.0f, a = 1.0f },
                });
            var rng = new EngineApi(state.Engine).Rng;
            long keyed = rng.DrawKeyed(17, -10, 10, "nativeaot-trial", "create").value;
            if (keyed != rng.DrawKeyed(17, -10, 10, "nativeaot-trial", "create").value)
            {
                return 4;
            }
            state.Rng = rng.CreateScoped(17, "nativeaot-trial");
            state.ForkedRng = rng.ForkScoped(state.Rng, "child");
            state.LastRandom = rng.NextU64(state.ForkedRng).value;
            state.UiStream = new EngineApi(state.Engine).Ui.OpenStream(
                "nativeaot-trial",
                "nativeaot.trial.hud");
            var spatial = new EngineApi(state.Engine).Spatial;
            state.Spatial = spatial.CreateSession(new NativeSpatialSessionConfig
            {
                collision_voxel_size = 1.0,
                collision_chunk_size = 16,
            });
            spatial.ReplaceNavigation(
                state.Spatial,
                new NativePlanarNavConfig { grid_id = 1, cell_size = 1.0, chunk_size = 16 },
                [
                    new NativePlanarNavCell { x = 0, y = 0, z = 0 },
                    new NativePlanarNavCell { x = 1, y = 0, z = 0 },
                ]);
            _ = spatial.ProposeNavigationStep(new NativeNavigationStepRequest
            {
                session = state.Spatial,
                from = new NativeVec3 { x = 0.5f, y = 0.5f, z = 0.5f },
                target = new NativeVec3 { x = 1.5f, y = 0.5f, z = 0.5f },
                max_step_units = 0.5f,
                max_visited = 16,
            });
            *handle = (void*)GCHandle.ToIntPtr(GCHandle.Alloc(state));
            return 1;
        }
        catch
        {
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    public static int Start(void* handle)
    {
        try
        {
            State state = Get(handle);
            state.Started = true;
            state.Paused = false;
            return PublishPresentation(state);
        }
        catch
        {
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    public static int Turn(void* handle, NativeTurnArgs* args)
    {
        try
        {
            State state = Get(handle);
            if (!state.Started || state.Paused || state.Shutdown)
            {
                return 4;
            }
            if (args is null || args->kind is < 1 or > 3)
            {
                return 5;
            }
            if (args->event_count != 0 && args->events is null)
            {
                return 6;
            }

            for (nuint index = 0; index < args->event_count; index++)
            {
                NativeInputEvent input = args->events[index];
                if (input.label_len != 0 && input.label is null)
                {
                    return 7;
                }
                if (input.kind == 1 && input.edge == 1 && IsKeyW(input))
                {
                    state.X += 1.0f;
                }
                if (input.kind == 3)
                {
                    state.Look = new EngineApi(state.Engine).Look.Integrate(new NativeLookRequest
                    {
                        state = state.Look,
                        delta = new NativeVec2 { x = input.x, y = input.y },
                        config = LookConfig(),
                    }).state;
                }
            }
            state.Turns++;
            state.LastRandom = new EngineApi(state.Engine).Rng.NextBoundedU32(
                new NativeScopedRngBoundedRequest { stream = state.Rng, upper = 100 }).value;
            return PublishPresentation(state);
        }
        catch
        {
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    public static int Pause(void* handle)
    {
        try
        {
            Get(handle).Paused = true;
            return 1;
        }
        catch
        {
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    public static int Resume(void* handle)
    {
        try
        {
            Get(handle).Paused = false;
            return 1;
        }
        catch
        {
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    public static int Shutdown(void* handle)
    {
        try
        {
            Get(handle).Shutdown = true;
            return 1;
        }
        catch
        {
            return 99;
        }
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    public static void Destroy(void* handle)
    {
        try
        {
            if (handle is not null)
            {
                State state = Get(handle);
                var rng = new EngineApi(state.Engine).Rng;
                _ = rng.NextBool(state.Rng);
                rng.DestroyScoped(state.ForkedRng);
                rng.DestroyScoped(state.Rng);
                new EngineApi(state.Engine).Spatial.DestroySession(state.Spatial);
                GCHandle.FromIntPtr((nint)handle).Free();
            }
        }
        catch
        {
        }
    }

    private static State Get(void* handle)
    {
        if (handle is null)
        {
            throw new ArgumentNullException(nameof(handle));
        }
        return (State)GCHandle.FromIntPtr((nint)handle).Target!;
    }

    private static bool IsKeyW(NativeInputEvent input)
    {
        return input.label_len == 4 &&
            input.label[0] == (byte)'K' &&
            input.label[1] == (byte)'e' &&
            input.label[2] == (byte)'y' &&
            input.label[3] == (byte)'W';
    }

    private static NativeLookConfig LookConfig() => new()
    {
        horizontal_radians_per_unit = 0.01f,
        vertical_radians_per_unit = 0.01f,
        minimum_pitch_radians = -1.4f,
        maximum_pitch_radians = 1.4f,
        maximum_delta_radians = 1.0f,
        wrap_yaw = 1,
    };

    private static int PublishPresentation(State state)
    {
        try
        {
            var engine = new EngineApi(state.Engine);
            PublishAppearanceSnapshot(engine, state);
            engine.Ui.PublishProjection(state.UiStream, ++state.UiSequence, UiValue(state));
            return 1;
        }
        catch
        {
            return 8;
        }
    }

    private static StructuredValueArena UiValue(State state)
    {
        var values = new StructuredValueBuilder();
        uint turns = values.Number(state.Turns);
        uint yaw = values.Number(state.Look.yaw_radians);
        uint x = values.Number(state.X);
        return values.Build(values.Object(("turns", turns), ("yaw", yaw), ("x", x)));
    }

    private static void PublishAppearanceSnapshot(EngineApi engine, State state)
    {
        if (state.Turns >= 2)
        {
            engine.Appearance.PublishSnapshot(ReadOnlySpan<NativeAppearanceFact>.Empty);
            return;
        }

        NativeAppearanceFact fact = new()
        {
            object_id = 41,
            transform = new NativeTransform
            {
                translation = new NativeVec3 { x = state.X },
                rotation = new NativeQuat { w = 1.0f },
                scale = new NativeVec3 { x = 1.0f, y = 1.0f, z = 1.0f },
            },
            appearance = state.Appearance,
            visible = 1,
        };
        engine.Appearance.PublishSnapshot(new ReadOnlySpan<NativeAppearanceFact>(&fact, 1));
    }
}
