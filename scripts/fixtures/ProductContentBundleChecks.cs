using System;
using System.IO;
using System.Linq;
using Rusty.Engine;

internal static class ProductContentBundleChecks
{
    internal static void Run(ProductCreateContext context)
    {
        ProductContent content = context.Content;
        Require(content.Files.Length == 1 && content.ReadText("trial.txt").Contains("package-only"), "legacy snapshot excludes bundle bodies and inventory");
        Require(content.ListBundles().Single(bundle => bundle.Id == "rules").Id == "rules", "rules metadata discovery");
        ProductContentBundle bundle = content.OpenBundle("rules");
        Require(bundle.Entries.Length == 4, "file inventory");
        Require(bundle.ReadDirectory().Select(file => file.Name).SequenceEqual(new[] { "_index.json", "large.bin", "renamed.json" }), "directory boundary/order");
        Require(bundle.ReadDirectory(recursive: true).Length == 4, "recursive directory read");
        Require(!bundle.TryReadFile("Renamed.json", out _), "case-sensitive read");
        Require(bundle.ReadBytes("large.bin").Length == 1048589, "multi-chunk payload");
        ProductContentFile retained = bundle.ReadFile("renamed.json");
        ContentReference reference = bundle.OpenReference("renamed.json");
        bundle.Dispose();
        bundle.Dispose();
        Require(retained.ReadText().Contains("enemy"), "managed payload survives bundle close");
        Require(context.Engine.Content.ReadBytes(new(reference, 0, 1024)).Span.SequenceEqual(retained.Bytes.Span), "native reference survives bundle close");
        reference.Dispose();
        try { bundle.ReadFile("renamed.json"); throw new Exception("closed bundle accepted read"); }
        catch (ObjectDisposedException) { }
        using ProductContentBundle reopened = content.OpenBundle("rules");
        Require(reopened.ReadText("renamed.json") == retained.ReadText(), "reopen same built content");
        try { reopened.ReadFile("missing.json"); throw new Exception("missing file accepted"); }
        catch (FileNotFoundException) { }
    }

    private static void Require(bool condition, string description)
    {
        if (!condition) throw new InvalidOperationException("ProductContent bundle: " + description);
    }
}
