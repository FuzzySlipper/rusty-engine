using System.Reflection;
using System.Runtime.Loader;
using PublicApiGenerator;

// Print the public C# surface of one Rusty.Engine.dll, for release API diffs.
// Each assembly loads into its own context so any pair's SDK can be read.
if (args.Length != 1)
{
    Console.Error.WriteLine("usage: Rusty.Engine.ApiSurface <Rusty.Engine.dll>");
    return 2;
}

string path = Path.GetFullPath(args[0]);
Assembly assembly = new AssemblyLoadContext(path, isCollectible: true).LoadFromAssemblyPath(path);
Console.Out.Write(assembly.GeneratePublicApi(new ApiGeneratorOptions
{
    IncludeAssemblyAttributes = false,
}));
return 0;
