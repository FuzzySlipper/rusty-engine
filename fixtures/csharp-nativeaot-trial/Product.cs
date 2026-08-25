using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using System.Collections.Generic;

// Fixture ABI for task 7283. It deliberately models one trusted product, not
// a versioned extension point. Product state lives in this C# allocation.
public static unsafe class Product
{
    private static readonly object OutputLock = new();
    private static readonly HashSet<nint> ActiveOutputPointers = new();
    private static int FreedOutputs;
    private static int DuplicateFrees;
    [StructLayout(LayoutKind.Sequential)]
    public struct ContentFile
    {
        public byte* Path;
        public nuint PathLen;
        public byte* Bytes;
        public nuint BytesLen;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct CreateArgs
    {
        public ContentFile* Content;
        public nuint ContentLen;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct InputEvent
    {
        public uint Kind;
        public uint Edge;
        public ulong Sequence;
        public float X;
        public float Y;
        public byte* Label;
        public nuint LabelLen;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct TurnArgs
    {
        public uint Kind;
        public uint Reserved;
        public ulong ObservedTimeOrStep;
        public InputEvent* Events;
        public nuint EventCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct OutputBuffer
    {
        public byte* Data;
        public nuint Len;
    }

    private sealed class State
    {
        public int ContentFiles;
        public int Turns;
        public int InputEvents;
        public bool Started;
        public bool Paused;
        public bool Shutdown;
    }

    [UnmanagedCallersOnly(EntryPoint = "rusty_product_create", CallConvs = [typeof(CallConvCdecl)])]
    public static int Create(CreateArgs* args, void** handle, OutputBuffer* output)
    {
        try
        {
            if (args is null || handle is null || (args->ContentLen != 0 && args->Content is null)) return 2;
            for (nuint index = 0; index < args->ContentLen; index++)
            {
                ContentFile file = args->Content[index];
                if ((file.PathLen != 0 && file.Path is null) || (file.BytesLen != 0 && file.Bytes is null)) return 3;
            }
            var state = new State { ContentFiles = checked((int)args->ContentLen) };
            *handle = (void*)GCHandle.ToIntPtr(GCHandle.Alloc(state));
            return WriteOutput(state, output);
        }
        catch { return 99; }
    }

    [UnmanagedCallersOnly(EntryPoint = "rusty_product_start", CallConvs = [typeof(CallConvCdecl)])]
    public static int Start(void* handle, OutputBuffer* output)
    {
        try { var state = Get(handle); state.Started = true; state.Paused = false; return WriteOutput(state, output); } catch { return 99; }
    }

    [UnmanagedCallersOnly(EntryPoint = "rusty_product_turn", CallConvs = [typeof(CallConvCdecl)])]
    public static int Turn(void* handle, TurnArgs* args, OutputBuffer* output)
    {
        try
        {
            var state = Get(handle);
            if (!state.Started || state.Paused || state.Shutdown) return 4;
            if (args is null || args->Kind is < 1 or > 3) return 5;
            if (args->EventCount != 0 && args->Events is null) return 6;
            for (nuint index = 0; index < args->EventCount; index++)
            {
                if (args->Events[index].LabelLen != 0 && args->Events[index].Label is null) return 7;
            }
            state.InputEvents = checked((int)args->EventCount);
            state.Turns++;
            return WriteOutput(state, output);
        }
        catch { return 99; }
    }

    [UnmanagedCallersOnly(EntryPoint = "rusty_product_pause", CallConvs = [typeof(CallConvCdecl)])]
    public static int Pause(void* handle, OutputBuffer* output)
    {
        try { var state = Get(handle); state.Paused = true; return WriteOutput(state, output); } catch { return 99; }
    }

    [UnmanagedCallersOnly(EntryPoint = "rusty_product_resume", CallConvs = [typeof(CallConvCdecl)])]
    public static int Resume(void* handle, OutputBuffer* output)
    {
        try { var state = Get(handle); state.Paused = false; return WriteOutput(state, output); } catch { return 99; }
    }

    [UnmanagedCallersOnly(EntryPoint = "rusty_product_shutdown", CallConvs = [typeof(CallConvCdecl)])]
    public static int Shutdown(void* handle, OutputBuffer* output)
    {
        try { var state = Get(handle); state.Shutdown = true; return WriteOutput(state, output); } catch { return 99; }
    }

    [UnmanagedCallersOnly(EntryPoint = "rusty_product_destroy", CallConvs = [typeof(CallConvCdecl)])]
    public static void Destroy(void* handle)
    {
        try { if (handle is not null) GCHandle.FromIntPtr((nint)handle).Free(); } catch { }
    }

    [UnmanagedCallersOnly(EntryPoint = "rusty_product_free_output", CallConvs = [typeof(CallConvCdecl)])]
    public static void FreeOutput(OutputBuffer output)
    {
        try
        {
            if (output.Data is null) return;
            lock (OutputLock)
            {
                if (!ActiveOutputPointers.Remove((nint)output.Data)) { DuplicateFrees++; return; }
                FreedOutputs++;
            }
            Marshal.FreeCoTaskMem((nint)output.Data);
        }
        catch { }
    }

    private static State Get(void* handle)
    {
        if (handle is null) throw new ArgumentNullException(nameof(handle));
        return (State)GCHandle.FromIntPtr((nint)handle).Target!;
    }

    private static int WriteOutput(State state, OutputBuffer* output)
    {
        if (output is null) return 1;
        string ui = "{\"artifact\":\"rusty.product.ui-projection\",\"runtime\":{\"instanceId\":\"1\",\"generation\":\"1\",\"controlRevision\":\"1\"},\"sequence\":\"" + state.Turns + "\",\"stream\":\"csharp.trial.hud\",\"contract\":\"csharp.trial.ui.v1\",\"value\":{\"turns\":" + state.Turns + ",\"contentFiles\":" + state.ContentFiles + ",\"paused\":" + (state.Paused ? "true" : "false") + "}}";
        const string frame = "{\"schemaVersion\":1,\"ops\":[{\"op\":\"create\",\"handle\":1,\"parent\":null,\"node\":{\"geometry\":{\"kind\":\"cube\"},\"material\":{\"color\":[1,1,1,1],\"wireframe\":false},\"transform\":{\"translation\":[1,0,0],\"rotation\":[0,0,0,1],\"scale\":[1,1,1]},\"visible\":true,\"layer\":\"scene\",\"metadata\":{\"sourceEntity\":1,\"sourceSceneNode\":null,\"tags\":[\"csharp-trial\"],\"label\":\"csharp trial\"}}}]}";
        int frees;
        int duplicates;
        lock (OutputLock) { frees = FreedOutputs; duplicates = DuplicateFrees; }
        byte[] bytes = Encoding.UTF8.GetBytes("{\"turns\":" + state.Turns + ",\"frees\":" + frees + ",\"duplicateFrees\":" + duplicates + ",\"inputEvents\":" + state.InputEvents + ",\"ui\":" + ui + ",\"frame\":" + frame + "}");
        output->Data = (byte*)Marshal.AllocCoTaskMem(bytes.Length);
        output->Len = (nuint)bytes.Length;
        lock (OutputLock) { ActiveOutputPointers.Add((nint)output->Data); }
        bytes.CopyTo(new Span<byte>(output->Data, bytes.Length));
        return 1;
    }
}
