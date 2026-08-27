using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using ClangSharp.Interop;

if (args.Length != 5) throw new ArgumentException("usage: BindingGenerator <header> <contracts.cs> <values.cs> <inputs-dir> <clang-resource-dir>");
BindingModel model = BindingModel.Parse(args[0], args[4]);
Directory.CreateDirectory(Path.GetDirectoryName(args[1])!);
Directory.CreateDirectory(args[3]);
File.WriteAllText(args[1], Emit.Contracts(model));
File.WriteAllText(args[2], Emit.Values(model));
File.WriteAllText(Path.Combine(args[3], "Interop.g.cs"), Emit.Interop(model));
File.WriteAllText(Path.Combine(args[3], "EngineServiceImplementations.g.cs"), Emit.Implementations(model));

internal sealed record Field(string Name, string Type);
internal sealed record LeaseCollection(Field Pointer, Field Count);
internal sealed record Struct(string Name, IReadOnlyList<Field> Fields);
internal sealed record EnumMember(string Name, long Value);
internal sealed record Enum(string Name, IReadOnlyList<EnumMember> Members);
internal sealed record Callback(string Name, string ReturnType, IReadOnlyList<string> Parameters);
internal sealed record Service(string Name, IReadOnlyList<(string Name, string Callback)> Operations);

internal sealed class BindingModel
{
    public required IReadOnlyDictionary<string, Struct> Structs { get; init; }
    public required IReadOnlyDictionary<string, Enum> Enums { get; init; }
    public required IReadOnlyDictionary<string, Callback> Callbacks { get; init; }
    public required IReadOnlyList<Service> Services { get; init; }

    public static unsafe BindingModel Parse(string header, string clangResourceDirectory)
    {
        using CXIndex index = CXIndex.Create(false, false);
        using CXTranslationUnit unit = CXTranslationUnit.Parse(index, header,
            ["-x", "c", "-std=c11", "--target=x86_64-unknown-linux-gnu", $"-resource-dir={clangResourceDirectory}"], [], CXTranslationUnit_Flags.CXTranslationUnit_None);
        List<CXCursor> declarations = Children(unit.Cursor);
        Dictionary<string, Struct> structs = declarations.Where(cursor => cursor.Kind == CXCursorKind.CXCursor_StructDecl && cursor.IsDefinition && cursor.Spelling.ToString().StartsWith("Native", StringComparison.Ordinal))
            .Select(cursor => new Struct(cursor.Spelling.ToString(), Children(cursor).Where(field => field.Kind == CXCursorKind.CXCursor_FieldDecl).Select(field => new Field(field.Spelling.ToString(), field.Type.Spelling.ToString())).ToArray()))
            .GroupBy(value => value.Name).ToDictionary(group => group.Key, group => group.First(), StringComparer.Ordinal);
        Dictionary<string, Enum> enums = declarations.Where(cursor => cursor.Kind == CXCursorKind.CXCursor_EnumDecl && cursor.IsDefinition && cursor.Spelling.ToString().StartsWith("Native", StringComparison.Ordinal))
            .Select(cursor => new Enum(cursor.Spelling.ToString(), Children(cursor).Where(member => member.Kind == CXCursorKind.CXCursor_EnumConstantDecl).Select(member => new EnumMember(member.Spelling.ToString(), member.EnumConstantDeclValue)).ToArray()))
            .GroupBy(value => value.Name).ToDictionary(group => group.Key, group => group.First(), StringComparer.Ordinal);
        Dictionary<string, Callback> callbacks = declarations.Where(cursor => cursor.Kind == CXCursorKind.CXCursor_TypedefDecl && cursor.Spelling.ToString().StartsWith("Native", StringComparison.Ordinal))
            .Select(cursor => new Callback(cursor.Spelling.ToString(), cursor.Type.CanonicalType.Spelling.ToString().Split(" (*)", StringSplitOptions.None)[0], Children(cursor).Where(parameter => parameter.Kind == CXCursorKind.CXCursor_ParmDecl).Select(parameter => parameter.Type.Spelling.ToString()).ToArray()))
            .Where(callback => callback.Parameters.Count > 0).GroupBy(value => value.Name).ToDictionary(group => group.Key, group => group.First(), StringComparer.Ordinal);
        if (!structs.TryGetValue("NativeEngineApi", out Struct? engine)) throw new InvalidOperationException("NativeEngineApi was not found in the generated cbindgen header.");
        Service[] services = engine.Fields.Select(field =>
        {
            string table = Bare(field.Type);
            if (!structs.TryGetValue(table, out Struct? tableStruct) || !table.EndsWith("Api", StringComparison.Ordinal)) throw new InvalidOperationException($"NativeEngineApi field {field.Name} has unsupported table type {field.Type}.");
            var operations = tableStruct.Fields.Where(operation => operation.Name != "context").Select(operation =>
            {
                if (!callbacks.TryGetValue(Bare(operation.Type), out Callback? callback)) throw new InvalidOperationException($"{table}.{operation.Name} references non-callback type {operation.Type}.");
                Validate(table, operation.Name, callback, structs, enums);
                return (operation.Name, callback.Name);
            }).ToArray();
            return new Service(table["Native".Length..^"Api".Length], operations);
        }).ToArray();
        return new BindingModel { Structs = structs, Enums = enums, Callbacks = callbacks, Services = services };
    }

    private static void Validate(string family, string method, Callback callback, IReadOnlyDictionary<string, Struct> structs, IReadOnlyDictionary<string, Enum> enums)
    {
        string signature = $"{callback.ReturnType} ({string.Join(", ", callback.Parameters)})";
        if (callback.ReturnType is not "int" and not "int32_t" || callback.Parameters[0] != "void *") Fail(family, method, signature, "expected context-first int32 status call");
        string[] parameters = callback.Parameters.Skip(1).ToArray();
        if (HasOperationErrorReceipt(parameters))
        {
            ValidateOperationErrorReceipt(family, method, signature, structs, enums);
            parameters = parameters[..^1];
        }
        if (parameters.Length == 0) Fail(family, method, signature, "service calls require one supported input or out receipt");
        if (parameters[^1].Contains('*', StringComparison.Ordinal) && !IsExactBorrowedPointer(parameters[^1]) && !IsExactOutPointer(parameters[^1])) Fail(family, method, signature, $"final pointer {parameters[^1]} must be exactly const T * input or T * out receipt");
        bool hasReceipt = IsExactOutPointer(parameters[^1]);
        int inputs = hasReceipt ? parameters.Length - 1 : parameters.Length;
        if (hasReceipt)
        {
            string receipt = Bare(parameters[^1]);
            if (IsLeaseResult(receipt, structs)) ValidateLeaseResult(family, method, signature, receipt, structs, enums);
            else ValidateFixedType(family, method, signature, receipt, structs, enums, new HashSet<string>(StringComparer.Ordinal), "out receipt");
        }
        if (inputs == 0 && hasReceipt) return;
        if (inputs == 2 && IsExactBorrowedPointer(parameters[0]) && Bare(parameters[1]) == "size_t")
        {
            ValidateFixedType(family, method, signature, Bare(parameters[0]), structs, enums, new HashSet<string>(StringComparer.Ordinal), "pointer/count span element");
            return;
        }
        if (inputs > 1 && IsExactBorrowedPointer(parameters[inputs - 1]))
        {
            for (int index = 0; index < inputs - 1; index++)
            {
                string leading = parameters[index];
                if (leading.Contains('*', StringComparison.Ordinal)) Fail(family, method, signature, $"unsupported leading pointer input {leading}");
                ValidateFixedType(family, method, signature, Bare(leading), structs, enums, new HashSet<string>(StringComparer.Ordinal), "leading direct input");
            }
            string borrowed = parameters[inputs - 1];
            string borrowedBare = Bare(borrowed);
            if (!structs.TryGetValue(borrowedBare, out Struct? request) || request is null) { Fail(family, method, signature, $"borrowed input {borrowed} does not name an emitted struct"); return; }
            ValidateBorrowedRequest(family, method, signature, request, structs, enums, new HashSet<string>(StringComparer.Ordinal));
            return;
        }
        if (inputs != 1) Fail(family, method, signature, "expected one input (or one pointer/count span) plus optional out receipt");
        string input = parameters[0];
        string bare = Bare(input);
        if (IsExactBorrowedPointer(input))
        {
            if (!structs.TryGetValue(bare, out Struct? request) || request is null) { Fail(family, method, signature, $"borrowed input {input} does not name an emitted struct"); return; }
            ValidateBorrowedRequest(family, method, signature, request, structs, enums, new HashSet<string>(StringComparer.Ordinal));
            return;
        }
        if (input.Contains('*', StringComparison.Ordinal)) Fail(family, method, signature, $"unsupported pointer input {input}");
        ValidateFixedType(family, method, signature, bare, structs, enums, new HashSet<string>(StringComparer.Ordinal), "direct input");
    }

    private static void ValidateOperationErrorReceipt(string family, string method, string signature, IReadOnlyDictionary<string, Struct> structs, IReadOnlyDictionary<string, Enum> enums)
    {
        if (!structs.TryGetValue("NativeOperationErrorReceipt", out Struct? value) || value is null) { Fail(family, method, signature, "operation error receipt was not emitted"); return; }
        if (value.Fields.Count != 4
            || value.Fields[0].Name != "service" || Bare(value.Fields[0].Type) != "NativeUtf8Slice"
            || value.Fields[1].Name != "operation" || Bare(value.Fields[1].Type) != "NativeUtf8Slice"
            || value.Fields[2].Name != "status" || Bare(value.Fields[2].Type) is not ("int" or "int32_t")
            || value.Fields[3].Name != "diagnostics" || Bare(value.Fields[3].Type) != "NativeEngineDiagnosticLease")
        {
            Fail(family, method, signature, "NativeOperationErrorReceipt must preserve service/operation/status plus NativeEngineDiagnosticLease");
            return;
        }
        ValidateLeaseResult(family, method, signature, "NativeEngineDiagnosticLease", structs, enums);
    }

    private static void ValidateFixedType(string family, string method, string signature, string type, IReadOnlyDictionary<string, Struct> structs, IReadOnlyDictionary<string, Enum> enums, HashSet<string> seen, string role)
    {
        if (IsScalar(type) || enums.ContainsKey(type)) return;
        if (!structs.TryGetValue(type, out Struct? value) || value is null) { Fail(family, method, signature, $"{role} {type} is not a supported scalar or emitted native struct"); return; }
        if (type.EndsWith("Api", StringComparison.Ordinal) || type is "NativeProductApi" or "NativeProductCreateArgs" or "NativeTurnArgs") Fail(family, method, signature, $"{role} {type} is an API/product table rather than a fixed value");
        if (type is "NativeUtf8Slice" or "NativeByteSlice" or "NativeWritableByteSlice" or "NativeStructuredValue") Fail(family, method, signature, $"{role} {type} is only supported as a specially marshalled request field");
        if (!seen.Add(type)) return;
        foreach (Field field in value.Fields)
        {
            if (field.Type.Contains('*', StringComparison.Ordinal)) Fail(family, method, signature, $"{role} {type}.{field.Name} ({field.Type}) is borrowed and cannot be emitted by-value");
            string nested = Bare(field.Type);
            if (nested is "NativeUtf8Slice" or "NativeByteSlice" or "NativeWritableByteSlice" or "NativeStructuredValue") Fail(family, method, signature, $"{role} {type}.{field.Name} ({field.Type}) requires request-only marshalling");
            ValidateFixedType(family, method, signature, nested, structs, enums, seen, $"{role} field {type}.{field.Name}");
        }
    }

    internal static bool IsLeaseResult(string type, IReadOnlyDictionary<string, Struct> structs) =>
        structs.TryGetValue(type, out Struct? value)
        && value.Name.EndsWith("Lease", StringComparison.Ordinal)
        && value.Fields.Any(field => field.Name == "handle" && Bare(field.Type).EndsWith("LeaseHandle", StringComparison.Ordinal));

    internal static bool IsOperationErrorReceipt(string type) => type == "NativeOperationErrorReceipt";
    internal static bool HasOperationErrorReceipt(IReadOnlyList<string> parameters) => parameters.Count > 0 && IsExactOutPointer(parameters[^1]) && IsOperationErrorReceipt(Bare(parameters[^1]));

    private static void ValidateLeaseResult(string family, string method, string signature, string type, IReadOnlyDictionary<string, Struct> structs, IReadOnlyDictionary<string, Enum> enums)
    {
        Struct value = structs[type];
        if (type == "NativeByteLease")
        {
            if (value.Fields.Count != 3 || Bare(value.Fields[0].Type) != "NativeByteLeaseHandle" || value.Fields[1].Name != "bytes" || !value.Fields[1].Type.Contains('*', StringComparison.Ordinal) || value.Fields[2].Name != "len" || Bare(value.Fields[2].Type) != "size_t")
                Fail(family, method, signature, "NativeByteLease must contain its typed handle plus bytes/len");
            return;
        }
        if (value.Fields.Count(field => field.Name == "handle") != 1 || !value.Fields.Any(field => field.Name == "handle" && Bare(field.Type).EndsWith("LeaseHandle", StringComparison.Ordinal)))
        {
            Fail(family, method, signature, $"lease result {type} must contain exactly one typed handle");
            return;
        }
        Field[] pointerFields = value.Fields.Where(field => field.Type.Contains('*', StringComparison.Ordinal)).ToArray();
        if (pointerFields.Length == 0) { Fail(family, method, signature, $"lease result {type} must contain at least one bounded collection pointer"); return; }
        foreach (Field pointer in pointerFields)
        {
            int pointerIndex = value.Fields.ToList().IndexOf(pointer);
            if (pointerIndex + 1 >= value.Fields.Count || value.Fields[pointerIndex + 1].Name != $"{pointer.Name}_len" || Bare(value.Fields[pointerIndex + 1].Type) != "size_t")
            {
                Fail(family, method, signature, $"lease result {type}.{pointer.Name} requires adjacent _len");
                continue;
            }
            string element = Bare(pointer.Type);
            if (!structs.TryGetValue(element, out Struct? elementValue) || elementValue is null)
            {
                Fail(family, method, signature, $"lease result {type}.{pointer.Name} element {element} is not an emitted struct");
                continue;
            }
            ValidateLeaseElement(family, method, signature, elementValue, structs, enums, new HashSet<string>(StringComparer.Ordinal));
        }
        HashSet<string> collectionFields = pointerFields
            .SelectMany(pointer => new[] { pointer.Name, $"{pointer.Name}_len" })
            .ToHashSet(StringComparer.Ordinal);
        foreach (Field metadata in value.Fields.Where(field => field.Name != "handle" && !collectionFields.Contains(field.Name)))
        {
            ValidateLeaseMetadata(family, method, signature, metadata, structs, enums, new HashSet<string>(StringComparer.Ordinal));
        }
    }

    private static void ValidateLeaseMetadata(string family, string method, string signature, Field field, IReadOnlyDictionary<string, Struct> structs, IReadOnlyDictionary<string, Enum> enums, HashSet<string> seen)
    {
        string type = Bare(field.Type);
        if (field.Type.Contains('*', StringComparison.Ordinal) || type.EndsWith("Handle", StringComparison.Ordinal))
        {
            Fail(family, method, signature, $"lease metadata {field.Name} ({field.Type}) must be a copied fixed value, not borrowed memory or a retained handle");
            return;
        }
        if (IsScalar(type) || enums.ContainsKey(type)) return;
        if (!structs.TryGetValue(type, out Struct? value) || value is null)
        {
            Fail(family, method, signature, $"lease metadata {field.Name} ({field.Type}) is not a supported fixed value");
            return;
        }
        // Immutable lease metadata may carry a borrowed UTF-8 slice only when the
        // generated receipt copies it before the matching lease is released.
        if (type == "NativeUtf8Slice") return;
        if (type.EndsWith("Api", StringComparison.Ordinal) || type is "NativeProductApi" or "NativeProductCreateArgs" or "NativeTurnArgs" or "NativeByteSlice" or "NativeWritableByteSlice" or "NativeStructuredValue")
        {
            Fail(family, method, signature, $"lease metadata {field.Name} ({field.Type}) is not a supported fixed value");
            return;
        }
        if (!seen.Add(type)) return;
        foreach (Field nested in value.Fields) ValidateLeaseMetadata(family, method, signature, nested, structs, enums, seen);
    }

    private static void ValidateLeaseElement(string family, string method, string signature, Struct value, IReadOnlyDictionary<string, Struct> structs, IReadOnlyDictionary<string, Enum> enums, HashSet<string> seen)
    {
        if (!seen.Add(value.Name)) return;
        foreach (Field field in value.Fields)
        {
            string nested = Bare(field.Type);
            if (nested is "NativeUtf8Slice" or "NativeByteSlice") continue;
            if (field.Type.Contains('*', StringComparison.Ordinal)) { Fail(family, method, signature, $"lease element {value.Name}.{field.Name} has unsupported pointer {field.Type}"); continue; }
            if (structs.TryGetValue(nested, out Struct? nestedValue) && nestedValue is not null)
            {
                ValidateLeaseElement(family, method, signature, nestedValue, structs, enums, seen);
            }
            else
            {
                ValidateFixedType(family, method, signature, nested, structs, enums, seen, $"lease element {value.Name}.{field.Name}");
            }
        }
    }

    private static void ValidateBorrowedRequest(string family, string method, string signature, Struct request, IReadOnlyDictionary<string, Struct> structs, IReadOnlyDictionary<string, Enum> enums, HashSet<string> seen)
    {
        if (request.Name.EndsWith("Api", StringComparison.Ordinal) || request.Name is "NativeProductApi" or "NativeProductCreateArgs" or "NativeTurnArgs") Fail(family, method, signature, $"borrowed request {request.Name} is an API/product table");
        if (!seen.Add(request.Name)) return;
        for (int index = 0; index < request.Fields.Count; index++)
        {
            Field field = request.Fields[index];
            if (field.Type.Contains('*', StringComparison.Ordinal))
            {
                string pointed = Bare(field.Type);
                if (!IsExactBorrowedPointer(field.Type)) Fail(family, method, signature, $"borrowed request {request.Name}.{field.Name} ({field.Type}) must be exactly const T *");
                if (index + 1 >= request.Fields.Count || (request.Fields[index + 1].Name != $"{field.Name}_len" && request.Fields[index + 1].Name != $"{field.Name}_count") || Bare(request.Fields[index + 1].Type) != "size_t") Fail(family, method, signature, $"borrowed request {request.Name}.{field.Name} ({field.Type}) lacks an adjacent size_t _len/_count field");
                ValidateBorrowedSpanElement(family, method, signature, pointed, structs, enums);
                index++;
                continue;
            }
            string nested = Bare(field.Type);
            if (nested is "NativeUtf8Slice" or "NativeByteSlice" or "NativeWritableByteSlice") continue;
            if (nested == "NativeStructuredValue") continue;
            ValidateFixedType(family, method, signature, nested, structs, enums, seen, $"borrowed request field {request.Name}.{field.Name}");
        }
    }

    /// A borrowed request span is synchronous input only. Its element may be a
    /// normal fixed value, or one shallow fixed struct with direct UTF-8/byte
    /// slices. The latter is marshalled by the generated call body into
    /// temporary backing storage; recursive graphs and further pointers stay
    /// outside the ABI grammar.
    private static void ValidateBorrowedSpanElement(string family, string method, string signature, string type, IReadOnlyDictionary<string, Struct> structs, IReadOnlyDictionary<string, Enum> enums)
    {
        if (IsScalar(type) || enums.ContainsKey(type)) return;
        if (!structs.TryGetValue(type, out Struct? value) || value is null)
        {
            Fail(family, method, signature, $"borrowed span element {type} is not a supported scalar or emitted native struct");
            return;
        }
        if (type.EndsWith("Api", StringComparison.Ordinal) || type is "NativeProductApi" or "NativeProductCreateArgs" or "NativeTurnArgs" or "NativeUtf8Slice" or "NativeByteSlice" or "NativeWritableByteSlice" or "NativeStructuredValue")
        {
            Fail(family, method, signature, $"borrowed span element {type} must be a fixed value or a shallow emitted struct");
            return;
        }
        foreach (Field field in value.Fields)
        {
            if (field.Type.Contains('*', StringComparison.Ordinal))
            {
                Fail(family, method, signature, $"borrowed span element {type}.{field.Name} ({field.Type}) cannot contain a pointer");
                continue;
            }
            string nested = Bare(field.Type);
            if (nested is "NativeUtf8Slice" or "NativeByteSlice") continue;
            if (nested is "NativeWritableByteSlice" or "NativeStructuredValue")
            {
                Fail(family, method, signature, $"borrowed span element {type}.{field.Name} ({field.Type}) is not a supported immediate field");
                continue;
            }
            ValidateFixedType(family, method, signature, nested, structs, enums, new HashSet<string>(StringComparer.Ordinal), $"borrowed span element {type}.{field.Name}");
        }
    }

    private static bool IsExactOutPointer(string type) => !type.StartsWith("const ", StringComparison.Ordinal) && PointerLevel(type) == 1 && Bare(type) != "void";
    private static bool IsExactBorrowedPointer(string type) => type.StartsWith("const ", StringComparison.Ordinal) && PointerLevel(type) == 1 && Bare(type) != "void";
    private static int PointerLevel(string type) => type.Count(character => character == '*');
    private static void Fail(string family, string method, string signature, string detail) => throw new InvalidOperationException($"unsupported {family}.{method} signature {signature}: {detail}.");

    private static unsafe List<CXCursor> Children(CXCursor cursor)
    {
        sChildren = [];
        clang.visitChildren(cursor, &CollectChild, null);
        return sChildren;
    }
    private static List<CXCursor> sChildren = [];
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static unsafe CXChildVisitResult CollectChild(CXCursor cursor, CXCursor parent, void* clientData) { sChildren.Add(cursor); return CXChildVisitResult.CXChildVisit_Continue; }
    public static string Bare(string type) => type.Replace("const ", "", StringComparison.Ordinal).Replace("struct ", "", StringComparison.Ordinal).Replace("enum ", "", StringComparison.Ordinal).Replace(" *", "", StringComparison.Ordinal).Trim();
    public static bool IsScalar(string type) => type is "void" or "bool" or "_Bool" or "int" or "int16_t" or "int32_t" or "int64_t" or "uint16_t" or "uint32_t" or "uint64_t" or "size_t" or "float" or "double" or "uint8_t";
}

internal static class Emit
{
    public static string Contracts(BindingModel model)
    {
        StringBuilder output = Header("safe contracts");
        output.AppendLine("namespace Rusty.Engine;").AppendLine();
        output.AppendLine("public interface IEngineContext").AppendLine("{");
        foreach (Service service in model.Services) output.AppendLine($"    I{SafeServiceName(service.Name)}Service {SafeServiceName(service.Name)} {{ get; }}");
        output.AppendLine("}").AppendLine();
        foreach (Service service in model.Services)
        {
            output.AppendLine($"public interface I{SafeServiceName(service.Name)}Service").AppendLine("{");
            foreach ((string name, string callbackName) in service.Operations)
            {
                Callback callback = model.Callbacks[callbackName];
                if (IsDestroy(callback)) continue;
                output.AppendLine($"    {SafeReturn(model, callback)} {Pascal(name)}({SafeParameters(model, callback)});");
            }
            output.AppendLine("}").AppendLine();
        }
        output.AppendLine("public sealed class EngineCallException : Exception").AppendLine("{");
        output.AppendLine("    public EngineCallException(string service, string operation, int status) : this(service, operation, status, ReadOnlyMemory<EngineDiagnostic>.Empty) { }");
        output.AppendLine("    public EngineCallException(string service, string operation, int status, ReadOnlyMemory<EngineDiagnostic> diagnostics) : base($\"Rusty Engine {service}.{operation} returned status {status}.\") { Service = service; Operation = operation; Status = status; Diagnostics = diagnostics; }");
        output.AppendLine("    public string Service { get; }").AppendLine("    public string Operation { get; }").AppendLine("    public int Status { get; }").AppendLine("    public ReadOnlyMemory<EngineDiagnostic> Diagnostics { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public readonly ref struct ProductUpdate").AppendLine("{");
        output.AppendLine("    public ProductUpdate(ProductUpdateFacts facts, ReadOnlySpan<ProductInputEvent> input) { Facts = facts; Input = input; }");
        output.AppendLine("    public ProductUpdateFacts Facts { get; }");
        output.AppendLine("    public ReadOnlySpan<ProductInputEvent> Input { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public sealed class ProductCreateContext").AppendLine("{");
        output.AppendLine("    public ProductCreateContext(IEngineContext engine, ProductContent content, ProductInputConfiguration input) { Engine = engine ?? throw new ArgumentNullException(nameof(engine)); Content = content ?? throw new ArgumentNullException(nameof(content)); Input = input ?? throw new ArgumentNullException(nameof(input)); }");
        output.AppendLine("    public IEngineContext Engine { get; }").AppendLine("    public ProductContent Content { get; }").AppendLine("    public ProductInputConfiguration Input { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public sealed class ProductContent").AppendLine("{");
        output.AppendLine("    public ProductContent(ReadOnlyMemory<ProductContentFile> files) => Files = files;");
        output.AppendLine("    public ReadOnlyMemory<ProductContentFile> Files { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public readonly record struct ProductContentFile(ReadOnlyMemory<byte> Path, ReadOnlyMemory<byte> Bytes);");
        output.AppendLine("public readonly record struct InputBinding(ulong InstanceId, ulong Generation, ulong ControlRevision);");
        output.AppendLine("public readonly record struct InputContext(ReadOnlyMemory<byte> Value);");
        output.AppendLine("public readonly record struct InputSequence(ulong Value);");
        output.AppendLine("public readonly record struct ProductInputDescriptor(ReadOnlyMemory<byte> Id, InputValueKind ValueKind, ReadOnlyMemory<byte> PayloadContract);");
        output.AppendLine("public readonly record struct ProductInputMapping(ReadOnlyMemory<byte> Id, ReadOnlyMemory<byte> Intent, InputTriggerKind TriggerKind, InputEdge Edge, InputAxis Axis, KeyboardControl Keyboard, PointerButton PointerButton, ControllerButton ControllerButton, ControllerAxis ControllerAxis, ReadOnlyMemory<KeyboardControl> Chord, InputContext Context);");
        output.AppendLine("public sealed class ProductInputConfiguration").AppendLine("{");
        output.AppendLine("    public ProductInputConfiguration(InputBinding binding, InputContext context, ReadOnlyMemory<ProductInputDescriptor> directIntents, ReadOnlyMemory<ProductInputMapping> physicalMappings) { Binding = binding; Context = context; DirectIntents = directIntents; PhysicalMappings = physicalMappings; }");
        output.AppendLine("    public InputBinding Binding { get; }").AppendLine("    public InputContext Context { get; }").AppendLine("    public ReadOnlyMemory<ProductInputDescriptor> DirectIntents { get; }").AppendLine("    public ReadOnlyMemory<ProductInputMapping> PhysicalMappings { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public readonly record struct ProductInputEvent(InputEventKind Kind, InputEdge Edge, InputDevice Device, InputChannel Channel, InputAxis Axis, KeyboardControl Keyboard, PointerButton PointerButton, ControllerButton ControllerButton, ControllerAxis ControllerAxis, InputClearReason ClearReason, InputValueKind ValueKind, InputPhase Phase, InputProvenance Provenance, InputBinding Binding, InputSequence Sequence, InputContext Context, float X, float Y, ReadOnlyMemory<byte> Label, ReadOnlyMemory<byte> MappingId, ReadOnlyMemory<byte> Intent, ReadOnlyMemory<byte> PayloadContract, ReadOnlyMemory<byte> PayloadData);");
        output.AppendLine();
        output.AppendLine("[AttributeUsage(AttributeTargets.Assembly, AllowMultiple = false)]");
        output.AppendLine("public sealed class EngineProductAttribute : Attribute").AppendLine("{");
        output.AppendLine("    public EngineProductAttribute(Type productType) => ProductType = productType ?? throw new ArgumentNullException(nameof(productType));");
        output.AppendLine("    public Type ProductType { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public interface IEngineProduct : IDisposable").AppendLine("{");
        output.AppendLine("    void Start();");
        output.AppendLine("    void Update(ProductUpdate update);");
        output.AppendLine("    void Pause();");
        output.AppendLine("    void Resume();");
        output.AppendLine("    void Shutdown();");
        output.AppendLine("}");
        return output.ToString();
    }

    public static string Values(BindingModel model)
    {
        StringBuilder output = Header("safe values");
        output.AppendLine("namespace Rusty.Engine;").AppendLine();
        foreach (Enum value in model.Enums.Values.OrderBy(value => value.Name, StringComparer.Ordinal))
        {
            output.AppendLine($"public enum {SafeType(value.Name)} : uint").AppendLine("{");
            foreach (EnumMember member in value.Members) output.AppendLine($"    {SafeEnumMember(value.Name, member.Name)} = {member.Value},");
            output.AppendLine("}").AppendLine();
        }
        foreach (Struct value in model.Structs.Values.Where(value => IsSafeValue(value, model)).OrderBy(value => value.Name, StringComparer.Ordinal))
        {
            string safeName = SafeType(value.Name);
            IReadOnlyList<(Field Field, string Type)> fields = SafeFields(value, model);
            if (value.Name.EndsWith("Handle", StringComparison.Ordinal))
            {
                output.AppendLine($"public readonly record struct {safeName}(ulong Value);").AppendLine();
            }
            else
            {
                output.Append($"public readonly record struct {safeName}(").Append(string.Join(", ", fields.Select(field => $"{field.Type} {Pascal(field.Field.Name)}"))).AppendLine(");").AppendLine();
            }
        }
        foreach (Struct lease in model.Structs.Values.Where(lease => UsesLeaseReceipt(model, lease)).OrderBy(value => value.Name, StringComparer.Ordinal))
        {
            string collections = string.Join(", ", LeasePointers(lease).Select(pointer => $"ReadOnlyMemory<{SafeType(model, BindingModel.Bare(pointer.Type))}> {Pascal(pointer.Name)}"));
            string metadata = string.Join(", ", LeaseMetadataFields(lease).Select(field => $"{SafeLeaseMetadataType(model, field)} {Pascal(field.Name)}"));
            output.Append($"public readonly record struct {LeaseReceiptType(lease)}(").Append(collections);
            if (metadata.Length > 0) output.Append(", ").Append(metadata);
            output.AppendLine(");").AppendLine();
        }
        foreach (string handle in DisposableHandleTypes(model))
        {
            string owner = SafeType(handle).Replace("Handle", "", StringComparison.Ordinal);
            output.AppendLine($"public sealed class {owner} : IDisposable").AppendLine("{");
            string handleType = SafeType(handle);
            output.AppendLine($"    public {owner}({handleType} handle, Action dispose) {{ Handle = handle; _dispose = dispose ?? throw new ArgumentNullException(nameof(dispose)); }}");
            output.AppendLine($"    public {handleType} Handle {{ get; }}").AppendLine("    private readonly object _disposeGate = new();").AppendLine("    private Action? _dispose;").AppendLine("    public void Dispose() { lock (_disposeGate) { Action? dispose = _dispose; if (dispose is null) return; dispose(); _dispose = null; } }").AppendLine("}").AppendLine();
        }
        output.AppendLine("public sealed class UiValue").AppendLine("{");
        output.AppendLine("    public UiValue(ReadOnlyMemory<StructuredValueNode> nodes, ReadOnlyMemory<uint> edges, uint root, ReadOnlyMemory<byte> utf8) { Nodes = nodes; Edges = edges; Root = root; Utf8 = utf8; }");
        output.AppendLine("    public ReadOnlyMemory<StructuredValueNode> Nodes { get; }").AppendLine("    public ReadOnlyMemory<uint> Edges { get; }").AppendLine("    public uint Root { get; }").AppendLine("    public ReadOnlyMemory<byte> Utf8 { get; }").AppendLine("}");
        return output.ToString();
    }

    public static string Interop(BindingModel model)
    {
        StringBuilder output = Header("internal NativeProduct ABI input");
        output.AppendLine("using System.Runtime.InteropServices;").AppendLine("namespace Rusty.Engine.NativeProduct;").AppendLine();
        foreach (Enum value in model.Enums.Values.OrderBy(value => value.Name, StringComparer.Ordinal))
        {
            output.AppendLine($"internal enum {value.Name} : uint").AppendLine("{");
            foreach (EnumMember member in value.Members) output.AppendLine($"    {RawIdentifier(member.Name)} = {member.Value},");
            output.AppendLine("}").AppendLine();
        }
        foreach (Struct value in model.Structs.Values.OrderBy(value => value.Name, StringComparer.Ordinal))
        {
            output.AppendLine("[StructLayout(LayoutKind.Sequential)]").AppendLine($"internal unsafe struct {value.Name}").AppendLine("{");
            foreach (Field field in value.Fields) output.AppendLine($"    {RawFieldDeclaration(field)}");
            output.AppendLine("}").AppendLine();
        }
        foreach (Callback callback in model.Callbacks.Values.OrderBy(value => value.Name, StringComparer.Ordinal)) output.AppendLine($"internal unsafe struct {callback.Name} {{ internal delegate* unmanaged[Cdecl]<{string.Join(", ", callback.Parameters.Select(RawType))}, {RawType(callback.ReturnType)}> Pointer; }}");
        return output.ToString();
    }

    public static string Implementations(BindingModel model)
    {
        StringBuilder output = Header("internal NativeProduct service implementation input");
        output.AppendLine("using System.Buffers;").AppendLine("using System.Linq;").AppendLine("using System.Text;").AppendLine("using Rusty.Engine;").AppendLine("namespace Rusty.Engine.NativeProduct;").AppendLine();
        output.AppendLine("// Injected into the NativeProduct compilation. Public Rusty.Engine has contracts and values only.");
        output.AppendLine("internal static unsafe class NativeCall").AppendLine("{");
        output.AppendLine("    internal static void Require(string service, string operation, int status) { if (status != 1) throw new EngineCallException(service, operation, status); }");
        output.AppendLine("    internal static void Require(string service, string operation, int status, NativeOperationErrorReceipt error, void* context, delegate* unmanaged[Cdecl]<void*, NativeEngineDiagnosticLeaseHandle, int> destroy) { if (status == 1) { if (error.diagnostics.handle.value == 0) return; int disposeStatus = destroy(context, error.diagnostics.handle); Require(service, \"DestroyOperationDiagnosticLease\", disposeStatus); throw new InvalidOperationException($\"Rusty Engine {service}.{operation} returned success with an operation diagnostic lease.\"); } ReadOnlyMemory<EngineDiagnostic> diagnostics = ReadOnlyMemory<EngineDiagnostic>.Empty; string receiptService = service; string receiptOperation = operation; int receiptStatus = error.status == 0 ? status : error.status; try { if (error.diagnostics.handle.value != 0) diagnostics = NativeConversions.CopyLease(error.diagnostics); if (error.service.len != 0) receiptService = NativeConversions.CopyUtf8(error.service); if (error.operation.len != 0) receiptOperation = NativeConversions.CopyUtf8(error.operation); } finally { if (error.diagnostics.handle.value != 0) { int disposeStatus = destroy(context, error.diagnostics.handle); Require(service, \"DestroyOperationDiagnosticLease\", disposeStatus); } } throw new EngineCallException(receiptService, receiptOperation, receiptStatus, diagnostics); }");
        output.AppendLine("}").AppendLine();
        EmitConversions(output, model);
        foreach (Service service in model.Services)
        {
            output.AppendLine($"internal unsafe sealed class {service.Name}ServiceImplementation : I{SafeServiceName(service.Name)}Service").AppendLine("{");
            output.AppendLine($"    private readonly Native{service.Name}Api _native;").AppendLine($"    internal {service.Name}ServiceImplementation(Native{service.Name}Api native) => _native = native;");
            foreach ((string name, string callbackName) in service.Operations)
            {
                Callback callback = model.Callbacks[callbackName];
                if (IsDestroy(callback)) continue;
                output.Append(EmitServiceMethod(model, service, name, callback));
            }
            output.AppendLine("}").AppendLine();
        }
        return output.ToString();
    }

    private static void EmitConversions(StringBuilder output, BindingModel model)
    {
        output.AppendLine("internal static unsafe class NativeConversions").AppendLine("{");
        output.AppendLine("    private const nuint MaxOwnedLeaseBytes = 256u * 1024u * 1024u;");
        output.AppendLine("    private const nuint MaxOwnedLeaseItems = 1_000_000u;");
        output.AppendLine("    private static readonly UTF8Encoding StrictUtf8 = new(false, true);");
        output.AppendLine("    internal static string CopyUtf8(NativeUtf8Slice value) { if (value.len > MaxOwnedLeaseBytes) throw new InvalidOperationException(\"Native UTF-8 lease exceeded the supported copy bound.\"); if (value.len == 0) return string.Empty; if (value.bytes is null) throw new InvalidOperationException(\"Native UTF-8 lease had length without bytes.\"); return StrictUtf8.GetString(new ReadOnlySpan<byte>(value.bytes, checked((int)value.len))); }");
        output.AppendLine("    private static ReadOnlyMemory<byte> CopyBytes(NativeByteSlice value) { if (value.len > MaxOwnedLeaseBytes) throw new InvalidOperationException(\"Native byte lease exceeded the supported copy bound.\"); if (value.len == 0) return ReadOnlyMemory<byte>.Empty; if (value.bytes is null) throw new InvalidOperationException(\"Native byte lease had length without bytes.\"); byte[] copy = new byte[checked((int)value.len)]; new ReadOnlySpan<byte>(value.bytes, copy.Length).CopyTo(copy); return copy; }");
        output.AppendLine("    internal static NativeVec2 ToNative(Vector2 value) => new() { x = value.X, y = value.Y };");
        output.AppendLine("    internal static Vector2 FromNative(NativeVec2 value) => new(value.x, value.y);");
        output.AppendLine("    internal static NativeVec3 ToNative(Vector3 value) => new() { x = value.X, y = value.Y, z = value.Z };");
        output.AppendLine("    internal static Vector3 FromNative(NativeVec3 value) => new(value.x, value.y, value.z);");
        output.AppendLine("    internal static NativeQuat ToNative(Quaternion value) => new() { x = value.X, y = value.Y, z = value.Z, w = value.W };");
        output.AppendLine("    internal static Quaternion FromNative(NativeQuat value) => new(value.x, value.y, value.z, value.w);");
        foreach (Enum value in model.Enums.Values.OrderBy(value => value.Name, StringComparer.Ordinal))
        {
            string safe = SafeType(value.Name);
            output.AppendLine($"    internal static {value.Name} ToNative({safe} value) => ({value.Name})(uint)value;");
            output.AppendLine($"    internal static {safe} FromNative({value.Name} value) => ({safe})(uint)value;");
        }
        foreach (Struct value in model.Structs.Values.Where(value => IsSafeValue(value, model)).OrderBy(value => value.Name, StringComparer.Ordinal))
        {
            string safe = SafeType(value.Name);
            if (value.Name.EndsWith("Handle", StringComparison.Ordinal))
            {
                output.AppendLine($"    internal static {value.Name} ToNative({safe} value) => new() {{ value = value.Value }};");
                output.AppendLine($"    internal static {safe} FromNative({value.Name} value) => new(value.value);");
                if (IsDisposableHandle(model, value.Name))
                {
                    output.AppendLine($"    internal static {value.Name} ToNative({OwnerType(value.Name)} value) => ToNative(value.Handle);");
                }
                continue;
            }
            if (HasBorrowedFields(value)) continue;
            string assignments = string.Join(", ", value.Fields.Select(field => $"{RawIdentifier(field.Name)} = {ToNativeExpression(field, $"value.{Pascal(field.Name)}")}"));
            string arguments = string.Join(", ", value.Fields.Select(field => FromNativeExpression(field, $"value.{RawIdentifier(field.Name)}")));
            output.AppendLine($"    internal static {value.Name} ToNative({safe} value) => new() {{ {assignments} }};");
            if (!HasDisposableHandleField(model, value)) output.AppendLine($"    internal static {safe} FromNative({value.Name} value) => new({arguments});");
        }
        foreach (Struct value in LeaseMetadataStructures(model))
        {
            string safe = SafeType(value.Name);
            string arguments = string.Join(", ", value.Fields.Select(field => LeaseMetadataFromNativeExpression(model, field, $"value.{RawIdentifier(field.Name)}")));
            output.AppendLine($"    private static {safe} CopyLeaseMetadata({value.Name} value) => new({arguments});");
        }
        HashSet<string> emittedLeaseElements = new(StringComparer.Ordinal);
        foreach (Struct lease in model.Structs.Values.Where(value => BindingModel.IsLeaseResult(value.Name, model.Structs)).OrderBy(value => value.Name, StringComparer.Ordinal))
        {
            if (lease.Name == "NativeByteLease")
            {
                output.AppendLine("    internal static ReadOnlyMemory<byte> CopyLease(NativeByteLease value) => CopyBytes(new NativeByteSlice { bytes = value.bytes, len = value.len });");
                continue;
            }
            Field[] pointers = LeasePointers(lease).ToArray();
            foreach (Field pointer in pointers)
            {
                string element = BindingModel.Bare(pointer.Type);
                Struct elementValue = model.Structs[element];
                string safeElement = SafeType(model, element);
                string copyMethod = pointers.Length == 1 ? "CopyLease" : $"CopyLease{Pascal(pointer.Name)}";
                output.AppendLine($"    internal static ReadOnlyMemory<{safeElement}> {copyMethod}({lease.Name} value)").AppendLine("    {");
                output.AppendLine($"        if (value.{RawIdentifier($"{pointer.Name}_len")} > MaxOwnedLeaseItems) throw new InvalidOperationException(\"Native collection lease exceeded the supported item bound.\");");
                output.AppendLine($"        if (value.{RawIdentifier($"{pointer.Name}_len")} == 0) return ReadOnlyMemory<{safeElement}>.Empty;");
                output.AppendLine($"        if (value.{RawIdentifier(pointer.Name)} is null) throw new InvalidOperationException(\"Native collection lease had count without elements.\");");
                output.AppendLine($"        int count = checked((int)value.{RawIdentifier($"{pointer.Name}_len")});");
                output.AppendLine($"        {safeElement}[] copy = new {safeElement}[count];");
                output.AppendLine($"        for (int index = 0; index < count; index++) copy[index] = CopyLeaseElement(value.{RawIdentifier(pointer.Name)}[index]);");
                output.AppendLine("        return copy;").AppendLine("    }");
                if (emittedLeaseElements.Add(element))
                {
                    string arguments = string.Join(", ", elementValue.Fields.Select(field => LeaseElementFromNativeExpression(model, field, $"value.{RawIdentifier(field.Name)}")));
                    output.AppendLine($"    private static {safeElement} CopyLeaseElement({element} value) => new({arguments});");
                }
            }
            if (UsesLeaseReceipt(model, lease))
            {
                string collections = string.Join(", ", pointers.Select(pointer => $"{(pointers.Length == 1 ? "CopyLease" : $"CopyLease{Pascal(pointer.Name)}")}(value)"));
                string metadata = string.Join(", ", LeaseMetadataFields(lease).Select(field => LeaseMetadataFromNativeExpression(model, field, $"value.{RawIdentifier(field.Name)}")));
                output.Append($"    internal static {LeaseReceiptType(lease)} CopyLeaseReceipt({lease.Name} value) => new(").Append(collections);
                if (metadata.Length > 0) output.Append(", ").Append(metadata);
                output.AppendLine(");");
            }
        }
        output.AppendLine("    internal static byte ToNativeBool(bool value) => value ? (byte)1 : (byte)0;");
        output.AppendLine("    internal static int ToNative(int value) => value;");
        output.AppendLine("    internal static short ToNative(short value) => value;");
        output.AppendLine("    internal static long ToNative(long value) => value;");
        output.AppendLine("    internal static uint ToNative(uint value) => value;");
        output.AppendLine("    internal static ushort ToNative(ushort value) => value;");
        output.AppendLine("    internal static ulong ToNative(ulong value) => value;");
        output.AppendLine("    internal static nuint ToNative(nuint value) => value;");
        output.AppendLine("    internal static float ToNative(float value) => value;");
        output.AppendLine("    internal static double ToNative(double value) => value;");
        output.AppendLine("    internal static byte ToNative(byte value) => value;");
        output.AppendLine("    internal static bool FromNativeBool(byte value) => value != 0;");
        output.AppendLine("    internal static int FromNative(int value) => value;");
        output.AppendLine("    internal static short FromNative(short value) => value;");
        output.AppendLine("    internal static long FromNative(long value) => value;");
        output.AppendLine("    internal static uint FromNative(uint value) => value;");
        output.AppendLine("    internal static ushort FromNative(ushort value) => value;");
        output.AppendLine("    internal static ulong FromNative(ulong value) => value;");
        output.AppendLine("    internal static nuint FromNative(nuint value) => value;");
        output.AppendLine("    internal static float FromNative(float value) => value;");
        output.AppendLine("    internal static double FromNative(double value) => value;");
        output.AppendLine("    internal static byte FromNative(byte value) => value;");
        output.AppendLine("}").AppendLine();
    }

    private static string EmitServiceMethod(BindingModel model, Service service, string operation, Callback callback)
    {
        string returnType = SafeReturn(model, callback);
        string signature = SafeParameters(model, callback);
        string[] input = Inputs(callback);
        string result = ResultParameter(callback) ?? string.Empty;
        if (IsSpanCall(input)) return EmitSpanMethod(model, service, operation, callback, returnType, signature);
        if (input.Length >= 1 && input[^1].StartsWith("const ", StringComparison.Ordinal) && input[^1].Contains('*', StringComparison.Ordinal)) return EmitBorrowedRequestMethod(model, service, operation, callback, returnType, signature, BindingModel.Bare(input[^1]), result, input[..^1]);
        return EmitDirectMethod(model, service, operation, callback, returnType, signature, input, result);
    }

    private static string EmitDirectMethod(BindingModel model, Service service, string operation, Callback callback, string returnType, string signature, string[] input, string result)
    {
        StringBuilder output = new();
        output.AppendLine($"    public {returnType} {Pascal(operation)}({signature})").AppendLine("    {");
        for (int index = 0; index < input.Length; index++) output.AppendLine($"        {RawType(input[index])} raw{index} = NativeConversions.ToNative(arg{index});");
        if (!string.IsNullOrEmpty(result)) output.AppendLine($"        {RawType(result)} rawResult = default;");
        bool hasErrorReadout = BindingModel.HasOperationErrorReceipt(callback.Parameters.Skip(1).ToArray());
        if (hasErrorReadout) output.AppendLine("        NativeOperationErrorReceipt rawError = default;");
        string invocation = string.Join(", ", new[] { "_native.context" }.Concat(input.Select((_, index) => $"raw{index}")).Concat(string.IsNullOrEmpty(result) ? [] : ["&rawResult"]).Concat(hasErrorReadout ? ["&rawError"] : []));
        output.AppendLine($"        int status = _native.{RawIdentifier(operation)}.Pointer({invocation});");
        EmitRequire(output, model, service, operation, hasErrorReadout, "        ");
        if (string.IsNullOrEmpty(result)) output.AppendLine("        return;");
        else if (BindingModel.IsLeaseResult(BindingModel.Bare(result), model.Structs))
        {
            (_, string destroyOperation) = DestroyLeaseFor(model, service, BindingModel.Bare(result));
            output.AppendLine($"        {RawType(result)} ownedResult = rawResult;");
            output.AppendLine($"        try {{ return NativeConversions.{(UsesLeaseReceipt(model, model.Structs[BindingModel.Bare(result)]) ? "CopyLeaseReceipt" : "CopyLease")}(ownedResult); }}");
            output.AppendLine($"        finally {{ int disposeStatus = _native.{RawIdentifier(destroyOperation)}.Pointer(_native.context, ownedResult.handle); NativeCall.Require(\"{SafeServiceName(service.Name)}\", \"{Pascal(destroyOperation)}\", disposeStatus); }}");
        }
        else if (returnType != SafeType(BindingModel.Bare(result)))
        {
            (_, string destroyOperation) = DestroyFor(model, BindingModel.Bare(result));
            output.AppendLine($"        {RawType(result)} ownedResult = rawResult;");
            output.AppendLine($"        return new {returnType}(NativeConversions.FromNative(ownedResult), () => {{ int disposeStatus = _native.{RawIdentifier(destroyOperation)}.Pointer(_native.context, ownedResult); NativeCall.Require(\"{SafeServiceName(service.Name)}\", \"{Pascal(destroyOperation)}\", disposeStatus); }});");
        }
        else output.AppendLine("        return NativeConversions.FromNative(rawResult);");
        output.AppendLine("    }").AppendLine();
        return output.ToString();
    }

    private static string EmitSpanMethod(BindingModel model, Service service, string operation, Callback callback, string returnType, string signature)
    {
        string item = BindingModel.Bare(Inputs(callback)[0]);
        StringBuilder output = new();
        output.AppendLine($"    public {returnType} {Pascal(operation)}({signature})").AppendLine("    {");
        output.AppendLine($"        {RawType(item)}[] rawValues = values.ToArray().Select(NativeConversions.ToNative).ToArray();");
        output.AppendLine($"        fixed ({RawType(item)}* pointer = rawValues)").AppendLine("        {");
        bool hasErrorReadout = BindingModel.HasOperationErrorReceipt(callback.Parameters.Skip(1).ToArray());
        if (hasErrorReadout) output.AppendLine("            NativeOperationErrorReceipt rawError = default;");
        output.AppendLine($"            int status = _native.{RawIdentifier(operation)}.Pointer(_native.context, rawValues.Length == 0 ? null : pointer, (nuint)rawValues.Length{(hasErrorReadout ? ", &rawError" : string.Empty)});");
        EmitRequire(output, model, service, operation, hasErrorReadout, "            ");
        output.AppendLine("        }").AppendLine("    }").AppendLine();
        return output.ToString();
    }

    private static string EmitBorrowedRequestMethod(BindingModel model, Service service, string operation, Callback callback, string returnType, string signature, string requestName, string result, string[] leading)
    {
        Struct request = model.Structs[requestName];
        StringBuilder output = new();
        output.AppendLine($"    public {returnType} {Pascal(operation)}({signature})").AppendLine("    {");
        for (int index = 0; index < leading.Length; index++) output.AppendLine($"        {RawType(leading[index])} raw{index} = NativeConversions.ToNative(arg{index});");
        int requestIndex = leading.Length;
        string requestArgument = $"arg{requestIndex}";
        List<string> closers = [];
        Field[] specialSpanFields = request.Fields.Where(field => field.Type.Contains('*', StringComparison.Ordinal) && BorrowedSpanElementHasImmediateFields(model, BindingModel.Bare(field.Type))).ToArray();
        List<string> temporaryPinArrays = specialSpanFields.SelectMany(field => BorrowedSpanElementPinNames(model, field)).ToList();
        bool hasSpecialSpanElements = temporaryPinArrays.Count > 0;
        if (hasSpecialSpanElements)
        {
            foreach (string pins in temporaryPinArrays) output.AppendLine($"        MemoryHandle[] {pins} = [];");
            output.AppendLine("        try").AppendLine("        {");
        }
        foreach (Field field in request.Fields)
        {
            if (BindingModel.Bare(field.Type) == "NativeUtf8Slice")
            {
                string property = Pascal(field.Name);
                output.AppendLine($"        byte[] {field.Name}Bytes = Encoding.UTF8.GetBytes({requestArgument}.{property} ?? throw new ArgumentNullException(nameof({requestArgument}))); ");
                output.AppendLine($"        fixed (byte* {field.Name}Pointer = {field.Name}Bytes)").AppendLine("        {");
                closers.Add("        }");
            }
            if (BindingModel.Bare(field.Type) is "NativeByteSlice" or "NativeWritableByteSlice")
            {
                string property = Pascal(field.Name);
                output.AppendLine($"        using MemoryHandle {field.Name}Pin = {requestArgument}.{property}.Pin();");
            }
        }
        for (int index = 0; index < request.Fields.Count; index++)
        {
            Field field = request.Fields[index];
            if (!field.Type.Contains('*', StringComparison.Ordinal)) continue;
            if (index + 1 < request.Fields.Count && (request.Fields[index + 1].Name == $"{field.Name}_len" || request.Fields[index + 1].Name == $"{field.Name}_count"))
            {
                string rawElement = RawType(BindingModel.Bare(field.Type));
                string property = Pascal(field.Name);
                if (BorrowedSpanElementHasImmediateFields(model, BindingModel.Bare(field.Type)))
                {
                    EmitBorrowedSpanElementMarshalling(output, model, requestArgument, field);
                }
                else output.AppendLine($"        {rawElement}[] {field.Name}Raw = {requestArgument}.{property}.ToArray().Select(NativeConversions.ToNative).ToArray();");
                output.AppendLine($"        fixed ({rawElement}* {field.Name}Pointer = {field.Name}Raw)").AppendLine("        {");
                closers.Add("        }");
            }
        }
        foreach (Field field in request.Fields.Where(field => BindingModel.Bare(field.Type) == "NativeStructuredValue"))
        {
            string property = Pascal(field.Name);
            output.AppendLine($"        using MemoryHandle {field.Name}NodesPin = {requestArgument}.{property}.Nodes.Pin();");
            output.AppendLine($"        using MemoryHandle {field.Name}EdgesPin = {requestArgument}.{property}.Edges.Pin();");
            output.AppendLine($"        using MemoryHandle {field.Name}Utf8Pin = {requestArgument}.{property}.Utf8.Pin();");
        }
        output.AppendLine($"        {requestName} raw = new() {{");
        for (int index = 0; index < request.Fields.Count; index++)
        {
            Field field = request.Fields[index];
            if (index > 0 && request.Fields[index - 1].Type.Contains('*', StringComparison.Ordinal) && (field.Name == $"{request.Fields[index - 1].Name}_len" || field.Name == $"{request.Fields[index - 1].Name}_count"))
            {
                output.AppendLine($"            {RawIdentifier(field.Name)} = (nuint){requestArgument}.{Pascal(request.Fields[index - 1].Name)}.Length,"); continue;
            }
            string expression = BorrowedFieldExpression(field, requestArgument);
            output.AppendLine($"            {RawIdentifier(field.Name)} = {expression},");
        }
        output.AppendLine("        };");
        if (!string.IsNullOrEmpty(result)) output.AppendLine($"        {RawType(result)} rawResult = default;");
        bool hasErrorReadout = BindingModel.HasOperationErrorReceipt(callback.Parameters.Skip(1).ToArray());
        if (hasErrorReadout) output.AppendLine("        NativeOperationErrorReceipt rawError = default;");
        string leadingInvocation = string.Join(", ", leading.Select((_, index) => $"raw{index}"));
        string invocation = string.IsNullOrEmpty(result) ? $"_native.context{(leadingInvocation.Length == 0 ? string.Empty : ", " + leadingInvocation)}, &raw" : $"_native.context{(leadingInvocation.Length == 0 ? string.Empty : ", " + leadingInvocation)}, &raw, &rawResult";
        output.AppendLine($"        int status = _native.{RawIdentifier(operation)}.Pointer({invocation}{(hasErrorReadout ? ", &rawError" : string.Empty)});");
        EmitRequire(output, model, service, operation, hasErrorReadout, "        ");
        if (string.IsNullOrEmpty(result)) output.AppendLine("        return;");
        else if (BindingModel.IsLeaseResult(BindingModel.Bare(result), model.Structs))
        {
            (_, string destroyOperation) = DestroyLeaseFor(model, service, BindingModel.Bare(result));
            output.AppendLine($"        {RawType(result)} ownedResult = rawResult;");
            output.AppendLine($"        try {{ return NativeConversions.{(UsesLeaseReceipt(model, model.Structs[BindingModel.Bare(result)]) ? "CopyLeaseReceipt" : "CopyLease")}(ownedResult); }}");
            output.AppendLine($"        finally {{ int disposeStatus = _native.{RawIdentifier(destroyOperation)}.Pointer(_native.context, ownedResult.handle); NativeCall.Require(\"{SafeServiceName(service.Name)}\", \"{Pascal(destroyOperation)}\", disposeStatus); }}");
        }
        else if (returnType != SafeType(BindingModel.Bare(result)))
        {
            (_, string destroyOperation) = DestroyFor(model, BindingModel.Bare(result));
            output.AppendLine($"        {RawType(result)} ownedResult = rawResult;");
            output.AppendLine($"        return new {returnType}(NativeConversions.FromNative(ownedResult), () => {{ int disposeStatus = _native.{RawIdentifier(destroyOperation)}.Pointer(_native.context, ownedResult); NativeCall.Require(\"{SafeServiceName(service.Name)}\", \"{Pascal(destroyOperation)}\", disposeStatus); }});");
        }
        else output.AppendLine("        return NativeConversions.FromNative(rawResult);");
        for (int index = closers.Count - 1; index >= 0; index--) output.AppendLine(closers[index]);
        if (hasSpecialSpanElements)
        {
            output.AppendLine("        }").AppendLine("        finally").AppendLine("        {");
            foreach (string pins in temporaryPinArrays) output.AppendLine($"            foreach (MemoryHandle pin in {pins}) pin.Dispose();");
            output.AppendLine("        }");
        }
        output.AppendLine("    }").AppendLine();
        return output.ToString();
    }

    private static bool BorrowedSpanElementHasImmediateFields(BindingModel model, string element) =>
        model.Structs.TryGetValue(element, out Struct? value)
        && value.Fields.Any(field => BindingModel.Bare(field.Type) is "NativeUtf8Slice" or "NativeByteSlice");

    private static IEnumerable<string> BorrowedSpanElementPinNames(BindingModel model, Field spanField)
    {
        Struct value = model.Structs[BindingModel.Bare(spanField.Type)];
        return value.Fields
            .Where(field => BindingModel.Bare(field.Type) is "NativeUtf8Slice" or "NativeByteSlice")
            .Select(field => $"{spanField.Name}{Pascal(field.Name)}Pins");
    }

    private static void EmitBorrowedSpanElementMarshalling(StringBuilder output, BindingModel model, string requestArgument, Field spanField)
    {
        string element = BindingModel.Bare(spanField.Type);
        Struct value = model.Structs[element];
        string property = Pascal(spanField.Name);
        string values = $"{spanField.Name}Values";
        output.AppendLine($"        {SafeType(model, element)}[] {values} = {requestArgument}.{property}.ToArray();");
        foreach (Field field in value.Fields.Where(field => BindingModel.Bare(field.Type) is "NativeUtf8Slice" or "NativeByteSlice"))
        {
            string fieldProperty = Pascal(field.Name);
            string prefix = $"{spanField.Name}{fieldProperty}";
            if (BindingModel.Bare(field.Type) == "NativeUtf8Slice")
            {
                string bytes = $"{prefix}Bytes";
                string pins = $"{prefix}Pins";
                output.AppendLine($"        byte[][] {bytes} = {values}.Select(value => Encoding.UTF8.GetBytes(value.{fieldProperty} ?? throw new ArgumentNullException(nameof({requestArgument})))).ToArray();");
                output.AppendLine($"        {pins} = new MemoryHandle[{bytes}.Length];");
                output.AppendLine($"        for (int index = 0; index < {bytes}.Length; index++) {pins}[index] = {bytes}[index].AsMemory().Pin();");
            }
            else
            {
                string pins = $"{prefix}Pins";
                output.AppendLine($"        {pins} = new MemoryHandle[{values}.Length];");
                output.AppendLine($"        for (int index = 0; index < {values}.Length; index++) {pins}[index] = {values}[index].{fieldProperty}.Pin();");
            }
        }
        output.AppendLine($"        {element}[] {spanField.Name}Raw = new {element}[{values}.Length];");
        output.AppendLine($"        for (int index = 0; index < {values}.Length; index++)").AppendLine("        {");
        output.AppendLine($"            {spanField.Name}Raw[index] = new {element}").AppendLine("            {");
        foreach (Field field in value.Fields)
        {
            string expression = BorrowedSpanElementFieldExpression(field, values, spanField.Name);
            output.AppendLine($"                {RawIdentifier(field.Name)} = {expression},");
        }
        output.AppendLine("            };").AppendLine("        }");
    }

    private static string BorrowedSpanElementFieldExpression(Field field, string values, string spanName)
    {
        string bare = BindingModel.Bare(field.Type);
        string property = Pascal(field.Name);
        string prefix = $"{spanName}{property}";
        if (bare == "NativeUtf8Slice") return $"new NativeUtf8Slice {{ bytes = {prefix}Bytes[index].Length == 0 ? null : (byte*){prefix}Pins[index].Pointer, len = (nuint){prefix}Bytes[index].Length }}";
        if (bare == "NativeByteSlice") return $"new NativeByteSlice {{ bytes = {values}[index].{property}.Length == 0 ? null : (byte*){prefix}Pins[index].Pointer, len = (nuint){values}[index].{property}.Length }}";
        if (bare is "bool" or "_Bool") return $"NativeConversions.ToNativeBool({values}[index].{property})";
        return $"NativeConversions.ToNative({values}[index].{property})";
    }

    private static void EmitRequire(StringBuilder output, BindingModel model, Service service, string operation, bool hasErrorReadout, string indent)
    {
        string safeService = SafeServiceName(service.Name);
        string safeOperation = Pascal(operation);
        if (!hasErrorReadout)
        {
            output.AppendLine($"{indent}NativeCall.Require(\"{safeService}\", \"{safeOperation}\", status);");
            return;
        }
        (_, string destroyOperation) = DestroyLeaseFor(model, service, "NativeEngineDiagnosticLease");
        output.AppendLine($"{indent}NativeCall.Require(\"{safeService}\", \"{safeOperation}\", status, rawError, _native.context, _native.{RawIdentifier(destroyOperation)}.Pointer);");
    }

    private static string BorrowedFieldExpression(Field field, string requestArgument)
    {
        string bare = BindingModel.Bare(field.Type);
        string property = Pascal(field.Name);
        if (bare == "NativeUtf8Slice") return $"new NativeUtf8Slice {{ bytes = {field.Name}Bytes.Length == 0 ? null : {field.Name}Pointer, len = (nuint){field.Name}Bytes.Length }}";
        if (bare == "NativeByteSlice") return $"new NativeByteSlice {{ bytes = {requestArgument}.{property}.Length == 0 ? null : (byte*){field.Name}Pin.Pointer, len = (nuint){requestArgument}.{property}.Length }}";
        if (bare == "NativeWritableByteSlice") return $"new NativeWritableByteSlice {{ bytes = {requestArgument}.{property}.Length == 0 ? null : (byte*){field.Name}Pin.Pointer, len = (nuint){requestArgument}.{property}.Length }}";
        if (bare == "NativeStructuredValue") return $"new NativeStructuredValue {{ nodes = {requestArgument}.{property}.Nodes.Length == 0 ? null : (NativeStructuredValueNode*){field.Name}NodesPin.Pointer, node_count = (nuint){requestArgument}.{property}.Nodes.Length, edges = {requestArgument}.{property}.Edges.Length == 0 ? null : (uint*){field.Name}EdgesPin.Pointer, edge_count = (nuint){requestArgument}.{property}.Edges.Length, root = {requestArgument}.{property}.Root, utf8 = {requestArgument}.{property}.Utf8.Length == 0 ? null : (byte*){field.Name}Utf8Pin.Pointer, utf8_len = (nuint){requestArgument}.{property}.Utf8.Length }}";
        if (field.Type.Contains('*', StringComparison.Ordinal)) return $"{requestArgument}.{property}.Length == 0 ? null : {field.Name}Pointer";
        if (bare is "bool" or "_Bool") return $"NativeConversions.ToNativeBool({requestArgument}.{property})";
        return $"NativeConversions.ToNative({requestArgument}.{property})";
    }

    private static string ToNativeExpression(Field field, string value) => BindingModel.Bare(field.Type) is "bool" or "_Bool" ? $"ToNativeBool({value})" : $"ToNative({value})";
    private static string FromNativeExpression(Field field, string value) => BindingModel.Bare(field.Type) is "bool" or "_Bool" ? $"FromNativeBool({value})" : $"FromNative({value})";
    private static string LeaseElementFromNativeExpression(BindingModel model, Field field, string value) => BindingModel.Bare(field.Type) switch
    {
        "NativeUtf8Slice" => $"CopyUtf8({value})",
        "NativeByteSlice" => $"CopyBytes({value})",
        string type when model.Structs.ContainsKey(type) => $"CopyLeaseMetadata({value})",
        _ => FromNativeExpression(field, value),
    };
    private static string LeaseMetadataFromNativeExpression(BindingModel model, Field field, string value) => BindingModel.Bare(field.Type) switch
    {
        "NativeUtf8Slice" => $"CopyUtf8({value})",
        string type when model.Structs.ContainsKey(type) => $"CopyLeaseMetadata({value})",
        _ => FromNativeExpression(field, value),
    };
    private static string SafeLeaseMetadataType(BindingModel model, Field field) => BindingModel.Bare(field.Type) switch
    {
        "NativeUtf8Slice" => "string",
        _ => SafeType(model, BindingModel.Bare(field.Type)),
    };

    private static IEnumerable<Struct> LeaseMetadataStructures(BindingModel model)
    {
        HashSet<string> names = new(StringComparer.Ordinal);
        void Include(Field field)
        {
            string type = BindingModel.Bare(field.Type);
            if (type is "NativeUtf8Slice" or "NativeByteSlice" || !model.Structs.TryGetValue(type, out Struct? value) || !names.Add(type)) return;
            foreach (Field nested in value.Fields) Include(nested);
        }
        foreach (Struct lease in model.Structs.Values.Where(lease => HasLeaseMetadata(model, lease)))
        {
            foreach (Field field in LeaseMetadataFields(lease)) Include(field);
        }
        foreach (Struct lease in model.Structs.Values.Where(lease => BindingModel.IsLeaseResult(lease.Name, model.Structs)))
        {
            foreach (Field pointer in LeasePointers(lease))
            {
                if (model.Structs.TryGetValue(BindingModel.Bare(pointer.Type), out Struct? element))
                {
                    foreach (Field field in element.Fields) Include(field);
                }
            }
        }
        return names.OrderBy(name => name, StringComparer.Ordinal).Select(name => model.Structs[name]);
    }

    private static string[] Inputs(Callback callback)
    {
        string[] args = ServiceParameters(callback);
        if (args.Length > 0 && !args[^1].StartsWith("const ", StringComparison.Ordinal) && args[^1].Contains('*', StringComparison.Ordinal) && BindingModel.Bare(args[^1]) != "void") return args[..^1];
        return args;
    }
    private static string? ResultParameter(Callback callback)
    {
        string[] args = ServiceParameters(callback);
        string last = args.Last();
        return !last.StartsWith("const ", StringComparison.Ordinal) && last.Contains('*', StringComparison.Ordinal) && BindingModel.Bare(last) != "void" ? BindingModel.Bare(last) : null;
    }
    private static string[] ServiceParameters(Callback callback)
    {
        string[] args = callback.Parameters.Skip(1).ToArray();
        return BindingModel.HasOperationErrorReceipt(args) ? args[..^1] : args;
    }
    private static bool IsSpanCall(string[] input) => input.Length == 2 && input[0].StartsWith("const ", StringComparison.Ordinal) && input[0].Contains('*', StringComparison.Ordinal) && BindingModel.Bare(input[1]) == "size_t";
    private static bool HasBorrowedFields(Struct value) => value.Fields.Any(field => field.Type.Contains('*', StringComparison.Ordinal) || BindingModel.Bare(field.Type) is "NativeUtf8Slice" or "NativeByteSlice" or "NativeWritableByteSlice" or "NativeStructuredValue");
    private static bool HasDisposableHandleField(BindingModel model, Struct value) => value.Fields.Any(field => IsDisposableHandle(model, BindingModel.Bare(field.Type)));
    private static (string Service, string Operation) DestroyFor(BindingModel model, string handle) => model.Services.SelectMany(service => service.Operations.Select(operation => (service, operation))).First(pair => IsDestroy(model.Callbacks[pair.operation.Callback]) && BindingModel.Bare(model.Callbacks[pair.operation.Callback].Parameters[1]) == handle) is var found ? (found.service.Name, found.operation.Name) : throw new InvalidOperationException($"no destroy operation found for {handle}");
    private static (string Service, string Operation) DestroyLeaseFor(BindingModel model, Service service, string lease)
    {
        Struct value = model.Structs[lease];
        Field handle = value.Fields.Single(field => field.Name == "handle");
        string handleType = BindingModel.Bare(handle.Type);
        (string Name, string Callback) operation = service.Operations.First(operation =>
            IsDestroy(model.Callbacks[operation.Callback])
            && BindingModel.Bare(model.Callbacks[operation.Callback].Parameters[1]) == handleType);
        return (service.Name, operation.Name);
    }

    private static bool IsSafeValue(Struct value, BindingModel model) => value.Name is not "NativeEngineApi" and not "NativeProductApi" and not "NativeProductCreateArgs" and not "NativeTurnArgs" and not "NativeContentFile" and not "NativeInputBinding" and not "NativeInputSequence" and not "NativeInputDescriptor" and not "NativeInputMapping" and not "NativeInputConfiguration" and not "NativeInputEvent" and not "NativeUtf8Slice" and not "NativeByteSlice" and not "NativeWritableByteSlice" and not "NativeStructuredValue" and not "NativeOperationErrorReceipt" and not "NativeVec2" and not "NativeVec3" and not "NativeQuat" && !value.Name.EndsWith("Api", StringComparison.Ordinal) && !BindingModel.IsLeaseResult(value.Name, model.Structs) && !LeaseHandleTypes(model).Contains(value.Name, StringComparer.Ordinal);
    private static IReadOnlyList<(Field Field, string Type)> SafeFields(Struct value, BindingModel model)
    {
        List<(Field, string)> fields = [];
        for (int index = 0; index < value.Fields.Count; index++)
        {
            Field field = value.Fields[index];
            if (BindingModel.Bare(field.Type) == "NativeUtf8Slice") { fields.Add((field, "string")); continue; }
            if (BindingModel.Bare(field.Type) == "NativeByteSlice") { fields.Add((field, "ReadOnlyMemory<byte>")); continue; }
            if (BindingModel.Bare(field.Type) == "NativeWritableByteSlice") { fields.Add((field, "Memory<byte>")); continue; }
            if (BindingModel.Bare(field.Type) == "NativeStructuredValue") { fields.Add((field, "UiValue")); continue; }
            if (field.Type.Contains('*', StringComparison.Ordinal))
            {
                if (index + 1 >= value.Fields.Count || !(value.Fields[index + 1].Name == $"{field.Name}_len" || value.Fields[index + 1].Name == $"{field.Name}_count")) throw new InvalidOperationException($"unsupported value field {value.Name}.{field.Name} ({field.Type}): pointer values require an adjacent _len or _count field.");
                fields.Add((field, $"ReadOnlyMemory<{SafeType(model, BindingModel.Bare(field.Type))}>")); index++; continue;
            }
            fields.Add((field, SafeType(model, BindingModel.Bare(field.Type))));
        }
        return fields;
    }
    private static string SafeReturn(BindingModel model, Callback callback)
    {
        string last = ServiceParameters(callback).Last();
        if (last.StartsWith("const ", StringComparison.Ordinal) || !last.Contains('*', StringComparison.Ordinal) || BindingModel.Bare(last) == "void") return "void";
        string handle = BindingModel.Bare(last);
        if (BindingModel.IsLeaseResult(handle, model.Structs))
        {
            if (handle == "NativeByteLease") return "ReadOnlyMemory<byte>";
            Struct lease = model.Structs[handle];
            if (HasLeaseMetadata(model, lease)) return LeaseReceiptType(lease);
            Field pointer = lease.Fields.First(field => field.Type.Contains('*', StringComparison.Ordinal));
            return $"ReadOnlyMemory<{SafeType(model, BindingModel.Bare(pointer.Type))}>";
        }
        return IsDisposableHandle(model, handle) ? OwnerType(handle) : SafeType(handle);
    }
    private static string SafeParameters(BindingModel model, Callback callback)
    {
        string[] args = ServiceParameters(callback);
        if (args.Length > 0 && !args[^1].StartsWith("const ", StringComparison.Ordinal) && args[^1].Contains('*', StringComparison.Ordinal) && BindingModel.Bare(args[^1]) != "void") args = args[..^1];
        if (args.Length == 2 && args[0].StartsWith("const ", StringComparison.Ordinal) && args[0].Contains('*', StringComparison.Ordinal) && BindingModel.Bare(args[1]) == "size_t") return $"ReadOnlySpan<{SafeType(model, BindingModel.Bare(args[0]))}> values";
        return string.Join(", ", args.Select((type, index) => $"{SafeType(model, BindingModel.Bare(type))} arg{index}"));
    }
    private static string SafeType(string native) => native switch { "bool" or "_Bool" => "bool", "int16_t" => "short", "int" or "int32_t" => "int", "int64_t" => "long", "uint16_t" => "ushort", "uint32_t" => "uint", "uint64_t" => "ulong", "size_t" => "nuint", "float" => "float", "double" => "double", "uint8_t" => "byte", "NativeVec2" => "Vector2", "NativeVec3" => "Vector3", "NativeQuat" => "Quaternion", "NativeStructuredValue" => "UiValue", _ when native.StartsWith("Native", StringComparison.Ordinal) => native["Native".Length..], _ => native };
    private static string SafeEnumMember(string enumName, string member) => RawIdentifier(member.StartsWith($"{enumName}_", StringComparison.Ordinal) ? member[(enumName.Length + 1)..] : member);
    private static string SafeType(BindingModel model, string native) => IsDisposableHandle(model, native) ? OwnerType(native) : SafeType(native);
    private static string RawType(string type)
    {
        string value = type.Replace("const ", "", StringComparison.Ordinal).Replace("struct ", "", StringComparison.Ordinal).Replace("enum ", "", StringComparison.Ordinal).Trim(); int pointers = value.Count(character => character == '*'); value = value.TrimEnd('*').Trim();
        string mapped = value switch { "void" => "void", "bool" or "_Bool" => "byte", "int16_t" => "short", "int32_t" or "int" => "int", "int64_t" => "long", "uint16_t" => "ushort", "uint32_t" => "uint", "uint64_t" => "ulong", "size_t" => "nuint", "float" => "float", "double" => "double", "uint8_t" => "byte", _ => value };
        return mapped + new string('*', pointers);
    }
    private static string RawFieldDeclaration(Field field)
    {
        const string marker = " (*)(";
        if (!field.Type.Contains(marker, StringComparison.Ordinal)) return $"internal {RawType(field.Type)} {RawIdentifier(field.Name)};";
        int markerIndex = field.Type.IndexOf(marker, StringComparison.Ordinal);
        string returnType = field.Type[..markerIndex];
        string parameters = field.Type[(markerIndex + marker.Length)..^1];
        string[] rawParameters = parameters == "void" ? [] : parameters.Split(", ", StringSplitOptions.TrimEntries);
        return $"internal delegate* unmanaged[Cdecl]<{string.Join(", ", rawParameters.Select(RawType).Append(RawType(returnType)))}> {RawIdentifier(field.Name)};";
    }
    private static string Pascal(string value) => string.Concat(value.Split('_', StringSplitOptions.RemoveEmptyEntries).Select(part => char.ToUpperInvariant(part[0]) + part[1..]));
    private static readonly HashSet<string> CSharpKeywords = new(StringComparer.Ordinal)
    {
        // Reserved keywords.
        "abstract", "as", "base", "bool", "break", "byte", "case", "catch", "char", "checked", "class", "const", "continue", "decimal", "default", "delegate", "do", "double", "else", "enum", "event", "explicit", "extern", "false", "finally", "fixed", "float", "for", "foreach", "goto", "if", "implicit", "in", "int", "interface", "internal", "is", "lock", "long", "namespace", "new", "null", "object", "operator", "out", "override", "params", "private", "protected", "public", "readonly", "ref", "return", "sbyte", "sealed", "short", "sizeof", "stackalloc", "static", "string", "struct", "switch", "this", "throw", "true", "try", "typeof", "uint", "ulong", "unchecked", "unsafe", "ushort", "using", "virtual", "void", "volatile", "while",
        // Contextual keywords are also escaped at the raw ABI boundary so a
        // future language-version change cannot make a generated member invalid.
        "add", "alias", "allows", "and", "ascending", "async", "await", "by", "descending", "dynamic", "equals", "extension", "field", "file", "from", "get", "global", "group", "init", "into", "join", "let", "managed", "nameof", "nint", "not", "notnull", "nuint", "on", "or", "orderby", "partial", "record", "ref", "remove", "required", "scoped", "select", "set", "unmanaged", "unbox", "value", "var", "when", "where", "with", "yield"
    };
    private static string RawIdentifier(string value) => CSharpKeywords.Contains(value) ? $"@{value}" : value;
    private static string SafeServiceName(string service) => service == "Rng" ? "Random" : service;
    private static bool IsDestroy(Callback callback) => callback.Parameters.Count == 2 && BindingModel.Bare(callback.Parameters[1]).EndsWith("Handle", StringComparison.Ordinal) && callback.Name.Contains("Destroy", StringComparison.Ordinal);
    private static IEnumerable<string> LeaseHandleTypes(BindingModel model) => model.Structs.Values.Where(value => BindingModel.IsLeaseResult(value.Name, model.Structs)).Select(value => BindingModel.Bare(value.Fields.Single(field => field.Name == "handle").Type)).Distinct(StringComparer.Ordinal);
    private static IEnumerable<string> DisposableHandleTypes(BindingModel model) => model.Services.SelectMany(service => service.Operations.Select(operation => model.Callbacks[operation.Callback])).Where(IsDestroy).Select(callback => BindingModel.Bare(callback.Parameters[1])).Where(handle => !LeaseHandleTypes(model).Contains(handle, StringComparer.Ordinal)).Distinct(StringComparer.Ordinal);
    private static bool IsDisposableHandle(BindingModel model, string handle) => DisposableHandleTypes(model).Contains(handle, StringComparer.Ordinal);
    private static string OwnerType(string handle) => SafeType(handle).Replace("Handle", "", StringComparison.Ordinal);
    private static IEnumerable<Field> LeasePointers(Struct lease) => lease.Fields.Where(field => field.Type.Contains('*', StringComparison.Ordinal));
    private static IEnumerable<Field> LeaseMetadataFields(Struct lease)
    {
        HashSet<string> collectionFields = LeasePointers(lease)
            .SelectMany(pointer => new[] { pointer.Name, $"{pointer.Name}_len" })
            .ToHashSet(StringComparer.Ordinal);
        return lease.Fields.Where(field => field.Name != "handle" && !collectionFields.Contains(field.Name));
    }
    private static bool HasLeaseMetadata(BindingModel model, Struct lease) => lease.Name != "NativeByteLease" && BindingModel.IsLeaseResult(lease.Name, model.Structs) && LeaseMetadataFields(lease).Any();
    private static bool UsesLeaseReceipt(BindingModel model, Struct lease) => lease.Name != "NativeByteLease" && BindingModel.IsLeaseResult(lease.Name, model.Structs) && (LeasePointers(lease).Skip(1).Any() || HasLeaseMetadata(model, lease));
    private static string LeaseReceiptType(Struct lease) => $"{SafeType(lease.Name)}Receipt";
    private static StringBuilder Header(string purpose) => new($"// <auto-generated />{Environment.NewLine}// Generated from csharp-engine-abi through the ClangSharp AST: {purpose}.{Environment.NewLine}// Do not edit.{Environment.NewLine}#nullable enable{Environment.NewLine}using System;{Environment.NewLine}using System.Numerics;{Environment.NewLine}{Environment.NewLine}");
}
