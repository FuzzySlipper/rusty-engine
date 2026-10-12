using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using Microsoft.Build.Framework;
using Microsoft.Build.Utilities;

public sealed class RustyEngineContentBundleManifestTask : Task
{
    private const string ManifestFileName = ".rusty-bundles.json";

    [Required]
    public string ContentRoot { get; set; } = string.Empty;

    [Required]
    public string StagedContentRoot { get; set; } = string.Empty;

    public ITaskItem[] Bundles { get; set; } = Array.Empty<ITaskItem>();

    public override bool Execute()
    {
        if (Bundles.Length == 0)
        {
            return true;
        }

        var sourceContentRoot = Path.GetFullPath(ContentRoot);
        var stagedContentRoot = Path.GetFullPath(StagedContentRoot);
        if (!Directory.Exists(sourceContentRoot))
        {
            Log.LogError("RustyEngineProductContentRoot does not exist: {0}", sourceContentRoot);
            return false;
        }

        if (!Directory.Exists(stagedContentRoot))
        {
            Log.LogError("Rusty Engine did not stage ProductContent at {0}", stagedContentRoot);
            return false;
        }

        var sourceManifestPath = Path.Combine(sourceContentRoot, ManifestFileName);
        if (File.Exists(sourceManifestPath))
        {
            Log.LogError(
                "ProductContent reserves {0} for the Engine-generated bundle inventory; remove the input file at {1}.",
                ManifestFileName,
                sourceManifestPath);
            return false;
        }

        var stagedManifestPath = Path.Combine(stagedContentRoot, ManifestFileName);
        if (File.Exists(stagedManifestPath))
        {
            Log.LogError(
                "ProductContent copied a reserved bundle inventory input to {0}; remove {1} from RustyEngineProductContentRoot.",
                stagedManifestPath,
                ManifestFileName);
            return false;
        }

        var bundleDefinitions = new List<BundleDefinition>();
        var bundleIds = new HashSet<string>(StringComparer.Ordinal);
        foreach (var bundle in Bundles)
        {
            var id = bundle.ItemSpec;
            if (!bundleIds.Add(id))
            {
                Log.LogError("RustyEngineContentBundle ID '{0}' is declared more than once.", id);
                continue;
            }

            var root = bundle.GetMetadata("Root");
            if (string.IsNullOrEmpty(root))
            {
                root = id;
            }

            // The inventory records content paths with '/' on every OS.
            root = root.Replace('\\', '/');
            var sourceRoot = Path.GetFullPath(Path.Combine(sourceContentRoot, root));
            var stagedRoot = Path.GetFullPath(Path.Combine(stagedContentRoot, root));

            if (!Directory.Exists(sourceRoot))
            {
                Log.LogError("RustyEngineContentBundle '{0}' Root does not exist under RustyEngineProductContentRoot: {1}", id, sourceRoot);
                continue;
            }

            if (!Directory.Exists(stagedRoot))
            {
                Log.LogError("RustyEngineContentBundle '{0}' Root was not staged under content: {1}", id, stagedRoot);
                continue;
            }

            bundleDefinitions.Add(new BundleDefinition(id, root, stagedRoot));
        }

        if (Log.HasLoggedErrors)
        {
            return false;
        }

        bundleDefinitions.Sort((left, right) => StringComparer.Ordinal.Compare(left.Id, right.Id));
        var manifest = new StringBuilder();
        manifest.Append("{\"bundles\":[");
        for (var bundleIndex = 0; bundleIndex < bundleDefinitions.Count; bundleIndex++)
        {
            if (bundleIndex > 0)
            {
                manifest.Append(',');
            }

            var bundle = bundleDefinitions[bundleIndex];
            manifest.Append("{\"id\":");
            AppendJsonString(manifest, bundle.Id);
            manifest.Append(",\"root\":");
            AppendJsonString(manifest, bundle.Root);
            manifest.Append(",\"files\":[");

            var files = new List<string>(Directory.EnumerateFiles(bundle.StagedRoot, "*", SearchOption.AllDirectories));
            files.Sort((left, right) => StringComparer.Ordinal.Compare(
                ToBundleRelativePath(bundle.StagedRoot, left),
                ToBundleRelativePath(bundle.StagedRoot, right)));

            for (var fileIndex = 0; fileIndex < files.Count; fileIndex++)
            {
                if (fileIndex > 0)
                {
                    manifest.Append(',');
                }

                var filePath = files[fileIndex];
                var relativePath = ToBundleRelativePath(bundle.StagedRoot, filePath);
                manifest.Append("{\"path\":");
                AppendJsonString(manifest, relativePath);
                manifest.Append(",\"byteLength\":");
                manifest.Append(new FileInfo(filePath).Length.ToString(CultureInfo.InvariantCulture));
                manifest.Append(",\"sha256\":");
                AppendJsonString(manifest, ComputeSha256(filePath));
                manifest.Append('}');
            }

            manifest.Append("]}");
        }

        manifest.Append("]}");
        File.WriteAllText(stagedManifestPath, manifest.ToString(), new UTF8Encoding(false));
        return true;
    }

    private static string ToBundleRelativePath(string bundleRoot, string path)
    {
        return path.Substring(bundleRoot.Length).TrimStart(Path.DirectorySeparatorChar).Replace(Path.DirectorySeparatorChar, '/');
    }

    private static string ComputeSha256(string path)
    {
        using var stream = File.OpenRead(path);
        using var algorithm = SHA256.Create();
        var bytes = algorithm.ComputeHash(stream);
        var builder = new StringBuilder(bytes.Length * 2);
        foreach (var value in bytes)
        {
            builder.Append(value.ToString("x2", CultureInfo.InvariantCulture));
        }

        return builder.ToString();
    }

    private static void AppendJsonString(StringBuilder builder, string value)
    {
        builder.Append('"');
        foreach (var character in value)
        {
            switch (character)
            {
                case '"': builder.Append("\\\""); break;
                case '\\': builder.Append("\\\\"); break;
                case '\b': builder.Append("\\b"); break;
                case '\f': builder.Append("\\f"); break;
                case '\n': builder.Append("\\n"); break;
                case '\r': builder.Append("\\r"); break;
                case '\t': builder.Append("\\t"); break;
                default:
                    if (character < ' ')
                    {
                        builder.Append("\\u");
                        builder.Append(((int)character).ToString("x4", CultureInfo.InvariantCulture));
                    }
                    else
                    {
                        builder.Append(character);
                    }

                    break;
            }
        }

        builder.Append('"');
    }

    private sealed class BundleDefinition
    {
        public BundleDefinition(string id, string root, string stagedRoot)
        {
            Id = id;
            Root = root;
            StagedRoot = stagedRoot;
        }

        public string Id { get; }

        public string Root { get; }

        public string StagedRoot { get; }
    }
}
