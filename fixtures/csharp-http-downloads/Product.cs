using System.Text;
using System.Text.Json;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpHttpDownloads;

/// Reads a GitHub release index, downloads its module container through the
/// release CDN redirect into a library directory, and opens it as a content
/// bundle. The product picks the URLs and the library; the Engine owns the
/// transfers.
public sealed class Product(ProductCreateContext context) : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const string ReleaseIndexUrl = "https://api.github.com/repos/FuzzySlipper/rusty-engine/releases/tags/fixture-http-downloads-v1";
    private const string ModuleAsset = "fixture-module-1.0.0.rpak";
    private const string UserAgent = "rusty-engine-http-fixture/1";
    private const string GitHubJson = "application/vnd.github+json";
    // Nothing listens on port 1, so the connection is refused on any network.
    private const string UnreachableUrl = "https://127.0.0.1:1/index.json";
    private const ulong ProgressReportBytes = 16 * 1024 * 1024;

    private readonly IHttpService http = context.Engine.Http;
    private readonly string libraryPath = Path.Combine(Path.GetTempPath(), "rusty-http-downloads-fixture", "modules");
    private HttpLibrary? library;
    private HttpTransfer? index;
    private HttpTransfer? download;
    private HttpTransfer? failure;
    private string indexEtag = "";
    private string? assetUrl;
    private ulong reportedBytes;

    [DebugCommand("http.index")]
    public string Index()
    {
        index?.Dispose();
        index = http.Get(new HttpGetRequest(ReleaseIndexUrl, UserAgent, GitHubJson, BearerToken: "", IfNoneMatch: indexEtag));
        return $"index requested; if-none-match={(indexEtag == "" ? "none" : indexEtag)}";
    }

    [DebugCommand("http.download")]
    public string Download()
    {
        if (assetUrl is null) return "read the index first (http.index, then http.inspect)";
        library ??= http.OpenLibrary(new HttpLibraryOpenRequest(libraryPath));
        download?.Dispose();
        download = http.Download(new HttpDownloadRequest(library, ModuleAsset, assetUrl, UserAgent, "application/octet-stream", BearerToken: ""));
        reportedBytes = 0;
        return $"downloading {assetUrl} into {libraryPath}";
    }

    [DebugCommand("http.cancel")]
    public string Cancel()
    {
        if (download is null) return "no download";
        http.Cancel(download);
        return Describe("download", download);
    }

    [DebugCommand("http.fail")]
    public string Fail()
    {
        failure?.Dispose();
        failure = http.Get(new HttpGetRequest(UnreachableUrl, UserAgent, "", "", ""));
        return "requested an unreachable host";
    }

    [DebugCommand("http.inspect")]
    public string Inspect()
    {
        StringBuilder text = new();
        if (index is not null)
        {
            text.AppendLine(Describe("index", index));
            HttpTransferReadout readout = http.Read(index);
            if (readout.State == HttpTransferState.Completed)
            {
                string etag = Header(index, "etag");
                text.AppendLine($"  etag={etag} ratelimit-remaining={Header(index, "x-ratelimit-remaining")} ratelimit-reset={Header(index, "x-ratelimit-reset")}");
                if (readout.Status == 200)
                {
                    indexEtag = etag;
                    assetUrl = AssetUrl(http.ReadBody(index));
                    text.AppendLine($"  asset={assetUrl ?? "missing"}");
                }
                else if (readout.Status == 304)
                {
                    text.AppendLine("  not modified: the index is unchanged");
                }
            }
        }
        if (download is not null) text.AppendLine(Describe("download", download));
        if (failure is not null) text.AppendLine(Describe("failure probe", failure));
        return text.ToString().TrimEnd();
    }

    [DebugCommand("http.library")]
    public string Library()
    {
        library ??= http.OpenLibrary(new HttpLibraryOpenRequest(libraryPath));
        StringBuilder text = new($"library {libraryPath}:");
        foreach (HttpLibraryFile file in http.ReadLibraryFiles(library).Span)
            text.Append($" {file.FileName} ({file.ByteLength} bytes)");
        return text.ToString();
    }

    [DebugCommand("http.open")]
    public string Open()
    {
        using ContentBundle bundle = context.Engine.Content.OpenContainer(new ContentContainerOpenRequest(Path.Combine(libraryPath, ModuleAsset)));
        ContentSha256 identity = context.Engine.Content.ReadBundleIdentity(bundle);
        ReadOnlyMemory<ContentReferenceInfo> files = context.Engine.Content.ReadBundleFiles(bundle);
        ulong bytes = 0;
        foreach (ContentReferenceInfo file in files.Span) bytes += file.ByteLength;
        return $"opened {ModuleAsset}: {files.Length} files, {bytes} bytes, identity {identity.Word0:x16}{identity.Word1:x16}{identity.Word2:x16}{identity.Word3:x16}";
    }

    [DebugCommand("http.remove")]
    public string Remove()
    {
        library ??= http.OpenLibrary(new HttpLibraryOpenRequest(libraryPath));
        return $"removed={http.RemoveLibraryFile(new HttpLibraryFileRequest(library, ModuleAsset)).Removed}";
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        if (download is null) return ProductUpdateResult.None;
        HttpTransferReadout readout = http.Read(download);
        if (readout.State == HttpTransferState.Running && readout.ReceivedBytes >= reportedBytes + ProgressReportBytes)
        {
            reportedBytes = readout.ReceivedBytes;
            Console.WriteLine($"HTTP_FIXTURE_PROGRESS {readout.ReceivedBytes}/{readout.ExpectedBytes}");
        }
        return ProductUpdateResult.None;
    }

    private string Describe(string label, HttpTransfer transfer)
    {
        HttpTransferReadout readout = http.Read(transfer);
        string diagnostic = Encoding.UTF8.GetString(http.ReadDiagnostic(transfer).Span);
        return $"{label}: {readout.State} status={readout.Status} received={readout.ReceivedBytes}/{readout.ExpectedBytes}"
            + (readout.Failure == HttpFailure.None ? "" : $" failure={readout.Failure} ({diagnostic})");
    }

    private string Header(HttpTransfer transfer, string name)
    {
        foreach (HttpHeader header in http.ReadHeaders(transfer).Span)
            if (header.Name == name) return header.Value;
        return "";
    }

    private static string? AssetUrl(ReadOnlyMemory<byte> release)
    {
        using JsonDocument document = JsonDocument.Parse(release);
        foreach (JsonElement asset in document.RootElement.GetProperty("assets").EnumerateArray())
            if (asset.GetProperty("name").GetString() == ModuleAsset)
                return asset.GetProperty("browser_download_url").GetString();
        return null;
    }

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);
    public void Start() { }
    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        index?.Dispose();
        download?.Dispose();
        failure?.Dispose();
        library?.Dispose();
    }
}
