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
internal sealed record Struct(string Name, IReadOnlyList<Field> Fields);
internal sealed record Enum(string Name, IReadOnlyList<string> Members);
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
            .Select(cursor => new Enum(cursor.Spelling.ToString(), Children(cursor).Where(member => member.Kind == CXCursorKind.CXCursor_EnumConstantDecl).Select(member => member.Spelling.ToString()).ToArray()))
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
        if (parameters.Length == 0) Fail(family, method, signature, "service calls require one supported input or out receipt");
        if (parameters[^1].Contains('*', StringComparison.Ordinal) && !IsExactBorrowedPointer(parameters[^1]) && !IsExactOutPointer(parameters[^1])) Fail(family, method, signature, $"final pointer {parameters[^1]} must be exactly const T * input or T * out receipt");
        bool hasReceipt = IsExactOutPointer(parameters[^1]);
        int inputs = hasReceipt ? parameters.Length - 1 : parameters.Length;
        if (hasReceipt) ValidateFixedType(family, method, signature, Bare(parameters[^1]), structs, enums, new HashSet<string>(StringComparer.Ordinal), "out receipt");
        if (inputs == 2 && IsExactBorrowedPointer(parameters[0]) && Bare(parameters[1]) == "size_t")
        {
            ValidateFixedType(family, method, signature, Bare(parameters[0]), structs, enums, new HashSet<string>(StringComparer.Ordinal), "pointer/count span element");
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

    private static void ValidateFixedType(string family, string method, string signature, string type, IReadOnlyDictionary<string, Struct> structs, IReadOnlyDictionary<string, Enum> enums, HashSet<string> seen, string role)
    {
        if (IsScalar(type) || enums.ContainsKey(type)) return;
        if (!structs.TryGetValue(type, out Struct? value) || value is null) { Fail(family, method, signature, $"{role} {type} is not a supported scalar or emitted native struct"); return; }
        if (type.EndsWith("Api", StringComparison.Ordinal) || type is "NativeProductApi" or "NativeProductCreateArgs" or "NativeTurnArgs") Fail(family, method, signature, $"{role} {type} is an API/product table rather than a fixed value");
        if (type is "NativeUtf8Slice" or "NativeStructuredValue") Fail(family, method, signature, $"{role} {type} is only supported as a specially marshalled request field");
        if (!seen.Add(type)) return;
        foreach (Field field in value.Fields)
        {
            if (field.Type.Contains('*', StringComparison.Ordinal)) Fail(family, method, signature, $"{role} {type}.{field.Name} ({field.Type}) is borrowed and cannot be emitted by-value");
            string nested = Bare(field.Type);
            if (nested is "NativeUtf8Slice" or "NativeStructuredValue") Fail(family, method, signature, $"{role} {type}.{field.Name} ({field.Type}) requires request-only marshalling");
            ValidateFixedType(family, method, signature, nested, structs, enums, seen, $"{role} field {type}.{field.Name}");
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
                if (pointed is "NativeUtf8Slice" or "NativeStructuredValue") Fail(family, method, signature, $"borrowed request {request.Name}.{field.Name} ({field.Type}) cannot point to special immediate value {pointed}");
                if (!IsExactBorrowedPointer(field.Type)) Fail(family, method, signature, $"borrowed request {request.Name}.{field.Name} ({field.Type}) must be exactly const T *");
                if (index + 1 >= request.Fields.Count || (request.Fields[index + 1].Name != $"{field.Name}_len" && request.Fields[index + 1].Name != $"{field.Name}_count") || Bare(request.Fields[index + 1].Type) != "size_t") Fail(family, method, signature, $"borrowed request {request.Name}.{field.Name} ({field.Type}) lacks an adjacent size_t _len/_count field");
                ValidateFixedType(family, method, signature, pointed, structs, enums, new HashSet<string>(StringComparer.Ordinal), $"borrowed span element {request.Name}.{field.Name}");
                index++;
                continue;
            }
            string nested = Bare(field.Type);
            if (nested == "NativeUtf8Slice") continue;
            if (nested == "NativeStructuredValue") continue;
            ValidateFixedType(family, method, signature, nested, structs, enums, seen, $"borrowed request field {request.Name}.{field.Name}");
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
    public static bool IsScalar(string type) => type is "void" or "int" or "int32_t" or "int64_t" or "uint32_t" or "uint64_t" or "size_t" or "float" or "double" or "uint8_t";
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
        output.AppendLine("    public EngineCallException(string service, string operation, int status) : base($\"Rusty Engine {service}.{operation} returned status {status}.\") => Status = status;");
        output.AppendLine("    public int Status { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public readonly ref struct ProductUpdate").AppendLine("{");
        output.AppendLine("    public ProductUpdate(uint kind, ReadOnlySpan<ProductInputEvent> input, ulong observation) { Kind = kind; Input = input; Observation = observation; }");
        output.AppendLine("    public uint Kind { get; }");
        output.AppendLine("    public ReadOnlySpan<ProductInputEvent> Input { get; }").AppendLine("    public ulong Observation { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public sealed class ProductCreateContext").AppendLine("{");
        output.AppendLine("    public ProductCreateContext(IEngineContext engine, ProductContent content) { Engine = engine ?? throw new ArgumentNullException(nameof(engine)); Content = content ?? throw new ArgumentNullException(nameof(content)); }");
        output.AppendLine("    public IEngineContext Engine { get; }").AppendLine("    public ProductContent Content { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public sealed class ProductContent").AppendLine("{");
        output.AppendLine("    public ProductContent(ReadOnlyMemory<ProductContentFile> files) => Files = files;");
        output.AppendLine("    public ReadOnlyMemory<ProductContentFile> Files { get; }").AppendLine("}").AppendLine();
        output.AppendLine("public readonly record struct ProductContentFile(ReadOnlyMemory<byte> Path, ReadOnlyMemory<byte> Bytes);");
        output.AppendLine("public readonly record struct ProductInputEvent(uint Kind, uint Edge, ulong Sequence, float X, float Y, ReadOnlyMemory<byte> Label);");
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
            foreach (string member in value.Members) output.AppendLine($"    {SafeEnumMember(value.Name, member)},");
            output.AppendLine("}").AppendLine();
        }
        foreach (Struct value in model.Structs.Values.Where(IsSafeValue).OrderBy(value => value.Name, StringComparer.Ordinal))
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
            foreach (string member in value.Members) output.AppendLine($"    {member},");
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
        output.AppendLine("internal static class NativeCall").AppendLine("{");
        output.AppendLine("    internal static void Require(string service, string operation, int status) { if (status != 1) throw new EngineCallException(service, operation, status); }");
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
        foreach (Struct value in model.Structs.Values.Where(IsSafeValue).OrderBy(value => value.Name, StringComparer.Ordinal))
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
            string assignments = string.Join(", ", value.Fields.Select(field => $"{RawIdentifier(field.Name)} = ToNative(value.{Pascal(field.Name)})"));
            string arguments = string.Join(", ", value.Fields.Select(field => $"FromNative(value.{RawIdentifier(field.Name)})"));
            output.AppendLine($"    internal static {value.Name} ToNative({safe} value) => new() {{ {assignments} }};");
            if (!HasDisposableHandleField(model, value)) output.AppendLine($"    internal static {safe} FromNative({value.Name} value) => new({arguments});");
        }
        output.AppendLine("    internal static int ToNative(int value) => value;");
        output.AppendLine("    internal static long ToNative(long value) => value;");
        output.AppendLine("    internal static uint ToNative(uint value) => value;");
        output.AppendLine("    internal static ulong ToNative(ulong value) => value;");
        output.AppendLine("    internal static nuint ToNative(nuint value) => value;");
        output.AppendLine("    internal static float ToNative(float value) => value;");
        output.AppendLine("    internal static double ToNative(double value) => value;");
        output.AppendLine("    internal static byte ToNative(byte value) => value;");
        output.AppendLine("    internal static int FromNative(int value) => value;");
        output.AppendLine("    internal static long FromNative(long value) => value;");
        output.AppendLine("    internal static uint FromNative(uint value) => value;");
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
        if (IsSpanCall(input)) return EmitSpanMethod(service, operation, callback, returnType, signature);
        if (input.Length == 1 && input[0].StartsWith("const ", StringComparison.Ordinal) && input[0].Contains('*', StringComparison.Ordinal)) return EmitBorrowedRequestMethod(model, service, operation, callback, returnType, signature, BindingModel.Bare(input[0]), result);
        return EmitDirectMethod(model, service, operation, callback, returnType, signature, input, result);
    }

    private static string EmitDirectMethod(BindingModel model, Service service, string operation, Callback callback, string returnType, string signature, string[] input, string result)
    {
        StringBuilder output = new();
        output.AppendLine($"    public {returnType} {Pascal(operation)}({signature})").AppendLine("    {");
        for (int index = 0; index < input.Length; index++) output.AppendLine($"        {RawType(input[index])} raw{index} = NativeConversions.ToNative(arg{index});");
        if (!string.IsNullOrEmpty(result)) output.AppendLine($"        {RawType(result)} rawResult = default;");
        string invocation = string.Join(", ", new[] { "_native.context" }.Concat(input.Select((_, index) => $"raw{index}")).Concat(string.IsNullOrEmpty(result) ? [] : ["&rawResult"]));
        output.AppendLine($"        int status = _native.{operation}.Pointer({invocation});");
        output.AppendLine($"        NativeCall.Require(\"{SafeServiceName(service.Name)}\", \"{Pascal(operation)}\", status);");
        if (string.IsNullOrEmpty(result)) output.AppendLine("        return;");
        else if (returnType != SafeType(BindingModel.Bare(result)))
        {
            (_, string destroyOperation) = DestroyFor(model, BindingModel.Bare(result));
            output.AppendLine($"        {RawType(result)} ownedResult = rawResult;");
            output.AppendLine($"        return new {returnType}(NativeConversions.FromNative(ownedResult), () => {{ int disposeStatus = _native.{destroyOperation}.Pointer(_native.context, ownedResult); NativeCall.Require(\"{SafeServiceName(service.Name)}\", \"{Pascal(destroyOperation)}\", disposeStatus); }});");
        }
        else output.AppendLine("        return NativeConversions.FromNative(rawResult);");
        output.AppendLine("    }").AppendLine();
        return output.ToString();
    }

    private static string EmitSpanMethod(Service service, string operation, Callback callback, string returnType, string signature)
    {
        string item = BindingModel.Bare(Inputs(callback)[0]);
        StringBuilder output = new();
        output.AppendLine($"    public {returnType} {Pascal(operation)}({signature})").AppendLine("    {");
        output.AppendLine($"        {RawType(item)}[] rawValues = values.ToArray().Select(NativeConversions.ToNative).ToArray();");
        output.AppendLine($"        fixed ({RawType(item)}* pointer = rawValues)").AppendLine("        {");
        output.AppendLine($"            int status = _native.{operation}.Pointer(_native.context, rawValues.Length == 0 ? null : pointer, (nuint)rawValues.Length);");
        output.AppendLine($"            NativeCall.Require(\"{SafeServiceName(service.Name)}\", \"{Pascal(operation)}\", status);");
        output.AppendLine("        }").AppendLine("    }").AppendLine();
        return output.ToString();
    }

    private static string EmitBorrowedRequestMethod(BindingModel model, Service service, string operation, Callback callback, string returnType, string signature, string requestName, string result)
    {
        Struct request = model.Structs[requestName];
        StringBuilder output = new();
        output.AppendLine($"    public {returnType} {Pascal(operation)}({signature})").AppendLine("    {");
        List<string> closers = [];
        foreach (Field field in request.Fields)
        {
            if (BindingModel.Bare(field.Type) == "NativeUtf8Slice")
            {
                string property = Pascal(field.Name);
                output.AppendLine($"        byte[] {field.Name}Bytes = Encoding.UTF8.GetBytes(arg0.{property} ?? throw new ArgumentNullException(nameof(arg0))); ");
                output.AppendLine($"        fixed (byte* {field.Name}Pointer = {field.Name}Bytes)").AppendLine("        {");
                closers.Add("        }");
            }
        }
        for (int index = 0; index < request.Fields.Count; index++)
        {
            Field field = request.Fields[index];
            if (!field.Type.Contains('*', StringComparison.Ordinal)) continue;
            if (index + 1 < request.Fields.Count && (request.Fields[index + 1].Name == $"{field.Name}_len" || request.Fields[index + 1].Name == $"{field.Name}_count"))
            {
                output.AppendLine($"        using MemoryHandle {field.Name}Pin = arg0.{Pascal(field.Name)}.Pin();");
            }
        }
        foreach (Field field in request.Fields.Where(field => BindingModel.Bare(field.Type) == "NativeStructuredValue"))
        {
            string property = Pascal(field.Name);
            output.AppendLine($"        using MemoryHandle {field.Name}NodesPin = arg0.{property}.Nodes.Pin();");
            output.AppendLine($"        using MemoryHandle {field.Name}EdgesPin = arg0.{property}.Edges.Pin();");
            output.AppendLine($"        using MemoryHandle {field.Name}Utf8Pin = arg0.{property}.Utf8.Pin();");
        }
        output.AppendLine($"        {requestName} raw = new() {{");
        for (int index = 0; index < request.Fields.Count; index++)
        {
            Field field = request.Fields[index];
            if (index > 0 && request.Fields[index - 1].Type.Contains('*', StringComparison.Ordinal) && (field.Name == $"{request.Fields[index - 1].Name}_len" || field.Name == $"{request.Fields[index - 1].Name}_count"))
            {
                output.AppendLine($"            {RawIdentifier(field.Name)} = (nuint)arg0.{Pascal(request.Fields[index - 1].Name)}.Length,"); continue;
            }
            string expression = BorrowedFieldExpression(field);
            output.AppendLine($"            {RawIdentifier(field.Name)} = {expression},");
        }
        output.AppendLine("        };");
        if (!string.IsNullOrEmpty(result)) output.AppendLine($"        {RawType(result)} rawResult = default;");
        string invocation = string.IsNullOrEmpty(result) ? "_native.context, &raw" : "_native.context, &raw, &rawResult";
        output.AppendLine($"        int status = _native.{operation}.Pointer({invocation});");
        output.AppendLine($"        NativeCall.Require(\"{SafeServiceName(service.Name)}\", \"{Pascal(operation)}\", status);");
        if (string.IsNullOrEmpty(result)) output.AppendLine("        return;");
        else if (returnType != SafeType(BindingModel.Bare(result)))
        {
            (_, string destroyOperation) = DestroyFor(model, BindingModel.Bare(result));
            output.AppendLine($"        {RawType(result)} ownedResult = rawResult;");
            output.AppendLine($"        return new {returnType}(NativeConversions.FromNative(ownedResult), () => {{ int disposeStatus = _native.{destroyOperation}.Pointer(_native.context, ownedResult); NativeCall.Require(\"{SafeServiceName(service.Name)}\", \"{Pascal(destroyOperation)}\", disposeStatus); }});");
        }
        else output.AppendLine("        return NativeConversions.FromNative(rawResult);");
        for (int index = closers.Count - 1; index >= 0; index--) output.AppendLine(closers[index]);
        output.AppendLine("    }").AppendLine();
        return output.ToString();
    }

    private static string BorrowedFieldExpression(Field field)
    {
        string bare = BindingModel.Bare(field.Type);
        string property = Pascal(field.Name);
        if (bare == "NativeUtf8Slice") return $"new NativeUtf8Slice {{ bytes = {field.Name}Bytes.Length == 0 ? null : {field.Name}Pointer, len = (nuint){field.Name}Bytes.Length }}";
        if (bare == "NativeStructuredValue") return $"new NativeStructuredValue {{ nodes = arg0.{property}.Nodes.Length == 0 ? null : (NativeStructuredValueNode*){field.Name}NodesPin.Pointer, node_count = (nuint)arg0.{property}.Nodes.Length, edges = arg0.{property}.Edges.Length == 0 ? null : (uint*){field.Name}EdgesPin.Pointer, edge_count = (nuint)arg0.{property}.Edges.Length, root = arg0.{property}.Root, utf8 = arg0.{property}.Utf8.Length == 0 ? null : (byte*){field.Name}Utf8Pin.Pointer, utf8_len = (nuint)arg0.{property}.Utf8.Length }}";
        if (field.Type.Contains('*', StringComparison.Ordinal)) return $"arg0.{property}.Length == 0 ? null : ({RawType(bare)}*){field.Name}Pin.Pointer";
        return $"NativeConversions.ToNative(arg0.{property})";
    }

    private static string[] Inputs(Callback callback)
    {
        string[] args = callback.Parameters.Skip(1).ToArray();
        if (args.Length > 0 && !args[^1].StartsWith("const ", StringComparison.Ordinal) && args[^1].Contains('*', StringComparison.Ordinal) && BindingModel.Bare(args[^1]) != "void") return args[..^1];
        return args;
    }
    private static string? ResultParameter(Callback callback)
    {
        string last = callback.Parameters.Last();
        return !last.StartsWith("const ", StringComparison.Ordinal) && last.Contains('*', StringComparison.Ordinal) && BindingModel.Bare(last) != "void" ? BindingModel.Bare(last) : null;
    }
    private static bool IsSpanCall(string[] input) => input.Length == 2 && input[0].StartsWith("const ", StringComparison.Ordinal) && input[0].Contains('*', StringComparison.Ordinal) && BindingModel.Bare(input[1]) == "size_t";
    private static bool HasBorrowedFields(Struct value) => value.Fields.Any(field => field.Type.Contains('*', StringComparison.Ordinal) || BindingModel.Bare(field.Type) is "NativeUtf8Slice" or "NativeStructuredValue");
    private static bool HasDisposableHandleField(BindingModel model, Struct value) => value.Fields.Any(field => IsDisposableHandle(model, BindingModel.Bare(field.Type)));
    private static (string Service, string Operation) DestroyFor(BindingModel model, string handle) => model.Services.SelectMany(service => service.Operations.Select(operation => (service, operation))).First(pair => IsDestroy(model.Callbacks[pair.operation.Callback]) && BindingModel.Bare(model.Callbacks[pair.operation.Callback].Parameters[1]) == handle) is var found ? (found.service.Name, found.operation.Name) : throw new InvalidOperationException($"no destroy operation found for {handle}");

    private static bool IsSafeValue(Struct value) => value.Name is not "NativeEngineApi" and not "NativeProductApi" and not "NativeProductCreateArgs" and not "NativeTurnArgs" and not "NativeContentFile" and not "NativeInputEvent" and not "NativeUtf8Slice" and not "NativeStructuredValue" and not "NativeVec2" and not "NativeVec3" and not "NativeQuat" && !value.Name.EndsWith("Api", StringComparison.Ordinal);
    private static IReadOnlyList<(Field Field, string Type)> SafeFields(Struct value, BindingModel model)
    {
        List<(Field, string)> fields = [];
        for (int index = 0; index < value.Fields.Count; index++)
        {
            Field field = value.Fields[index];
            if (BindingModel.Bare(field.Type) == "NativeUtf8Slice") { fields.Add((field, "string")); continue; }
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
        string last = callback.Parameters.Last();
        if (last.StartsWith("const ", StringComparison.Ordinal) || !last.Contains('*', StringComparison.Ordinal) || BindingModel.Bare(last) == "void") return "void";
        string handle = BindingModel.Bare(last);
        return IsDisposableHandle(model, handle) ? OwnerType(handle) : SafeType(handle);
    }
    private static string SafeParameters(BindingModel model, Callback callback)
    {
        string[] args = callback.Parameters.Skip(1).ToArray();
        if (args.Length > 0 && !args[^1].StartsWith("const ", StringComparison.Ordinal) && args[^1].Contains('*', StringComparison.Ordinal) && BindingModel.Bare(args[^1]) != "void") args = args[..^1];
        if (args.Length == 2 && args[0].StartsWith("const ", StringComparison.Ordinal) && args[0].Contains('*', StringComparison.Ordinal) && BindingModel.Bare(args[1]) == "size_t") return $"ReadOnlySpan<{SafeType(model, BindingModel.Bare(args[0]))}> values";
        return string.Join(", ", args.Select((type, index) => $"{SafeType(model, BindingModel.Bare(type))} arg{index}"));
    }
    private static string SafeType(string native) => native switch { "int" or "int32_t" => "int", "int64_t" => "long", "uint32_t" => "uint", "uint64_t" => "ulong", "size_t" => "nuint", "float" => "float", "double" => "double", "uint8_t" => "byte", "NativeVec2" => "Vector2", "NativeVec3" => "Vector3", "NativeQuat" => "Quaternion", "NativeStructuredValue" => "UiValue", _ when native.StartsWith("Native", StringComparison.Ordinal) => native["Native".Length..], _ => native };
    private static string SafeEnumMember(string enumName, string member) => member.StartsWith($"{enumName}_", StringComparison.Ordinal) ? member[(enumName.Length + 1)..] : member;
    private static string SafeType(BindingModel model, string native) => IsDisposableHandle(model, native) ? OwnerType(native) : SafeType(native);
    private static string RawType(string type)
    {
        string value = type.Replace("const ", "", StringComparison.Ordinal).Replace("struct ", "", StringComparison.Ordinal).Replace("enum ", "", StringComparison.Ordinal).Trim(); int pointers = value.Count(character => character == '*'); value = value.TrimEnd('*').Trim();
        string mapped = value switch { "void" => "void", "int32_t" or "int" => "int", "int64_t" => "long", "uint32_t" => "uint", "uint64_t" => "ulong", "size_t" => "nuint", "float" => "float", "double" => "double", "uint8_t" => "byte", _ => value };
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
    private static string RawIdentifier(string value) => value is "base" ? "@base" : value;
    private static string SafeServiceName(string service) => service == "Rng" ? "Random" : service;
    private static bool IsDestroy(Callback callback) => callback.Parameters.Count == 2 && BindingModel.Bare(callback.Parameters[1]).EndsWith("Handle", StringComparison.Ordinal) && callback.Name.Contains("Destroy", StringComparison.Ordinal);
    private static IEnumerable<string> DisposableHandleTypes(BindingModel model) => model.Services.SelectMany(service => service.Operations.Select(operation => model.Callbacks[operation.Callback])).Where(IsDestroy).Select(callback => BindingModel.Bare(callback.Parameters[1])).Distinct(StringComparer.Ordinal);
    private static bool IsDisposableHandle(BindingModel model, string handle) => DisposableHandleTypes(model).Contains(handle, StringComparer.Ordinal);
    private static string OwnerType(string handle) => SafeType(handle).Replace("Handle", "", StringComparison.Ordinal);
    private static StringBuilder Header(string purpose) => new($"// <auto-generated />{Environment.NewLine}// Generated from csharp-engine-abi through the ClangSharp AST: {purpose}.{Environment.NewLine}// Do not edit.{Environment.NewLine}#nullable enable{Environment.NewLine}using System;{Environment.NewLine}using System.Numerics;{Environment.NewLine}{Environment.NewLine}");
}
