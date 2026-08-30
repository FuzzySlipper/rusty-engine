using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using Microsoft.CodeAnalysis;

namespace Rusty.Engine.ProductGenerator;

internal static class DebugCommandCatalogGenerator
{
    private const string DebugCommandAttribute = "Rusty.Engine.Debugging.DebugCommandAttribute";
    private const string DebugCommandModule = "Rusty.Engine.Debugging.IDebugCommandModule";
    private const string DebugCommandResult = "Rusty.Engine.Debugging.DebugCommandResult";
    private const string EngineProductAttribute = "Rusty.Engine.EngineProductAttribute";

    private static readonly DiagnosticDescriptor InvalidCommand = new(
        "RUSTYDBG001",
        "Unsupported debug command signature",
        "Debug command '{0}' is unsupported: {1}",
        "Rusty.Engine.Debugging",
        DiagnosticSeverity.Error,
        true);

    private static readonly DiagnosticDescriptor DuplicateCommand = new(
        "RUSTYDBG002",
        "Duplicate debug command name",
        "Debug command name '{0}' is also declared by '{1}'",
        "Rusty.Engine.Debugging",
        DiagnosticSeverity.Error,
        true);

    internal static void Generate(SourceProductionContext output, Compilation compilation)
    {
        INamedTypeSymbol? moduleInterface = compilation.GetTypeByMetadataName(DebugCommandModule);
        if (moduleInterface is null)
        {
            return;
        }

        List<Command> commands = new();
        foreach (INamedTypeSymbol type in AllTypes(compilation))
        {
            foreach (IMethodSymbol method in type.GetMembers().OfType<IMethodSymbol>())
            {
                AttributeData? attribute = method.GetAttributes()
                    .FirstOrDefault(candidate => candidate.AttributeClass?.ToDisplayString() == DebugCommandAttribute);
                if (attribute is null)
                {
                    continue;
                }

                if (!TryCreateCommand(compilation, method, attribute, moduleInterface, out Command? command, out string? reason))
                {
                    output.ReportDiagnostic(Diagnostic.Create(InvalidCommand, method.Locations.FirstOrDefault(), method.Name, reason));
                    continue;
                }
                commands.Add(command!);
            }
        }

        foreach (IGrouping<string, Command> duplicate in commands.GroupBy(command => command.Name, StringComparer.Ordinal).Where(group => group.Count() > 1))
        {
            Command[] members = duplicate.OrderBy(command => command.SortKey, StringComparer.Ordinal).ToArray();
            foreach (Command member in members.Skip(1))
            {
                output.ReportDiagnostic(Diagnostic.Create(
                    DuplicateCommand,
                    member.Method.Locations.FirstOrDefault(),
                    duplicate.Key,
                    members[0].Method.ToDisplayString()));
            }
        }

        Command[] unique = commands
            .GroupBy(command => command.Name, StringComparer.Ordinal)
            .Select(group => group.OrderBy(command => command.SortKey, StringComparer.Ordinal).First())
            .OrderBy(command => command.Name, StringComparer.Ordinal)
            .ThenBy(command => command.SortKey, StringComparer.Ordinal)
            .ToArray();
        output.AddSource("DebugCommandCatalog.g.cs", SourceText(unique));
    }

    private static bool TryCreateCommand(
        Compilation compilation,
        IMethodSymbol method,
        AttributeData attribute,
        INamedTypeSymbol moduleInterface,
        out Command? command,
        out string? reason)
    {
        command = null;
        reason = null;
        string? name = attribute.ConstructorArguments.Length == 1 ? attribute.ConstructorArguments[0].Value as string : null;
        if (string.IsNullOrWhiteSpace(name) || name.Any(char.IsWhiteSpace))
        {
            reason = "the command name must be a non-empty, whitespace-free token";
            return false;
        }
        if (method.IsStatic || method.IsGenericMethod || method.MethodKind != MethodKind.Ordinary)
        {
            reason = "only non-static, non-generic ordinary instance methods are supported";
            return false;
        }
        bool localModule = SymbolEqualityComparer.Default.Equals(method.ContainingAssembly, compilation.Assembly);
        if (method.ContainingType.IsGenericType || !IsAccessibleFromGeneratedCode(method, localModule))
        {
            reason = "the method must be accessible to generated code on a non-generic module type (public when declared in a referenced product assembly)";
            return false;
        }
        if (!method.ContainingType.AllInterfaces.Any(candidate => SymbolEqualityComparer.Default.Equals(candidate, moduleInterface)))
        {
            reason = $"the containing type must implement {DebugCommandModule}";
            return false;
        }
        if (!IsSupportedReturnType(method.ReturnType))
        {
            reason = "return type must be void, string, or DebugCommandResult";
            return false;
        }
        foreach (IParameterSymbol parameter in method.Parameters)
        {
            if (parameter.RefKind != RefKind.None || parameter.IsParams || !IsSupportedParameterType(parameter.Type))
            {
                reason = $"parameter '{parameter.Name}' must be a value string, enum, or ISpanParsable value";
                return false;
            }
        }

        string description = attribute.NamedArguments.FirstOrDefault(pair => pair.Key == "Description").Value.Value as string ?? string.Empty;
        command = new Command(name!, description, method, method.Parameters.ToArray());
        return true;
    }

    private static bool IsSupportedReturnType(ITypeSymbol type)
        => type.SpecialType == SpecialType.System_Void
            || type.SpecialType == SpecialType.System_String
            || type.ToDisplayString() == DebugCommandResult;

    private static bool IsSupportedParameterType(ITypeSymbol type)
    {
        if (type.SpecialType == SpecialType.System_String || type.TypeKind == TypeKind.Enum)
        {
            return true;
        }
        return type.AllInterfaces.Any(candidate => candidate.OriginalDefinition.ToDisplayString() == "System.ISpanParsable<TSelf>");
    }

    private static bool IsAccessibleFromGeneratedCode(IMethodSymbol method, bool localModule)
    {
        if (!IsAccessible(method.DeclaredAccessibility, localModule))
        {
            return false;
        }
        for (INamedTypeSymbol? type = method.ContainingType; type is not null; type = type.ContainingType)
        {
            if (!IsAccessible(type.DeclaredAccessibility, localModule))
            {
                return false;
            }
        }
        return true;
    }

    private static bool IsAccessible(Accessibility accessibility, bool localModule)
        => accessibility == Accessibility.Public
            || (localModule && (accessibility == Accessibility.Internal || accessibility == Accessibility.ProtectedOrInternal));

    private static IEnumerable<INamedTypeSymbol> AllTypes(Compilation compilation)
    {
        HashSet<string> visited = new(StringComparer.Ordinal);
        foreach (IAssemblySymbol assembly in ProductAssemblies(compilation))
        {
            foreach (INamedTypeSymbol type in AllTypes(assembly.GlobalNamespace))
            {
                if (visited.Add(type.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat)))
                {
                    yield return type;
                }
            }
        }
    }

    private static IEnumerable<IAssemblySymbol> ProductAssemblies(Compilation compilation)
    {
        HashSet<string> visited = new(StringComparer.Ordinal);
        IAssemblySymbol[] productAssemblies = compilation.Assembly.GetAttributes()
                .Where(attribute => attribute.AttributeClass?.ToDisplayString() == EngineProductAttribute)
                .Select(attribute => attribute.ConstructorArguments.Length == 1 ? attribute.ConstructorArguments[0].Value as INamedTypeSymbol : null)
                .Where(type => type is not null)
                .Select(type => type!.ContainingAssembly)
                .ToArray();
        foreach (IAssemblySymbol assembly in new[] { compilation.Assembly }
            .Concat(productAssemblies)
            .Concat(DirectReferences(compilation.Assembly))
            .Concat(productAssemblies.SelectMany(DirectReferences)))
        {
            if (visited.Add(assembly.Identity.GetDisplayName()))
            {
                yield return assembly;
            }
        }
    }

    private static IEnumerable<IAssemblySymbol> DirectReferences(IAssemblySymbol assembly)
        => assembly.Modules.SelectMany(module => module.ReferencedAssemblySymbols);

    private static IEnumerable<INamedTypeSymbol> AllTypes(INamespaceSymbol @namespace)
    {
        foreach (INamedTypeSymbol type in @namespace.GetTypeMembers())
        {
            yield return type;
            foreach (INamedTypeSymbol nested in AllNestedTypes(type))
            {
                yield return nested;
            }
        }
        foreach (INamespaceSymbol child in @namespace.GetNamespaceMembers())
        {
            foreach (INamedTypeSymbol type in AllTypes(child))
            {
                yield return type;
            }
        }
    }

    private static IEnumerable<INamedTypeSymbol> AllNestedTypes(INamedTypeSymbol type)
    {
        foreach (INamedTypeSymbol nested in type.GetTypeMembers())
        {
            yield return nested;
            foreach (INamedTypeSymbol child in AllNestedTypes(nested))
            {
                yield return child;
            }
        }
    }

    private static string SourceText(IReadOnlyList<Command> commands)
    {
        INamedTypeSymbol[] modules = commands
            .Select(command => command.Method.ContainingType)
            .GroupBy(type => type.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat), StringComparer.Ordinal)
            .Select(group => group.First())
            .OrderBy(type => type.ToDisplayString(), StringComparer.Ordinal)
            .ToArray();
        StringBuilder source = new();
        source.AppendLine("// <auto-generated />");
        source.AppendLine("#nullable enable");
        source.AppendLine("using System;");
        source.AppendLine("using System.Collections.Generic;");
        source.AppendLine("using System.Globalization;");
        source.AppendLine("using Rusty.Engine;");
        source.AppendLine("using Rusty.Engine.Debugging;");
        source.AppendLine();
        source.AppendLine("namespace Rusty.Engine.NativeProduct;");
        source.AppendLine();
        source.AppendLine("internal static class GeneratedDebugCommandCatalogFactory");
        source.AppendLine("{");
        source.AppendLine("    internal static IDebugCommandCatalog Create(IEngineProduct product)");
        source.AppendLine("    {");
        source.AppendLine("        GeneratedDebugCommandCatalog catalog = new();");
        source.AppendLine("        if (product is IDebugCommandModuleSource source) source.RegisterDebugCommands(catalog);");
        source.AppendLine("        return catalog;");
        source.AppendLine("    }");
        source.AppendLine("}");
        source.AppendLine();
        source.AppendLine("internal sealed class GeneratedDebugCommandCatalog : IDebugCommandCatalog, IDebugCommandModuleRegistrar");
        source.AppendLine("{");
        foreach (INamedTypeSymbol module in modules)
        {
            source.Append("    private ").Append(TypeName(module)).Append("? ").Append(ModuleField(module)).AppendLine(";");
        }
        source.AppendLine();
        source.AppendLine("    private static readonly DebugCommandDescriptor[] Descriptors =");
        source.AppendLine("    [");
        foreach (Command command in commands)
        {
            source.Append("        new DebugCommandDescriptor(\"").Append(Escape(command.Name)).Append("\", \"")
                .Append(Escape(command.Description)).Append("\", new DebugCommandParameterDescriptor[] { ");
            source.Append(string.Join(", ", command.Parameters.Select(parameter =>
                $"new DebugCommandParameterDescriptor(\"{Escape(parameter.Name)}\", \"{Escape(TypeDisplay(parameter.Type))}\")")));
            source.AppendLine(" }),");
        }
        source.AppendLine("    ];");
        source.AppendLine();
        source.AppendLine("    public IReadOnlyList<DebugCommandDescriptor> Commands => Descriptors;");
        source.AppendLine();
        source.AppendLine("    public DebugCommandRegistrationResult Register<TModule>(TModule module) where TModule : class, IDebugCommandModule");
        source.AppendLine("    {");
        source.AppendLine("        if (module is null) return new(DebugCommandRegistrationStatus.UnsupportedModule, \"A debug module instance is required.\");");
        foreach (INamedTypeSymbol module in modules)
        {
            string field = ModuleField(module);
            string variable = ModuleVariable(module);
            source.Append("        if (module is ").Append(TypeName(module)).Append(' ').Append(variable).AppendLine(")");
            source.AppendLine("        {");
            source.Append("            if (").Append(field).Append(" is not null) return new(DebugCommandRegistrationStatus.AlreadyRegistered, \"")
                .Append(Escape(TypeDisplay(module))).AppendLine(" is already registered.\");");
            source.Append("            ").Append(field).Append(" = ").Append(variable).AppendLine(";");
            source.Append("            return new(DebugCommandRegistrationStatus.Registered, \"").Append(Escape(TypeDisplay(module))).AppendLine(" registered.\");");
            source.AppendLine("        }");
        }
        source.AppendLine("        return new(DebugCommandRegistrationStatus.UnsupportedModule, \"No generated debug commands exist for the supplied module type.\");");
        source.AppendLine("    }");
        source.AppendLine();
        source.AppendLine("    public DebugCommandResult Execute(string commandLine)");
        source.AppendLine("    {");
        source.AppendLine("        if (!DebugCommandLine.TryTokenize(commandLine, out string[] tokens, out string error)) return DebugCommandResult.Failure(DebugCommandStatus.InvalidArguments, error);");
        source.AppendLine("        if (tokens.Length == 0) return DebugCommandResult.Failure(DebugCommandStatus.Empty, \"A command name is required.\");");
        source.AppendLine("        return tokens[0] switch");
        source.AppendLine("        {");
        foreach (Command command in commands)
        {
            source.Append("            \"").Append(Escape(command.Name)).Append("\" => Execute_").Append(CommandIdentifier(command)).AppendLine("(tokens),");
        }
        source.AppendLine("            _ => DebugCommandResult.Failure(DebugCommandStatus.UnknownCommand, $\"Unknown debug command '{tokens[0]}'.\"),");
        source.AppendLine("        };");
        source.AppendLine("    }");
        foreach (Command command in commands)
        {
            AppendCommand(source, command);
        }
        source.AppendLine("}");
        return source.ToString();
    }

    private static void AppendCommand(StringBuilder source, Command command)
    {
        string module = ModuleField(command.Method.ContainingType);
        source.AppendLine();
        source.Append("    private DebugCommandResult Execute_").Append(CommandIdentifier(command)).AppendLine("(string[] tokens)");
        source.AppendLine("    {");
        source.Append("        if (").Append(module).Append(" is null) return DebugCommandResult.Failure(DebugCommandStatus.ModuleUnavailable, \"")
            .Append(Escape(command.Name)).AppendLine(" is not registered on a live module.\");");
        source.Append("        if (tokens.Length != ").Append(command.Parameters.Length + 1).Append(") return DebugCommandResult.Failure(DebugCommandStatus.InvalidArguments, \"")
            .Append(Escape(Usage(command))).AppendLine("\");");
        for (int index = 0; index < command.Parameters.Length; index++)
        {
            IParameterSymbol parameter = command.Parameters[index];
            string variable = $"arg{index}";
            string type = TypeName(parameter.Type);
            if (parameter.Type.SpecialType == SpecialType.System_String)
            {
                source.Append("        string ").Append(variable).Append(" = tokens[").Append(index + 1).AppendLine("];");
            }
            else if (parameter.Type.TypeKind == TypeKind.Enum)
            {
                source.Append("        if (!Enum.TryParse<").Append(type).Append(">(tokens[").Append(index + 1).Append("], true, out ").Append(type).Append(' ').Append(variable)
                    .Append(")) return DebugCommandResult.Failure(DebugCommandStatus.InvalidArguments, \"Argument '").Append(Escape(parameter.Name)).Append("' must be a ").Append(Escape(TypeDisplay(parameter.Type))).AppendLine(" value.\");");
            }
            else
            {
                source.Append("        if (!DebugCommandLine.TryParse<").Append(type).Append(">(tokens[").Append(index + 1).Append("], out ").Append(type).Append(' ').Append(variable)
                    .Append(")) return DebugCommandResult.Failure(DebugCommandStatus.InvalidArguments, \"Argument '").Append(Escape(parameter.Name)).Append("' must be a ").Append(Escape(TypeDisplay(parameter.Type))).AppendLine(" value.\");");
            }
        }
        source.AppendLine("        try");
        source.AppendLine("        {");
        string arguments = string.Join(", ", Enumerable.Range(0, command.Parameters.Length).Select(index => $"arg{index}"));
        if (command.Method.ReturnType.SpecialType == SpecialType.System_Void)
        {
            source.Append("            ").Append(module).Append('.').Append(command.Method.Name).Append('(').Append(arguments).AppendLine(");");
            source.Append("            return DebugCommandResult.Success(\"").Append(Escape(command.Name)).AppendLine(" executed.\");");
        }
        else if (command.Method.ReturnType.SpecialType == SpecialType.System_String)
        {
            source.Append("            return DebugCommandResult.Success(").Append(module).Append('.').Append(command.Method.Name).Append('(').Append(arguments).AppendLine(") ?? string.Empty);");
        }
        else
        {
            source.Append("            return ").Append(module).Append('.').Append(command.Method.Name).Append('(').Append(arguments).AppendLine(");");
        }
        source.AppendLine("        }");
        source.AppendLine("        catch (Exception exception)");
        source.AppendLine("        {");
        source.Append("            return DebugCommandResult.Failure(DebugCommandStatus.Failed, $\"Debug command '").Append(Escape(command.Name)).AppendLine("' failed: {exception.Message}\");");
        source.AppendLine("        }");
        source.AppendLine("    }");
    }

    private static string Usage(Command command)
        => command.Parameters.Length == 0
            ? $"Usage: {command.Name}"
            : $"Usage: {command.Name} {string.Join(" ", command.Parameters.Select(parameter => $"<{parameter.Name}>"))}";

    private static string TypeName(ITypeSymbol type) => type.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat);

    private static string TypeDisplay(ITypeSymbol type) => type.ToDisplayString(SymbolDisplayFormat.MinimallyQualifiedFormat);

    private static string ModuleField(INamedTypeSymbol type)
        => "_module_" + Identifier(type.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat));

    private static string ModuleVariable(INamedTypeSymbol type)
        => "typed_" + Identifier(type.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat));

    private static string CommandIdentifier(Command command)
        => Identifier(command.Name + "_" + command.SortKey);

    private static string Identifier(string value)
    {
        StringBuilder builder = new();
        foreach (char character in value)
        {
            builder.Append(char.IsLetterOrDigit(character) ? character : '_');
        }
        return builder.ToString();
    }

    private static string Escape(string value) => value.Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\r", "\\r").Replace("\n", "\\n");

    private sealed class Command
    {
        internal Command(string name, string description, IMethodSymbol method, IParameterSymbol[] parameters)
        {
            Name = name;
            Description = description;
            Method = method;
            Parameters = parameters;
            SortKey = method.ToDisplayString(SymbolDisplayFormat.FullyQualifiedFormat);
        }

        internal string Name { get; }
        internal string Description { get; }
        internal IMethodSymbol Method { get; }
        internal IParameterSymbol[] Parameters { get; }
        internal string SortKey { get; }
    }
}
