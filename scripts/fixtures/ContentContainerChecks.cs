#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using Rusty.Engine;

// Opens two independently packed content containers from a library directory
// the product chose, and composes across them as the product decides: a
// ruleset module names the asset module it requires and the assets it uses.
// The same checks run in a product and in a tool's EngineTestHost.
internal static class ContentContainerChecks
{
    private const string ModuleManifest = "module.json";

    public static void Run(IEngineContext engine, string library)
    {
        var modules = new Dictionary<string, ProductContentBundle>(StringComparer.Ordinal);
        try
        {
            // Opening reads only the inventory, so opening every installed
            // container to read its manifest is the discovery step.
            foreach (string path in Directory.GetFiles(library, "*.rpak"))
            {
                ProductContentBundle module = ProductContentBundle.OpenContainer(engine.Content, path);
                using JsonDocument manifest = JsonDocument.Parse(module.ReadText(ModuleManifest));
                modules.Add(manifest.RootElement.GetProperty("id").GetString()!, module);
            }
            Require(modules.Count == 2, $"expected two installed modules, found {modules.Count}");

            ProductContentBundle ruleset = modules["srd"];
            using JsonDocument rulesetManifest = JsonDocument.Parse(ruleset.ReadText(ModuleManifest));
            ProductContentBundle assets = modules[rulesetManifest.RootElement.GetProperty("requires")[0].GetString()!];
            Require(!ruleset.Identity.Equals(assets.Identity), "two different modules share an identity");
            Require(ruleset.Entries.Length == 2 && assets.Entries.Length == 4, "module inventories");

            using JsonDocument classes = JsonDocument.Parse(ruleset.ReadText("rules/classes.json"));
            JsonElement fighter = classes.RootElement.GetProperty("fighter");

            using (ContentReference portrait = assets.OpenReference(fighter.GetProperty("portrait").GetString()!))
            {
                RenderResourceInfo texture = engine.Graphics.OpenResourceFromContent(
                    new RenderResourceContentRequest(portrait, TextureFilter.Nearest, TextureWrap.Clamp));
                Require(texture.Kind == RenderResourceKind.Texture, "the portrait opened as a texture");
                texture.Handle.Dispose();
            }

            ContentReference model = assets.OpenReference(fighter.GetProperty("model").GetString()!);
            ContentSha256 identity = assets.Identity;
            string assetsPath = assets.Id;
            assets.Dispose();
            modules.Remove("walls");
            // The reference outlives its container's bundle, and the GLB's
            // relative image still resolves inside that container.
            using (model)
            using (RenderResource mesh = engine.Animation.OpenAnimatedMeshFromContent(new AnimationContentRequest(model)))
                Require(engine.Animation.ReadMeshInfo(mesh).MaterialCount > 0, "the module's animated mesh admitted");

            using (ProductContentBundle reopened = ProductContentBundle.OpenContainer(engine.Content, assetsPath))
                Require(reopened.Identity.Equals(identity), "a reopened container changed identity");

            PackInProcess(engine, ruleset);

            string notes = Path.Combine(library, "notes.txt");
            try
            {
                ProductContentBundle.OpenContainer(engine.Content, notes).Dispose();
                throw new InvalidOperationException("a text file opened as a container");
            }
            catch (EngineCallException refusal) when (refusal.Diagnostics.Length == 1
                && refusal.Diagnostics.Span[0].Code == "PRODUCT_CONTAINER_NOT_A_CONTAINER"
                && refusal.Diagnostics.Span[0].Message.Contains(notes, StringComparison.Ordinal))
            {
            }

            // An entry that fails to decompress refuses its read with the
            // container's code, naming the container and the entry.
            string broken = Path.Combine(library, "broken.container");
            using ProductContentBundle damaged = ProductContentBundle.OpenContainer(engine.Content, broken);
            Require(damaged.ReadText(ModuleManifest).Contains("broken", StringComparison.Ordinal), "a readable entry of a damaged container");
            try
            {
                damaged.ReadText("data/table.json");
                throw new InvalidOperationException("an entry that does not decompress was read");
            }
            catch (EngineCallException refusal) when (refusal.Diagnostics.Length == 1
                && refusal.Diagnostics.Span[0].Code == "PRODUCT_CONTAINER_CORRUPT"
                && refusal.Diagnostics.Span[0].Message.Contains(broken, StringComparison.Ordinal)
                && refusal.Diagnostics.Span[0].Message.Contains("data/table.json", StringComparison.Ordinal))
            {
            }
        }
        finally
        {
            foreach (ProductContentBundle module in modules.Values) module.Dispose();
        }
    }

    // The srd module's files (as make-content-modules.sh writes them), packed
    // in process, give the container `rusty pack-content` made from them.
    private static void PackInProcess(IEngineContext engine, ProductContentBundle packedByRusty)
    {
        string root = Directory.CreateTempSubdirectory("rusty-pack-content-").FullName;
        try
        {
            string module = Path.Combine(root, "srd");
            Directory.CreateDirectory(Path.Combine(module, "rules"));
            File.WriteAllText(Path.Combine(module, ModuleManifest), "{\"id\":\"srd\",\"requires\":[\"walls\"]}");
            File.WriteAllText(Path.Combine(module, "rules", "classes.json"),
                "{\"fighter\":{\"portrait\":\"portraits/fighter.png\",\"model\":\"models/character.glb\"}}");
            foreach (bool compress in new[] { false, true })
            {
                string output = Path.Combine(root, $"srd-{compress}.rpak");
                ProductContentBundle.PackContainer(engine.Content, module, output, compress);
                using ProductContentBundle packed = ProductContentBundle.OpenContainer(engine.Content, output);
                Require(packed.Identity.Equals(packedByRusty.Identity), "an in-process pack differs from rusty pack-content's");
            }
            try
            {
                ProductContentBundle.PackContainer(engine.Content, module, Path.Combine(module, "inside.rpak"));
                throw new InvalidOperationException("a container packed into its own directory");
            }
            catch (EngineCallException refusal) when (refusal.Diagnostics.Length == 1
                && refusal.Diagnostics.Span[0].Code == "PRODUCT_PACK_OVERLAP")
            {
            }
        }
        finally
        {
            Directory.Delete(root, recursive: true);
        }
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
