#nullable enable
using System;
using Rusty.Engine;

namespace SdkPackageConsumer;

/// <summary>
/// Catches managed exceptions the CPU raises inside one Update: a null
/// dereference in product code and in a generated Engine call (SIGSEGV), and
/// an integer division by zero (SIGFPE). The host must deliver each as an
/// ordinary exception and keep settling the callback, not die by signal.
/// </summary>
internal static class HardwareExceptionChecks
{
    private const string UnsetVariable = "RUSTY_ENGINE_HARDWARE_EXCEPTION_CHECK_UNSET";

    internal static void Run(IEngineContext engine)
    {
        Expect<NullReferenceException>("product null dereference", () => _ = Missing()!.Length);
        Expect<NullReferenceException>("generated Engine call with null", () => engine.CameraView.SetActiveCamera(null!));
        Expect<DivideByZeroException>("integer division by zero", () => _ = 1 / Zero());
        Console.WriteLine("HARDWARE_EXCEPTION_CHECKS_PASSED");
    }

    private static string? Missing() => Environment.GetEnvironmentVariable(UnsetVariable);

    private static int Zero() => Environment.ProcessorCount - Environment.ProcessorCount;

    private static void Expect<TException>(string label, Action action)
        where TException : Exception
    {
        try
        {
            action();
        }
        catch (TException)
        {
            return;
        }

        throw new InvalidOperationException($"{label} did not raise {typeof(TException).Name}.");
    }
}
