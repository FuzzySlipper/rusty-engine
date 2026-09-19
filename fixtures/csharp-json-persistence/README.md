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

This standalone fixture uses an in-memory service: it proves serialization and
NativeAOT trimming, not disk storage or the native binding. For the real path run
`scripts/test-csharp-release-pair.sh <pair.tar.gz> --aot`. Its packaged Product
uses the shared `scripts/fixtures/JsonPersistenceChecks.cs` exercise through both
CoreCLR and NativeAOT hosts, saving and reopening disk state beneath temporary
host-selected roots, with missing-key and malformed-JSON checks.
