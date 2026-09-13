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
            if (!IsSafeBundleId(id))
            {
                Log.LogError(
                    "RustyEngineContentBundle ID '{0}' is unsafe. IDs must start with an ASCII letter or digit and contain only ASCII letters, digits, '.', '_', or '-'.",
                    id);
                continue;
            }

            if (!bundleIds.Add(id))
            {
                Log.LogError("RustyEngineContentBundle ID '{0}' is declared more than once.", id);
                continue;
            }

            var declaredRoot = bundle.GetMetadata("Root");
            if (string.IsNullOrEmpty(declaredRoot))
            {
                declaredRoot = id;
            }

            if (!TryCanonicalizeRelativeRoot(declaredRoot, out var root, out var rootError))
            {
                Log.LogError("RustyEngineContentBundle '{0}' has an unsafe Root '{1}': {2}", id, declaredRoot, rootError);
                continue;
            }

            var sourceRoot = Path.GetFullPath(Path.Combine(sourceContentRoot, root));
            var stagedRoot = Path.GetFullPath(Path.Combine(stagedContentRoot, root));
            if (!IsUnderRoot(sourceRoot, sourceContentRoot) || !IsUnderRoot(stagedRoot, stagedContentRoot))
            {
                Log.LogError("RustyEngineContentBundle '{0}' Root '{1}' must stay under RustyEngineProductContentRoot.", id, declaredRoot);
                continue;
            }

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

        for (var left = 0; left < bundleDefinitions.Count; left++)
        {
            for (var right = left + 1; right < bundleDefinitions.Count; right++)
            {
                if (IsSameOrDescendantRoot(bundleDefinitions[left].Root, bundleDefinitions[right].Root) ||
                    IsSameOrDescendantRoot(bundleDefinitions[right].Root, bundleDefinitions[left].Root))
                {
                    Log.LogError(
                        "RustyEngineContentBundle roots overlap: '{0}' ({1}) and '{2}' ({3}). Each bundle must own a nonoverlapping ProductContent directory.",
                        bundleDefinitions[left].Id,
                        bundleDefinitions[left].Root,
                        bundleDefinitions[right].Id,
                        bundleDefinitions[right].Root);
                }
            }
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

    private static bool IsSafeBundleId(string value)
    {
        if (string.IsNullOrEmpty(value) || !IsAsciiLetterOrDigit(value[0]))
        {
            return false;
        }

        for (var index = 1; index < value.Length; index++)
        {
            var character = value[index];
            if (!IsAsciiLetterOrDigit(character) && character != '.' && character != '_' && character != '-')
            {
                return false;
            }
        }

        return true;
    }

    private static bool TryCanonicalizeRelativeRoot(string value, out string root, out string error)
    {
        root = string.Empty;
        error = string.Empty;
        if (string.IsNullOrWhiteSpace(value) || Path.IsPathRooted(value))
        {
            error = "the path must be a nonempty relative directory";
            return false;
        }

        var segments = value.Replace('\\', '/').Split('/');
        var canonicalSegments = new List<string>(segments.Length);
        foreach (var segment in segments)
        {
            if (string.IsNullOrEmpty(segment) || segment == "." || segment == "..")
            {
                error = "the path cannot contain empty, '.' or '..' segments";
                return false;
            }

            foreach (var character in segment)
            {
                if (character < ' ' || character == '<' || character == '>' || character == ':' || character == '"' ||
                    character == '|' || character == '?' || character == '*')
                {
                    error = "the path contains an unsafe directory character";
                    return false;
                }
            }

            canonicalSegments.Add(segment);
        }

        root = string.Join("/", canonicalSegments);
        return true;
    }

    private static bool IsUnderRoot(string path, string root)
    {
        var comparison = Path.DirectorySeparatorChar == '\\' ? StringComparison.OrdinalIgnoreCase : StringComparison.Ordinal;
        var normalizedRoot = root.EndsWith(Path.DirectorySeparatorChar.ToString(), StringComparison.Ordinal)
            ? root
            : root + Path.DirectorySeparatorChar;
        return path.StartsWith(normalizedRoot, comparison);
    }

    private static bool IsSameOrDescendantRoot(string root, string possibleParent)
    {
        return string.Equals(root, possibleParent, StringComparison.Ordinal) ||
               root.StartsWith(possibleParent + "/", StringComparison.Ordinal);
    }

    private static string ToBundleRelativePath(string bundleRoot, string path)
    {
        var normalizedRoot = bundleRoot.EndsWith(Path.DirectorySeparatorChar.ToString(), StringComparison.Ordinal)
            ? bundleRoot
            : bundleRoot + Path.DirectorySeparatorChar;
        if (!path.StartsWith(normalizedRoot, StringComparison.Ordinal))
        {
            throw new InvalidOperationException("Staged bundle file escaped its declared root.");
        }

        return path.Substring(normalizedRoot.Length).Replace(Path.DirectorySeparatorChar, '/');
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

    private static bool IsAsciiLetterOrDigit(char value)
    {
        return (value >= 'a' && value <= 'z') ||
               (value >= 'A' && value <= 'Z') ||
               (value >= '0' && value <= '9');
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
