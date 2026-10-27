using System.Text;
using System.Text.Json.Nodes;
using Rusty.Engine;
using Rusty.Engine.Debugging;

namespace CsharpRelease;

/// <summary>
/// An entry screen for a product that cannot start without the container its
/// ReleaseImport tool packs. Import asks the host for a folder, runs the tool
/// on it while showing its output, then opens the container it installed,
/// saves the greeting it holds and continues.
/// </summary>
public sealed class Product(ProductCreateContext context) : IEngineProduct, IDebugCommandModuleSource, IDebugCommandModule
{
    private const string ContainerName = "release-data.rpak";
    private const string GreetingPath = "greeting.txt";
    private const string SaveScope = "release";
    private const string SaveKey = "greeting";
    private const string ImportTool = "ReleaseImport";
    private const string PickerTitle = "Choose the folder to import";
    private const string ImportIntent = "release.import";
    private const string CancelIntent = "release.cancel";
    private const string ScreenStream = "release";
    private const string ScreenContract = "release.screen.v1";

    private enum Phase { Missing, Choosing, Importing, Failed, Opened }

    private readonly IEngineContext engine = context.Engine;
    private UiStream? screen;
    private Phase phase = Phase.Missing;
    private FolderChoice? choice;
    private ToolRun? run;
    private ulong shownLines;
    private string output = "";
    private string message = "";
    private string greeting = "";
    private bool published;

    public void Start()
    {
        screen = engine.Ui.OpenStream(new UiStreamRequest(ScreenStream, ScreenContract));
        if (TryOpen()) return;
        message = $"{ContainerName} is not installed. Import a folder to make it.";
        Console.WriteLine($"RELEASE_FIXTURE_MISSING {engine.Host.ReadInstalledContentText()}");
    }

    public ProductUpdateResult Update(ProductUpdate update)
    {
        foreach (ProductInputEvent intent in update.Input)
        {
            if (IsIntent(intent, ImportIntent)) Import(folder: null);
            if (IsIntent(intent, CancelIntent)) Cancel();
        }
        bool changed = FollowChoice() | FollowRun();
        if (changed || !published) Publish();
        return ProductUpdateResult.None;
    }

    /// <summary>Import's action: with no folder the player chooses one.</summary>
    private void Import(string? folder)
    {
        if (phase is Phase.Choosing or Phase.Importing or Phase.Opened) return;
        if (folder is null)
        {
            choice = engine.Host.ChooseFolder(PickerTitle);
            phase = Phase.Choosing;
            message = "Choose the folder to import.";
        }
        else
        {
            StartTool(folder);
        }
        published = false;
    }

    private void Cancel()
    {
        if (run is null || phase != Phase.Importing) return;
        engine.Host.CancelToolRun(run);
    }

    private bool FollowChoice()
    {
        if (choice is null) return false;
        FolderChoiceReadout readout = engine.Host.ReadFolderChoice(choice);
        if (readout.State == FolderChoiceState.Pending) return false;
        string folder = engine.Host.ReadPathText(choice);
        string diagnostic = engine.Host.ReadDiagnosticText(choice);
        choice.Dispose();
        choice = null;
        switch (readout.State)
        {
            case FolderChoiceState.Chosen:
                StartTool(folder);
                break;
            case FolderChoiceState.Cancelled:
                phase = Phase.Missing;
                message = $"{ContainerName} is not installed. Import a folder to make it.";
                break;
            default:
                Fail($"No folder picker: {diagnostic}");
                break;
        }
        return true;
    }

    private void StartTool(string folder)
    {
        // The tool installs where the host opens relative containers.
        run = engine.Host.RunTool(ImportTool, folder, engine.Host.ReadInstalledContentText());
        phase = Phase.Importing;
        shownLines = 0;
        output = "";
        message = $"Importing {folder}…";
        Console.WriteLine($"RELEASE_FIXTURE_IMPORTING {folder}");
    }

    private bool FollowRun()
    {
        if (run is null) return false;
        ToolRunReadout readout = engine.Host.ReadToolRun(run);
        bool changed = readout.OutputLines != shownLines;
        if (changed)
        {
            output = engine.Host.ReadOutputText(run);
            shownLines = readout.OutputLines;
        }
        if (readout.State == ToolRunState.Running) return changed;
        run.Dispose();
        run = null;
        Console.WriteLine($"RELEASE_FIXTURE_TOOL {readout.State} {readout.ExitCode}");
        if (readout.State == ToolRunState.Exited && readout.ExitCode == 0)
        {
            if (!TryOpen()) Fail($"The import finished, but {ContainerName} did not open: {message}");
        }
        else
        {
            Fail(readout.State switch
            {
                ToolRunState.Exited => $"The import failed (exit code {readout.ExitCode}).",
                ToolRunState.Cancelled => "The import was cancelled.",
                _ => "The import tool could not start.",
            });
        }
        return true;
    }

    private bool TryOpen()
    {
        try
        {
            using ProductContentBundle data = ProductContentBundle.OpenContainer(engine.Content, ContainerName);
            greeting = data.ReadText(GreetingPath).Trim();
        }
        catch (EngineCallException error)
        {
            message = error.Message;
            return false;
        }
        IPersistenceService persistence = engine.Persistence;
        using PersistenceStore store = persistence.OpenStore(new(SaveScope));
        persistence.Save(new(store, SaveKey, PersistenceRevisionGuard.Any, 0, Encoding.UTF8.GetBytes(greeting)));
        phase = Phase.Opened;
        message = $"Opened {ContainerName}.";
        Console.WriteLine($"RELEASE_FIXTURE_OPENED {greeting}");
        published = false;
        return true;
    }

    private void Fail(string reason)
    {
        phase = Phase.Failed;
        message = reason;
        Console.WriteLine($"RELEASE_FIXTURE_FAILED {reason}");
    }

    private JsonObject Screen() => new()
    {
        ["phase"] = phase.ToString().ToLowerInvariant(),
        ["message"] = message,
        ["output"] = output,
        ["greeting"] = greeting,
    };

    private void Publish()
    {
        if (screen is null) return;
        engine.Ui.PublishProjection(new UiProjection(screen, UiValues.FromJson(Screen())));
        published = true;
    }

    private static bool IsIntent(ProductInputEvent intent, string id) =>
        intent.Kind == InputEventKind.DirectProductPayload && intent.Intent.Span.SequenceEqual(Encoding.UTF8.GetBytes(id));

    public void RegisterDebugCommands(IDebugCommandModuleRegistrar registrar) => registrar.Register(this);

    [DebugCommand("release.observe", Description = "Read-only: the entry screen's phase, message, the tool's recent output and the opened greeting.")]
    public string Observe() => Screen().ToJsonString();

    [DebugCommand("release.import", Description = "Do what Import does: open the folder picker.")]
    public string ImportCommand()
    {
        Import(folder: null);
        return Observe();
    }

    [DebugCommand("release.import_folder", Description = "Do what Import does once the player chose this folder: run the tool on it.")]
    public string ImportFolderCommand(string folder)
    {
        Import(folder);
        return Observe();
    }

    [DebugCommand("release.cancel", Description = "Do what Cancel does: stop a running import.")]
    public string CancelCommand()
    {
        Cancel();
        return Observe();
    }

    public void Pause() { }
    public void Resume() { }
    public void Restart() { }
    public void Shutdown() { }

    public void Dispose()
    {
        run?.Dispose();
        choice?.Dispose();
        screen?.Dispose();
    }
}
