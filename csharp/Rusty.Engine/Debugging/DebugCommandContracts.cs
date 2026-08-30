using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text;

namespace Rusty.Engine.Debugging;

/// <summary>
/// Marks an ordinary module method for inclusion in the product's generated live-debug catalog.
/// The attribute is consumed at compile time; it is never discovered at runtime.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false, Inherited = false)]
public sealed class DebugCommandAttribute : Attribute
{
    public DebugCommandAttribute(string name)
    {
        Name = name;
    }

    public string Name { get; }

    public string? Description { get; set; }
}

/// <summary>
/// Marks an ordinary live object that may expose generated debug-command methods.
/// Instances are supplied explicitly by the product; the Engine never scans for them.
/// </summary>
public interface IDebugCommandModule
{
}

/// <summary>
/// Lets a product explicitly supply the live module instances used by its generated catalog.
/// </summary>
public interface IDebugCommandModuleSource
{
    void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar);
}

/// <summary>
/// A generated catalog implements this narrow registration surface. Products register their
/// currently live modules during setup instead of relying on discovery or a service locator.
/// </summary>
public interface IDebugCommandModuleRegistrar
{
    DebugCommandRegistrationResult Register<TModule>(TModule module)
        where TModule : class, IDebugCommandModule;
}

public enum DebugCommandRegistrationStatus
{
    Registered,
    AlreadyRegistered,
    UnsupportedModule,
}

public readonly record struct DebugCommandRegistrationResult(
    DebugCommandRegistrationStatus Status,
    string Message)
{
    public bool Succeeded => Status == DebugCommandRegistrationStatus.Registered;
}

public enum DebugCommandStatus
{
    Success,
    Empty,
    UnknownCommand,
    ModuleUnavailable,
    InvalidArguments,
    Failed,
}

/// <summary>
/// Explicit managed result from a generated command invocation. No command failure needs to
/// escape the catalog or cross the eventual host boundary as an exception.
/// </summary>
public readonly record struct DebugCommandResult(DebugCommandStatus Status, string Message)
{
    public bool Succeeded => Status == DebugCommandStatus.Success;

    public static DebugCommandResult Success(string message = "") => new(DebugCommandStatus.Success, message);

    public static DebugCommandResult Failure(DebugCommandStatus status, string message) => new(status, message);
}

public readonly record struct DebugCommandParameterDescriptor(string Name, string TypeName);

public readonly record struct DebugCommandDescriptor(
    string Name,
    string Description,
    IReadOnlyList<DebugCommandParameterDescriptor> Parameters);

/// <summary>
/// Runtime shape shared by the generated catalog and its eventual generated product callback.
/// Command meanings and direct invocations remain in generated product code.
/// </summary>
public interface IDebugCommandCatalog
{
    IReadOnlyList<DebugCommandDescriptor> Commands { get; }

    DebugCommandResult Execute(string commandLine);
}

/// <summary>
/// Small command-line tokenizer for the compiled catalog. It supports quoted argument values
/// and backslash escaping; it neither parses a data protocol nor discovers command methods.
/// </summary>
public static class DebugCommandLine
{
    public static bool TryTokenize(string? commandLine, out string[] tokens, out string error)
    {
        if (string.IsNullOrWhiteSpace(commandLine))
        {
            tokens = Array.Empty<string>();
            error = string.Empty;
            return true;
        }

        List<string> result = [];
        StringBuilder token = new();
        bool quoted = false;
        bool escaping = false;
        bool tokenStarted = false;

        foreach (char character in commandLine)
        {
            if (escaping)
            {
                token.Append(character);
                escaping = false;
                tokenStarted = true;
                continue;
            }

            if (character == '\\')
            {
                escaping = true;
                tokenStarted = true;
                continue;
            }

            if (character == '"')
            {
                quoted = !quoted;
                tokenStarted = true;
                continue;
            }

            if (!quoted && char.IsWhiteSpace(character))
            {
                if (tokenStarted)
                {
                    result.Add(token.ToString());
                    token.Clear();
                    tokenStarted = false;
                }
                continue;
            }

            token.Append(character);
            tokenStarted = true;
        }

        if (escaping)
        {
            tokens = Array.Empty<string>();
            error = "Command line ends with an escape character.";
            return false;
        }
        if (quoted)
        {
            tokens = Array.Empty<string>();
            error = "Command line has an unterminated quoted argument.";
            return false;
        }
        if (tokenStarted)
        {
            result.Add(token.ToString());
        }

        tokens = result.ToArray();
        error = string.Empty;
        return true;
    }

    public static bool TryParse<T>(string value, out T parsed)
        where T : ISpanParsable<T>
    {
        if (T.TryParse(value.AsSpan(), CultureInfo.InvariantCulture, out T? candidate))
        {
            parsed = candidate!;
            return true;
        }

        parsed = default!;
        return false;
    }
}
