using System.Globalization;
using System.Text;
using System.Text.Json;
using Rusty.Engine;

// Deliberately shuffled input, a prefix neighbour, nested content and Unicode:
// directory reads must not depend on host discovery order or current culture.
ProductContent content = new(new ProductContentFile[]
{
    File("enemies/nested/boss.json", "{\"id\":\"boss\"}"),
    File("enemies/z.json", "{\"id\":\"rat\"}"),
    File("enemies-other/stray.json", "{}"),
    File("root.json", "{}"),
    File("enemies/_index.json", "{\"order\":[\"rat\",\"skeleton\"]}"),
    File("enemies/ä.json", "{\"id\":\"skeleton\"}"),
    File("empty.bin", ""),
    File("bom.txt", "\uFEFFhello"),
});
CultureInfo.CurrentCulture = CultureInfo.GetCultureInfo("tr-TR");
ProductContentFile[] files = content.ReadDirectory("enemies");
Assert(files.Select(file => file.Name).SequenceEqual(new[] { "_index.json", "z.json", "ä.json" }), "ordinal directory order");
Assert(content.ReadDirectory("enemies/").SequenceEqual(files), "optional trailing slash");
Assert(content.ReadDirectory("enemies", recursive: true).Length == 4, "explicit recursion");
Assert(content.ReadDirectory().Length == 3 && content.ReadDirectory(recursive: true).Length == 8, "root selection");
Assert(content.ReadDirectory("missing").Length == 0 && content.ReadDirectory("enemies/z.json").Length == 0, "missing directory");
Assert(!content.TryReadFile("Enemies/z.json", out _), "case-sensitive lookup");
Assert(content.TryReadFile("empty.bin", out var empty) && empty.Bytes.IsEmpty, "empty file is present");
Assert(content.ReadText("bom.txt") == "hello", "UTF-8 BOM");
Assert(content.ReadBytes("enemies/z.json").Span.SequenceEqual(Encoding.UTF8.GetBytes("{\"id\":\"rat\"}")), "named byte read");
Assert(content.Files.Span[0].RelativePath == "enemies/nested/boss.json", "legacy input order preserved");
try { content.ReadFile("absent.json"); throw new Exception("missing file did not throw"); }
catch (FileNotFoundException error) { Assert(error.FileName == "absent.json", "missing path diagnostics"); }

// The index selects definition IDs, so filenames can change without code changes.
using JsonDocument index = JsonDocument.Parse(files.Single(file => file.Name == "_index.json").Bytes);
Dictionary<string, ProductContentFile> definitions = [];
foreach (ProductContentFile file in files.Where(file => file.Name != "_index.json"))
{
    using JsonDocument definition = JsonDocument.Parse(file.Bytes);
    definitions.Add(definition.RootElement.GetProperty("id").GetString()!, file);
}
string[] authoredOrder = index.RootElement.GetProperty("order").EnumerateArray()
    .Select(id => definitions[id.GetString()!].Name).ToArray();
Assert(authoredOrder.SequenceEqual(new[] { "z.json", "ä.json" }), "index resolves IDs independently of filenames");
Console.WriteLine("ProductContent named reads, directory boundaries/order/recursion, UTF-8 and authored index: passed.");

static ProductContentFile File(string path, string text) => new(Encoding.UTF8.GetBytes(path), Encoding.UTF8.GetBytes(text));
static void Assert(bool condition, string label) { if (!condition) throw new Exception(label); }
