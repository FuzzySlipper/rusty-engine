using Rusty.Engine;
using Rusty.Engine.Testing;

// Packs the folder a player names into release-data.rpak beside
// product.rpak. A release keeps this tool in tools/ReleaseImport/, two
// levels below the release root.
const string ContainerName = "release-data.rpak";

if (args.Length != 1)
{
    Console.Error.WriteLine("usage: ReleaseImport <folder>");
    return 2;
}
string release = Path.GetFullPath(Path.Combine(AppContext.BaseDirectory, "..", ".."));
string output = Path.Combine(release, ContainerName);
using var host = EngineTestHost.Create();
host.Call(engine => ProductContentBundle.PackContainer(engine.Content, Path.GetFullPath(args[0]), output, compress: true));
Console.WriteLine($"Packed {args[0]} into {output}");
return 0;
