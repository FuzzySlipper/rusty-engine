#!/usr/bin/env python3
"""Mechanically emit a safe C# facade from the cbindgen C declarations.

`csharp-engine-abi/src/lib.rs` is the sole ABI source. This helper recognizes ordinary direct
service signatures (context plus request/result, borrowed spans, and borrowed
fixed-layout value fields) from the generated header; it has no service or
function inventory.
"""

from pathlib import Path
import re
import sys


def pascal(name: str) -> str:
    return "".join(part[:1].upper() + part[1:] for part in name.split("_"))


def table_name(receiver: str) -> str:
    return f"Native{pascal(receiver)}Api"


def normalize(argument: str) -> str:
    argument = re.sub(r"/\*.*?\*/", "", argument, flags=re.S)
    argument = re.sub(r"\bconst\b", "", argument)
    argument = re.sub(r"\bstruct\b", "", argument)
    return " ".join(argument.replace(" *", "*").split())


def pointer(argument: str) -> bool:
    return normalize(argument).endswith("*")


def base_type(argument: str) -> str:
    return normalize(argument).rstrip("*").strip()


def csharp_type(argument: str) -> str:
    name = base_type(argument)
    return {
        "int32_t": "int",
        "int64_t": "long",
        "uint32_t": "uint",
        "uint64_t": "ulong",
        "size_t": "nuint",
    }.get(name, name)


def parse_function_types(header: str) -> dict[str, list[str]]:
    return {
        name: [normalize(argument) for argument in arguments.split(",") if argument.strip()]
        for name, arguments in re.findall(
            r"typedef\s+int32_t\s+\(\*([A-Za-z0-9_]+)\)\((.*?)\);", header, re.S
        )
    }


def parse_struct(header: str, name: str) -> list[tuple[str, str]]:
    match = re.search(
        rf"typedef struct {name}\s*\{{(.*?)\}}\s*{name};", header, re.S
    )
    if match is None:
        raise SystemExit(f"generated header has no {name} definition")
    fields = []
    for declaration in match.group(1).split(";"):
        declaration = " ".join(declaration.split())
        if not declaration:
            continue
        field = re.search(r"(.+?)\s+(\*?)([A-Za-z0-9_]+)$", declaration)
        if field is None:
            raise SystemExit(f"cannot parse {name} field: {declaration}")
        fields.append((normalize(field.group(1) + field.group(2)), field.group(3)))
    return fields


def api_members(header: str) -> list[tuple[str, str]]:
    return parse_struct(header, "NativeEngineApi")


def table_methods(header: str, table: str, function_types: dict[str, list[str]]) -> list[tuple[str, list[str]]]:
    methods = []
    for field_type, field in parse_struct(header, table):
        if field != "context":
            methods.append((field, function_types[field_type]))
    return methods


def native_struct_fields(header: str, name: str) -> list[tuple[str, str]]:
    return parse_struct(header, name)


def request_with_utf8_slices(
    header: str, request: str
) -> tuple[list[tuple[str, str]], list[str]] | None:
    fields = native_struct_fields(header, request)
    slices = [field for field_type, field in fields if field_type == "NativeUtf8Slice"]
    if not 1 <= len(slices) <= 2 or any(
        pointer(field_type)
        for field_type, _ in fields
        if field_type != "NativeUtf8Slice"
    ):
        return None
    values = [
        (field, csharp_type(field_type))
        for field_type, field in fields
        if field_type != "NativeUtf8Slice"
    ]
    return (values, slices) if slices else None


def structured_projection_fields(header: str, request: str) -> tuple[str, str, str] | None:
    fields = dict((field, field_type) for field_type, field in native_struct_fields(header, request))
    stream = next((field for field, kind in fields.items() if kind == "NativeUiStreamHandle"), None)
    sequence = next((field for field, kind in fields.items() if kind == "uint64_t"), None)
    value = next((field for field, kind in fields.items() if kind == "NativeStructuredValue"), None)
    return (stream, sequence, value) if stream and sequence and value else None


def request_with_spans(header: str, request: str) -> tuple[list[tuple[str, str]], list[tuple[str, str, str]]] | None:
    fields = native_struct_fields(header, request)
    values: list[tuple[str, str]] = []
    spans: list[tuple[str, str, str]] = []
    index = 0
    while index < len(fields):
        field_type, field = fields[index]
        if pointer(field_type):
            if index + 1 >= len(fields) or base_type(fields[index + 1][0]) != "size_t":
                return None
            count_type, count = fields[index + 1]
            if count not in (f"{field}_len", f"{field}_count"):
                return None
            spans.append((field, csharp_type(field_type), count))
            index += 2
            continue
        values.append((field, csharp_type(field_type)))
        index += 1
    return (values, spans) if spans else None


def direct_method(header: str, field: str, arguments: list[str]) -> str:
    if len(arguments) == 3 and pointer(arguments[1]) and base_type(arguments[2]) == "size_t":
        item = csharp_type(arguments[1])
        return f'''        public void {pascal(field)}(ReadOnlySpan<{item}> values)
        {{
            fixed ({item}* pointer = values)
            {{
                NativeCall.RequireSuccess(_native.{field}(_native.context, values.Length == 0 ? null : pointer, (nuint)values.Length));
            }}
        }}'''
    raise SystemExit(f"unsupported direct signature for {field}: {arguments}")


def method(header: str, receiver: str, field: str, arguments: list[str]) -> str:
    if not arguments or arguments[0] != "void*":
        raise SystemExit(f"{field} must have context-first direct signature")
    user = arguments[1:]
    public = pascal(field)
    if len(user) == 2 and pointer(user[0]) and base_type(user[1]) == "size_t":
        item = csharp_type(user[0])
        return f'''        public void {public}(ReadOnlySpan<{item}> values)
        {{
            fixed ({item}* pointer = values)
            {{
                NativeCall.RequireSuccess(_native.{field}(_native.context, values.Length == 0 ? null : pointer, (nuint)values.Length));
            }}
        }}'''
    if len(user) == 2 and not pointer(user[0]) and pointer(user[1]):
        request, result = csharp_type(user[0]), csharp_type(user[1])
        return f'''        public {result} {public}({request} request)
        {{
            {result} result = default;
            NativeCall.RequireSuccess(_native.{field}(_native.context, request, &result));
            return result;
        }}'''
    if len(user) == 2 and pointer(user[0]) and pointer(user[1]):
        request, result = csharp_type(user[0]), csharp_type(user[1])
        utf8_request = request_with_utf8_slices(header, request)
        if utf8_request is not None:
            values, utf8_fields = utf8_request
            parameters = [f"{kind} {name}" for name, kind in values]
            parameters.extend(f"string {name}" for name in utf8_fields)
            callbacks = ", ".join(utf8_fields)
            slices = ", ".join(f"slice{index}" for index in range(len(utf8_fields)))
            assignments = [f"{name} = {name}" for name, _ in values]
            assignments.extend(
                f"{name} = {slice}"
                for name, slice in zip(utf8_fields, slices.split(", "))
            )
            helper = "WithSlice" if len(utf8_fields) == 1 else "WithSlices"
            return f'''        public {result} {public}({", ".join(parameters)})
        {{
            {table_name(receiver)} native = _native;
            return BorrowedUtf8.{helper}(({slices}) =>
            {{
                {request} request = new() {{ {", ".join(assignments)} }};
                {result} result = default;
                NativeCall.RequireSuccess(native.{field}(native.context, &request, &result));
                return result;
            }}, {callbacks});
        }}'''
        span_request = request_with_spans(header, request)
        if span_request is not None:
            values, spans = span_request
            parameters = [f"{kind} {name}" for name, kind in values]
            parameters.extend(f"ReadOnlySpan<{kind}> {name}" for name, kind, _ in spans)
            fixed = "\n".join(f"            fixed ({kind}* {name}Pointer = {name})" for name, kind, _ in spans)
            assignments = [f"{name} = {name}" for name, _ in values]
            assignments.extend(
                f"{name} = {name}.Length == 0 ? null : {name}Pointer, {count} = (nuint){name}.Length"
                for name, _, count in spans
            )
            return f'''        public {result} {public}({", ".join(parameters)})
        {{
            {table_name(receiver)} native = _native;
{fixed}
            {{
                {request} request = new() {{ {", ".join(assignments)} }};
                {result} result = default;
                NativeCall.RequireSuccess(native.{field}(native.context, &request, &result));
                return result;
            }}
        }}'''
        return f'''        public {result} {public}({request} request)
        {{
            {result} result = default;
            NativeCall.RequireSuccess(_native.{field}(_native.context, &request, &result));
            return result;
        }}'''
    if len(user) == 1 and pointer(user[0]):
        request = csharp_type(user[0])
        projection = structured_projection_fields(header, request)
        if projection is not None:
            stream, sequence, value = projection
            return f'''        public void {public}(NativeUiStreamHandle {stream}, ulong {sequence}, StructuredValueArena value)
        {{
            ArgumentNullException.ThrowIfNull(value);
            {table_name(receiver)} native = _native;
            value.WithNative(nativeValue =>
            {{
                {request} request = new() {{ {stream} = {stream}, {sequence} = {sequence}, {value} = nativeValue }};
                NativeCall.RequireSuccess(native.{field}(native.context, &request));
            }});
        }}'''
        return f'''        public void {public}({request} request)
        {{
            NativeCall.RequireSuccess(_native.{field}(_native.context, &request));
        }}'''
    if len(user) == 1 and not pointer(user[0]):
        request = csharp_type(user[0])
        return f'''        public void {public}({request} request)
        {{
            NativeCall.RequireSuccess(_native.{field}(_native.context, request));
        }}'''
    raise SystemExit(f"unsupported table signature for {field}: {arguments}")


def facade(header: str) -> str:
    function_types = parse_function_types(header)
    root_methods = []
    properties = []
    groups = []
    for member_type, member in api_members(header):
        if member == "context":
            continue
        if member_type in function_types:
            root_methods.append(direct_method(header, member, function_types[member_type]))
            continue
        if not member_type.endswith("Api"):
            raise SystemExit(f"NativeEngineApi member {member} has unsupported type {member_type}")
        title = member_type.removeprefix("Native").removesuffix("Api")
        properties.append(f"        public {title}Api {title} => new(_native.{member});")
        methods = "\n\n".join(method(header, member, name, args) for name, args in table_methods(header, member_type, function_types))
        groups.append(f'''    public readonly unsafe struct {title}Api
    {{
        private readonly {member_type} _native;
        internal {title}Api({member_type} native) => _native = native;

{methods}
    }}''')
    return f'''// <auto-generated />
// Generated mechanically from csharp-engine-abi/src/lib.rs through cbindgen. Do not edit.
using System;
using System.Collections.Generic;
using System.Text;

namespace Rusty.Engine.Native
{{
    public readonly unsafe struct EngineApi
    {{
        private readonly NativeEngineApi _native;
        public EngineApi(NativeEngineApi native) => _native = native;
{chr(10).join(properties)}

{chr(10).join(root_methods)}
    }}

    public sealed unsafe class StructuredValueArena
    {{
        public NativeStructuredValueNode[] Nodes {{ get; }}
        public uint[] Edges {{ get; }}
        public byte[] Utf8 {{ get; }}
        public uint Root {{ get; }}

        public StructuredValueArena(NativeStructuredValueNode[] nodes, uint[] edges, uint root, byte[] utf8)
        {{
            Nodes = nodes ?? throw new ArgumentNullException(nameof(nodes));
            Edges = edges ?? throw new ArgumentNullException(nameof(edges));
            Utf8 = utf8 ?? throw new ArgumentNullException(nameof(utf8));
            Root = root;
        }}

        internal void WithNative(Action<NativeStructuredValue> action)
        {{
            fixed (NativeStructuredValueNode* nodes = Nodes)
            fixed (uint* edges = Edges)
            fixed (byte* utf8 = Utf8)
            {{
                action(new NativeStructuredValue
                {{
                    nodes = Nodes.Length == 0 ? null : nodes,
                    node_count = (nuint)Nodes.Length,
                    edges = Edges.Length == 0 ? null : edges,
                    edge_count = (nuint)Edges.Length,
                    root = Root,
                    utf8 = Utf8.Length == 0 ? null : utf8,
                    utf8_len = (nuint)Utf8.Length,
                }});
            }}
        }}
    }}

    public sealed class StructuredValueBuilder
    {{
        private readonly List<NativeStructuredValueNode> _nodes = [];
        private readonly List<uint> _edges = [];
        private readonly List<byte> _utf8 = [];

        public uint Null() => Add(NativeStructuredValueKind.Null);
        public uint Bool(bool value) => Add(NativeStructuredValueKind.Bool, boolValue: value ? 1u : 0u);
        public uint Number(double value) => Add(NativeStructuredValueKind.Number, numberValue: value);
        public uint String(string value)
        {{
            (uint offset, uint length) = Bytes(value);
            return Add(NativeStructuredValueKind.String, textOffset: offset, textLength: length);
        }}

        public uint Array(params uint[] children)
        {{
            ArgumentNullException.ThrowIfNull(children);
            return Container(NativeStructuredValueKind.Array, children);
        }}

        public uint Object(params (string Key, uint Value)[] fields)
        {{
            ArgumentNullException.ThrowIfNull(fields);
            uint firstEdge = checked((uint)_edges.Count);
            foreach ((string key, uint value) in fields)
            {{
                if (value >= (uint)_nodes.Count) throw new ArgumentOutOfRangeException(nameof(fields));
                (uint offset, uint length) = Bytes(key);
                NativeStructuredValueNode node = _nodes[checked((int)value)];
                node.key_offset = offset;
                node.key_len = length;
                uint keyedValue = checked((uint)_nodes.Count);
                _nodes.Add(node);
                _edges.Add(keyedValue);
            }}
            return Add(NativeStructuredValueKind.Object, firstEdge: firstEdge, childCount: checked((uint)fields.Length));
        }}

        public StructuredValueArena Build(uint root)
        {{
            if (root >= (uint)_nodes.Count) throw new ArgumentOutOfRangeException(nameof(root));
            return new StructuredValueArena(_nodes.ToArray(), _edges.ToArray(), root, _utf8.ToArray());
        }}

        private uint Container(NativeStructuredValueKind kind, uint[] children)
        {{
            uint firstEdge = checked((uint)_edges.Count);
            foreach (uint child in children)
            {{
                if (child >= (uint)_nodes.Count) throw new ArgumentOutOfRangeException(nameof(children));
                _edges.Add(child);
            }}
            return Add(kind, firstEdge: firstEdge, childCount: checked((uint)children.Length));
        }}

        private uint Add(NativeStructuredValueKind kind, uint boolValue = 0, double numberValue = 0, uint textOffset = 0, uint textLength = 0, uint firstEdge = 0, uint childCount = 0)
        {{
            uint index = checked((uint)_nodes.Count);
            _nodes.Add(new NativeStructuredValueNode
            {{
                kind = (uint)kind,
                bool_value = boolValue,
                number_value = numberValue,
                text_offset = textOffset,
                text_len = textLength,
                first_edge = firstEdge,
                child_count = childCount,
            }});
            return index;
        }}

        private (uint Offset, uint Length) Bytes(string value)
        {{
            ArgumentNullException.ThrowIfNull(value);
            byte[] bytes = Encoding.UTF8.GetBytes(value);
            uint offset = checked((uint)_utf8.Count);
            _utf8.AddRange(bytes);
            return (offset, checked((uint)bytes.Length));
        }}
    }}

    internal static unsafe class BorrowedUtf8
    {{
        internal delegate T Single<T>(NativeUtf8Slice value);
        internal delegate T Pair<T>(NativeUtf8Slice first, NativeUtf8Slice second);

        internal static T WithSlice<T>(Single<T> callback, string value)
        {{
            ArgumentNullException.ThrowIfNull(callback);
            byte[] bytes = Encoding.UTF8.GetBytes(value ?? throw new ArgumentNullException(nameof(value)));
            fixed (byte* pointer = bytes)
            {{
                return callback(new NativeUtf8Slice {{ bytes = bytes.Length == 0 ? null : pointer, len = (nuint)bytes.Length }});
            }}
        }}

        internal static T WithSlices<T>(Pair<T> callback, string first, string second)
        {{
            ArgumentNullException.ThrowIfNull(callback);
            byte[] firstBytes = Encoding.UTF8.GetBytes(first ?? throw new ArgumentNullException(nameof(first)));
            byte[] secondBytes = Encoding.UTF8.GetBytes(second ?? throw new ArgumentNullException(nameof(second)));
            fixed (byte* firstPointer = firstBytes)
            fixed (byte* secondPointer = secondBytes)
            {{
                return callback(
                    new NativeUtf8Slice {{ bytes = firstBytes.Length == 0 ? null : firstPointer, len = (nuint)firstBytes.Length }},
                    new NativeUtf8Slice {{ bytes = secondBytes.Length == 0 ? null : secondPointer, len = (nuint)secondBytes.Length }});
            }}
        }}
    }}

{chr(10).join(groups)}

    internal static class NativeCall
    {{
        internal static void RequireSuccess(int status)
        {{
            if (status != 1) throw new InvalidOperationException($"Rusty Engine direct service returned {{status}}.");
        }}
    }}
}}
'''


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: generate-csharp-safe-facade.py <header.h> <output.cs>")
    Path(sys.argv[2]).write_text(facade(Path(sys.argv[1]).read_text()))


if __name__ == "__main__":
    main()
