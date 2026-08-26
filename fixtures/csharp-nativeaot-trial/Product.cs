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
            return PublishVisualSnapshot(state);
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
            }
            state.Turns++;
            return PublishVisualSnapshot(state);
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

    private static int PublishVisualSnapshot(State state)
    {
        var engine = new EngineApi(state.Engine);
        if (state.Turns >= 2)
        {
            return engine.PublishVisualSnapshot(ReadOnlySpan<NativeVisualFact>.Empty) == 1 ? 1 : 8;
        }

        ReadOnlySpan<byte> appearance = "appearance/nativeaot-trial"u8;
        fixed (byte* appearancePointer = appearance)
        {
            NativeVisualFact fact = new()
            {
                object_id = 41,
                appearance = appearancePointer,
                appearance_len = (nuint)appearance.Length,
                visible = 1,
            };
            fact.translation[0] = state.X;
            fact.rotation[3] = 1.0f;
            fact.scale[0] = 1.0f;
            fact.scale[1] = 1.0f;
            fact.scale[2] = 1.0f;
            return engine.PublishVisualSnapshot(new ReadOnlySpan<NativeVisualFact>(&fact, 1)) == 1 ? 1 : 8;
        }
    }
}
