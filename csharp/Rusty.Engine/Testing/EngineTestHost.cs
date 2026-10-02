using System.Runtime.InteropServices;
using System.Text;
using Rusty.Engine.NativeProduct;

namespace Rusty.Engine.Testing;

/// <summary>What an <see cref="EngineTestHost"/> starts with.</summary>
public sealed class EngineTestHostOptions
{
    /// <summary>
    /// The directory Persistence stores open their scopes under. Without one,
    /// <c>OpenStore</c> refuses, as in a host started without a persistence root.
    /// </summary>
    public string? PersistenceRoot { get; init; }

    /// <summary>Product content files by product-relative path, as a staged product's content folder supplies them.</summary>
    public IReadOnlyDictionary<string, ReadOnlyMemory<byte>> Content { get; init; } =
        new Dictionary<string, ReadOnlyMemory<byte>>();

    /// <summary>
    /// The test host library. By default a test project or tool uses the one its build recorded
    /// (<c>RustyEngineTestHostLibrary</c>): the pinned pair's runtime pack. A relative path
    /// is resolved against the application's base directory.
    /// </summary>
    public string? LibraryPath { get; init; }
}

/// <summary>
/// The Engine's real services, in process and headless, for a product's unit tests and
/// tool executables (<c>RustyEngineToolHost</c>). They
/// apply the same ownership, admission and handle rules as a running product and refuse
/// with the same <see cref="EngineCallException"/> codes. Nothing is rendered, shown or
/// played. Make Engine calls inside <see cref="Call"/>, which stands for one product
/// callback; renderer work a call produces is dropped. Not thread-safe.
/// </summary>
public sealed unsafe class EngineTestHost : IDisposable
{
    private const string LibraryConfigurationKey = "Rusty.Engine.TestHostLibrary";
    private const string CreateExport = "rusty_engine_test_host_create";
    private static readonly Dictionary<string, nint> s_libraries = new(StringComparer.Ordinal);

    private readonly NativeEngineTestHostApi _native;
    private bool _inCall;
    private bool _disposed;

    private EngineTestHost(NativeEngineTestHostApi native)
    {
        _native = native;
        Engine = new EngineContext(native.engine);
    }

    /// <summary>The Engine services. Use them inside <see cref="Call"/>.</summary>
    public IEngineContext Engine { get; }

    public static EngineTestHost Create(EngineTestHostOptions? options = null)
    {
        options ??= new EngineTestHostOptions();
        var create = (delegate* unmanaged[Cdecl]<NativeEngineTestHostRequest*, NativeEngineTestHostApi*, NativeOperationErrorReceipt*, int>)
            NativeLibrary.GetExport(Load(options.LibraryPath), CreateExport);
        string[] paths = options.Content.Keys.ToArray();
        byte[][] pathBytes = paths.Select(Encoding.UTF8.GetBytes).ToArray();
        var pins = new List<GCHandle>(paths.Length * 2);
        try
        {
            var files = new NativeContentFile[paths.Length];
            for (int index = 0; index < paths.Length; index++)
            {
                byte[] bytes = options.Content[paths[index]].ToArray();
                GCHandle path = GCHandle.Alloc(pathBytes[index], GCHandleType.Pinned);
                pins.Add(path);
                GCHandle content = GCHandle.Alloc(bytes, GCHandleType.Pinned);
                pins.Add(content);
                files[index] = new NativeContentFile
                {
                    path = (byte*)path.AddrOfPinnedObject(),
                    path_len = (nuint)pathBytes[index].Length,
                    bytes = (byte*)content.AddrOfPinnedObject(),
                    bytes_len = (nuint)bytes.Length,
                };
            }
            byte[] root = Encoding.UTF8.GetBytes(options.PersistenceRoot ?? string.Empty);
            fixed (byte* rootPointer = root)
            fixed (NativeContentFile* filesPointer = files)
            {
                NativeEngineTestHostRequest request = new()
                {
                    fingerprint = NativeProductAbiIdentity.Fingerprint(),
                    persistence_root = new NativeUtf8Slice { bytes = root.Length == 0 ? null : rootPointer, len = (nuint)root.Length },
                    content = files.Length == 0 ? null : filesPointer,
                    content_len = (nuint)files.Length,
                };
                NativeEngineTestHostApi native = default;
                NativeOperationErrorReceipt error = default;
                int status = create(&request, &native, &error);
                NativeCall.Require("TestHost", "Create", status, error);
                return new EngineTestHost(native);
            }
        }
        finally
        {
            foreach (GCHandle pin in pins) pin.Free();
        }
    }

    /// <summary>Runs <paramref name="call"/> as one product callback.</summary>
    public void Call(Action<IEngineContext> call)
    {
        ArgumentNullException.ThrowIfNull(call);
        Call(engine => { call(engine); return true; });
    }

    /// <summary>
    /// Runs <paramref name="call"/> as one product callback and returns its result. An Engine
    /// failure while finishing the call throws <see cref="EngineCallException"/> unless the
    /// call itself threw; either way the services keep what the call did.
    /// </summary>
    public T Call<T>(Func<IEngineContext, T> call)
    {
        ArgumentNullException.ThrowIfNull(call);
        ObjectDisposedException.ThrowIf(_disposed, this);
        if (_inCall) throw new InvalidOperationException("An Engine test host call is already running.");
        NativeCall.Require("TestHost", "BeginCall", _native.begin_call.Pointer(_native.context));
        _inCall = true;
        T result;
        try
        {
            result = call(Engine);
        }
        catch
        {
            Finish(throwOnRefusal: false);
            throw;
        }
        Finish(throwOnRefusal: true);
        return result;
    }

    private void Finish(bool throwOnRefusal)
    {
        _inCall = false;
        NativeOperationErrorReceipt error = default;
        int status = _native.finish_call.Pointer(_native.context, &error);
        if (throwOnRefusal) NativeCall.Require("TestHost", "FinishCall", status, error);
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        _native.destroy.Pointer(_native.context);
    }

    private static nint Load(string? path)
    {
        path ??= AppContext.GetData(LibraryConfigurationKey) as string;
        if (string.IsNullOrEmpty(path))
            throw new InvalidOperationException(
                "No Rusty Engine test host library is configured. A test project (IsTestProject) or tool (RustyEngineToolHost) that uses the Rusty.Engine package records the pinned pair's; run `rusty install`, or set RustyEngineTestHostLibrary or EngineTestHostOptions.LibraryPath.");
        // A tool records the library's file name; the build copied it beside the tool.
        path = Path.GetFullPath(path, AppContext.BaseDirectory);
        lock (s_libraries)
        {
            // Loaded once and kept for the process, like a product module.
            if (!s_libraries.TryGetValue(path, out nint library))
            {
                if (!File.Exists(path))
                    throw new FileNotFoundException($"The Rusty Engine test host library `{path}` does not exist; run `rusty install` for the pinned pair.", path);
                library = NativeLibrary.Load(path);
                s_libraries.Add(path, library);
            }
            return library;
        }
    }
}
