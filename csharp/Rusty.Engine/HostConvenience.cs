using System.Text;

namespace Rusty.Engine;

/// <summary>
/// Ending the product from its own UI, such as a title menu's Quit, and the
/// text readouts and common calls of a setup screen: choosing a folder and
/// running one of the product's declared tools.
/// </summary>
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

    /// <summary>
    /// The absolute directory relative <c>Content.OpenContainer</c> paths resolve in and
    /// tools run in; empty where the host names none.
    /// </summary>
    public static string ReadInstalledContentText(this IHostService host)
    {
        ArgumentNullException.ThrowIfNull(host);
        return Encoding.UTF8.GetString(host.ReadInstalledContent(default).Span);
    }

    /// <summary>Opens the platform's folder picker with this title.</summary>
    public static FolderChoice ChooseFolder(this IHostService host, string title)
    {
        ArgumentNullException.ThrowIfNull(host);
        return host.ChooseFolder(new FolderChoiceRequest(title));
    }

    /// <summary>The chosen folder's absolute path once <c>Chosen</c>; empty otherwise.</summary>
    public static string ReadPathText(this IHostService host, FolderChoice choice)
    {
        ArgumentNullException.ThrowIfNull(host);
        return Encoding.UTF8.GetString(host.ReadFolderChoicePath(choice).Span);
    }

    /// <summary>Why no picker could be shown once <c>Unavailable</c>; empty otherwise.</summary>
    public static string ReadDiagnosticText(this IHostService host, FolderChoice choice)
    {
        ArgumentNullException.ThrowIfNull(host);
        return Encoding.UTF8.GetString(host.ReadFolderChoiceDiagnostic(choice).Span);
    }

    /// <summary>Starts the declared tool named by its project name with these arguments.</summary>
    public static ToolRun RunTool(this IHostService host, string tool, params string[] arguments)
    {
        ArgumentNullException.ThrowIfNull(host);
        ArgumentNullException.ThrowIfNull(arguments);
        return host.RunTool(new ToolRunRequest(tool, arguments.Select(argument => new ToolArgument(argument)).ToArray()));
    }

    /// <summary>The tool's recent output lines, oldest first.</summary>
    public static string ReadOutputText(this IHostService host, ToolRun run)
    {
        ArgumentNullException.ThrowIfNull(host);
        return Encoding.UTF8.GetString(host.ReadToolRunOutput(run).Span);
    }
}
