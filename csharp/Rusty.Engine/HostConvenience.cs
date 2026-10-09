namespace Rusty.Engine;

/// <summary>Ending the product from its own UI, such as a title menu's Quit.</summary>
public static class HostConvenience
{
    /// <summary>
    /// Close the window and stop the host once this product call returns, as closing the
    /// window does; <c>rusty dev</c> then stops rather than restarting. Only where
    /// <c>Read().ExitAvailable</c> is true (window output); elsewhere it throws
    /// <see cref="EngineCallException"/> with <c>ENGINE_HOST_EXIT_UNAVAILABLE</c>.
    /// </summary>
    public static void RequestExit(this IHostService host)
    {
        ArgumentNullException.ThrowIfNull(host);
        host.RequestExit(default);
    }
}
