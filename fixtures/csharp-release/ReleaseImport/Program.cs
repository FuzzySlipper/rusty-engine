using Rusty.Engine;
using Rusty.Engine.Testing;

// Packs the folder a player names into release-data.rpak in the install
// directory: the one the product passes (Host.ReadInstalledContent), or the
// release root when a player runs it by hand. A release keeps this tool in
// tools/ReleaseImport/, two levels below the release root.
const string ContainerName = "release-data.rpak";

if (args.Length is < 1 or > 2)
{
    Console.Error.WriteLine("usage: ReleaseImport <folder> [<install directory>]");
    return 2;
}
string folder = Path.GetFullPath(args[0]);
if (!Directory.Exists(folder))
{
    Console.Error.WriteLine($"{folder} is not a folder");
    return 3;
}
string install = args.Length == 2 ? args[1] : Path.Combine(AppContext.BaseDirectory, "..", "..");
install = Path.GetFullPath(install);
Directory.CreateDirectory(install);
string output = Path.Combine(install, ContainerName);
Console.WriteLine($"Packing {folder}");
foreach (string file in Directory.EnumerateFiles(folder, "*", SearchOption.AllDirectories))
    Console.WriteLine($"  {Path.GetRelativePath(folder, file)}");
using var host = EngineTestHost.Create();
host.Call(engine => ProductContentBundle.PackContainer(engine.Content, folder, output, compress: true));
Console.WriteLine($"Packed {folder} into {output}");
return 0;
