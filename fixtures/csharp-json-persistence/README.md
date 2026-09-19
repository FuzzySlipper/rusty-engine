# JSON persistence NativeAOT proof

Engine-owned fidelity proof for `JsonProductStateCodec<T>` (task #8319): a minimal
`ProductStateStore` consumer whose only serialization metadata is a source-generated
`JsonSerializerContext`. No reflection-based serialization is used anywhere on this path.

```bash
dotnet publish fixtures/csharp-json-persistence -c Release
./fixtures/csharp-json-persistence/bin/Release/net10.0/linux-x64/publish/CsharpJsonPersistence
```

Exit 0 means the proof held: successful save receipt, nested/collection roundtrip through
`ProductStateStore`, absent-key behavior, and an understandable `JsonException` on invalid
bytes — all AOT-compiled with trimming.
