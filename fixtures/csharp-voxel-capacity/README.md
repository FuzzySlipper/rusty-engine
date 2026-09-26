# Sixteen-material voxel capacity

Packaged C# fixture for task8667. One generated 32x2 texture, one atlas with
sixteen disjoint 2x2 regions, sixteen authored materials/surfaces and sixteen
base bindings render a 4x4 array of separated colored voxels in GreedyCubes.

Build with `RustyEngineFixtureSdkVersion=VERSION` and
`RestoreAdditionalProjectSources=FEED` using
`dotnet msbuild CsharpVoxelCapacity.csproj -restore -t:VerifyRustyEngineAot`.
Use the matching runtime with either CoreCLR or NativeAOT.

`capacity.inspect` reports sixteen retained materials. `capacity.reject` attempts
source slot65536, catches the Engine diagnostic naming the material and source
slot limit65535, then successfully refreshes the original projection. The native
allocator regression separately fills all65536 renderer slots and checks that
the next binding is identified with the exhausted capacity. No artificial
sixteen-material ceiling or downstream workaround is introduced.
