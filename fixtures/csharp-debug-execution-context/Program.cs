using System;
using System.Runtime.CompilerServices;
using Rusty.Engine;
using Rusty.Engine.Debugging;
using Rusty.Engine.NativeProduct;

[assembly: EngineProduct(typeof(DebugExecutionContextFixture.FixtureProduct))]

namespace DebugExecutionContextFixture;

internal static class Program
{
    private static unsafe int Main()
    {
        NativeProductApi api = default;
        delegate* unmanaged[Cdecl]<NativeProductApi*, int> bind = &ProductExports.Bind;
        Expect(bind(&api) == 1, "generated product bootstrap did not bind");

        NativeProductCreateArgs create = default;
        void* handle = null;
        Expect(api.create(&create, &handle) == 1 && handle is not null, "generated product bootstrap did not create the product");
        Expect(Snapshot is { LifecycleState: ProductLifecycleState.Created, HasObservedUpdate: false, Generation: null, ControlRevision: null }, "create fabricated update facts");

        FixtureProduct.FailNext("start");
        Expect(api.start(handle) == 99, "failed start did not cross the generated error path");
        api.complete_call(handle, 0, 0);
        Expect(Snapshot is { LifecycleState: ProductLifecycleState.Created, HasObservedUpdate: false }, "failed start changed retained facts");
        Expect(api.start(handle) == 1, "successful start failed");
        Expect(Snapshot is { LifecycleState: ProductLifecycleState.Created, HasObservedUpdate: false }, "successful start published before completion");
        api.complete_call(handle, 1, 0);
        Expect(Snapshot is { LifecycleState: ProductLifecycleState.Running, HasObservedUpdate: false }, "start did not retain running/no-update state");

        NativeProductUpdateArgs update = new()
        {
            facts = new NativeProductUpdateFacts
            {
                mode = NativeProductUpdateMode.NativeProductUpdateMode_Realtime,
                lifecycle_state = NativeProductLifecycleState.NativeProductLifecycleState_Running,
                generation = 17,
                control_revision = 23,
                observed_host_time_nanoseconds = 29,
                simulation_step = 31,
                fixed_step_hz = 60,
                admitted_step_count = 1,
                dropped_step_count = 0,
                fixed_delta_seconds = 1.0 / 60.0,
            },
        };
        NativeProductUpdateResult updateResult = default;
        FixtureProduct.FailNext("update");
        Expect(api.update(handle, &update, &updateResult) == 99, "failed update did not cross the generated error path");
        api.complete_call(handle, 0, 0);
        Expect(Snapshot is { LifecycleState: ProductLifecycleState.Running, HasObservedUpdate: false }, "failed update changed retained facts");
        Expect(api.update(handle, &update, &updateResult) == 1, "successful update failed");
        Expect(Snapshot is { LifecycleState: ProductLifecycleState.Running, HasObservedUpdate: false }, "successful update published before completion");
        api.complete_call(handle, 0, 0);
        Expect(Snapshot is { LifecycleState: ProductLifecycleState.Running, HasObservedUpdate: false }, "uncommitted update changed retained facts");
        Expect(api.update(handle, &update, &updateResult) == 1, "committed update callback failed");
        api.complete_call(handle, 1, 0);
        Expect(Snapshot is { LifecycleState: ProductLifecycleState.Running, HasObservedUpdate: true, Generation: 17, ControlRevision: 23, LatestUpdateFacts: var copied }
            && copied == new ProductUpdateFacts(ProductUpdateMode.Realtime, ProductLifecycleState.Running, 17, 23, 29, 31, 60, 1, 0, 1.0 / 60.0), "successful update facts were not copied");

        AssertLifecycleFailureThenSuccess("pause", api.pause, api.complete_call, handle, ProductLifecycleState.Paused, expectUpdate: true);
        AssertLifecycleFailureThenSuccess("resume", api.resume, api.complete_call, handle, ProductLifecycleState.Running, expectUpdate: true);
        AssertLifecycleFailureThenSuccess("restart", api.restart, api.complete_call, handle, ProductLifecycleState.Running, expectUpdate: false);
        AssertLifecycleFailureThenSuccess("shutdown", api.shutdown, api.complete_call, handle, ProductLifecycleState.Shutdown, expectUpdate: false);

        api.destroy(handle);
        return 0;
    }

    private static unsafe void AssertLifecycleFailureThenSuccess(
        string callback,
        delegate* unmanaged[Cdecl]<void*, int> invoke,
        delegate* unmanaged[Cdecl]<void*, byte, byte, void> complete,
        void* handle,
        ProductLifecycleState expectedState,
        bool expectUpdate)
    {
        DebugExecutionSnapshot before = Snapshot;
        FixtureProduct.FailNext(callback);
        Expect(invoke(handle) == 99, $"failed {callback} did not cross the generated error path");
        complete(handle, 0, 0);
        Expect(Snapshot == before, $"failed {callback} changed retained facts");
        Expect(invoke(handle) == 1, $"successful {callback} failed");
        Expect(Snapshot == before, $"successful {callback} published before completion");
        complete(handle, 0, 0);
        Expect(Snapshot == before, $"uncommitted {callback} changed retained facts");
        Expect(invoke(handle) == 1, $"committed {callback} failed");
        complete(handle, 1, 0);
        Expect(Snapshot is { LifecycleState: var state, HasObservedUpdate: var observed } && state == expectedState && observed == expectUpdate, $"successful {callback} retained the wrong facts");
    }

    private static DebugExecutionSnapshot Snapshot => FixtureProduct.Debugging?.Snapshot
        ?? throw new InvalidOperationException("product did not retain its debug execution context");

    private static void Expect(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }

}

public sealed class FixtureProduct : IEngineProduct
{
    private static string? sFailure;

    internal static DebugExecutionContext? Debugging { get; private set; }

    public FixtureProduct(ProductCreateContext context)
    {
        Debugging = context.Debugging;
    }

    internal static void FailNext(string callback) => sFailure = callback;

    public void Start() => ThrowIfRequested("start");

    public ProductUpdateResult Update(ProductUpdate update)
    {
        ThrowIfRequested("update");
        return ProductUpdateResult.None;
    }

    public void Pause() => ThrowIfRequested("pause");

    public void Resume() => ThrowIfRequested("resume");

    public void Restart() => ThrowIfRequested("restart");

    public void Shutdown() => ThrowIfRequested("shutdown");

    public void Dispose() { }

    private static void ThrowIfRequested(string callback)
    {
        if (!string.Equals(sFailure, callback, StringComparison.Ordinal))
        {
            return;
        }

        sFailure = null;
        throw new InvalidOperationException($"fixture {callback} failure");
    }
}
