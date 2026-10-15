using System.Globalization;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;

namespace Rusty.Engine.Debugging;

/// <summary>How the tunables panel draws a control and what its value means.</summary>
public enum TunableKind
{
    /// <summary>A number from <see cref="TunableControl.Min"/> to <see cref="TunableControl.Max"/> in <see cref="TunableControl.Step"/>s.</summary>
    Slider,
    /// <summary>0 or 1.</summary>
    Toggle,
    /// <summary>The index of one of <see cref="TunableControl.Options"/>.</summary>
    Choice,
}

/// <summary>
/// Where a control's value is saved: a content-root-relative JSON file and a JSON pointer into it.
/// A boolean there is a toggle's 0 or 1; a string is a choice's option.
/// </summary>
public sealed record TunableSource(string Path, string Pointer);

/// <summary>One control as the panel draws it, with its current live value.</summary>
public sealed record TunableControl(string Id, string Label, TunableKind Kind, float Min, float Max, float Step, float Value)
{
    /// <summary>A choice's options, in index order.</summary>
    public IReadOnlyList<string>? Options { get; init; }
    /// <summary>Whether a change reshapes the page, so the panel describes again after one.</summary>
    public bool Reshapes { get; init; }
    /// <summary>Where <c>tunables.save</c> writes the value and <c>tunables.reset</c> reads it, if anywhere.</summary>
    public TunableSource? Source { get; init; }
}

/// <summary>A control and the owner call that applies a value to the live product.</summary>
public sealed record Tunable(TunableControl Control, Action<float> Set);

/// <summary>A titled group of tunables; <see cref="File"/> names the content file they mostly save to.</summary>
public sealed record TunableGroup(string Id, string Title, IReadOnlyList<Tunable> Tunables)
{
    public string? File { get; init; }
}

/// <summary>A choice that selects what the page shows, such as which item is being tuned.</summary>
public sealed record TunableChooser(string Id, string Label, string Value, IReadOnlyList<string> Options);

/// <summary>One key of a timeline, at <see cref="Time"/> seconds.</summary>
public sealed record TunableKey(int Index, string Label, float Time);

/// <summary>
/// A timeline the panel draws as key buttons, a scrubber and play speeds, sending the page's
/// <c>key</c>, <c>at</c> and <c>play</c> commands. <see cref="Key"/> is the held key.
/// </summary>
public sealed record TunableTimeline(IReadOnlyList<TunableKey> Keys, float Duration, int Key)
{
    public IReadOnlyList<float> Speeds { get; init; } = [0.25f, 0.5f, 1f];
}

/// <summary>
/// What the tunables panel shows now. <see cref="Commands"/> adds the product's own command
/// templates (<c>key {key}</c>, <c>at {seconds}</c>, <c>play {speed}</c>, <c>close</c>) to the module's.
/// </summary>
public sealed record TunablesPage(string Title, IReadOnlyList<TunableGroup> Groups)
{
    public IReadOnlyList<TunableChooser> Choosers { get; init; } = [];
    public TunableTimeline? Timeline { get; init; }
    public string Notice { get; init; } = "";
    public IReadOnlyDictionary<string, string> Commands { get; init; } = new Dictionary<string, string>();
}

/// <summary>
/// Serves the Engine tunables panel (<c>mountTunablesPanel</c>) and agents from the product's own explicit
/// declarations: <c>tunables.describe</c> answers the <c>tunables.v1</c> description, <c>tunables.select</c>
/// changes a chooser, <c>tunables.set</c> and <c>tunables.set-many</c> apply values, and <c>tunables.diff</c>,
/// <c>tunables.save</c> and <c>tunables.reset</c> compare, save and restore bound values through the content
/// source. Register one instance with the debug catalog.
/// </summary>
public sealed class TunablesDebugModule(IContentSourceService source, Func<TunablesPage> describe, Func<string, string, string?>? select = null) : IDebugCommandModule
{
    public const string Contract = "tunables.v1";
    private readonly IContentSourceService _source = source ?? throw new ArgumentNullException(nameof(source));
    private readonly Func<TunablesPage> _describe = describe ?? throw new ArgumentNullException(nameof(describe));

    [DebugCommand("tunables.describe", Description = "The tunables.v1 description of what the tunables panel shows now: choosers, timeline, groups of controls with live values and source bindings, and command templates.")]
    public DebugCommandResult Describe() => DebugCommandResult.Success(Json(_describe()));

    [DebugCommand("tunables.select", Description = "Sets a chooser to one of its options, then describes again.")]
    public DebugCommandResult Select(string chooser, string value)
    {
        if (select is null) return Invalid("This page has no choosers.");
        string? refusal = select(chooser, value);
        return refusal is null ? Describe() : Invalid(refusal);
    }

    [DebugCommand("tunables.set", Description = "Applies one control's value to the live product, clamped to its range; answers the value kept.")]
    public DebugCommandResult Set(string id, float value)
    {
        Applied applied = Apply(_describe(), id, value.ToString(CultureInfo.InvariantCulture));
        return applied.Refusal is { } refusal ? Invalid(refusal) : DebugCommandResult.Success(applied.Kept.ToString(CultureInfo.InvariantCulture));
    }

    [DebugCommand("tunables.set-many", Description = "Applies id=value pairs, comma separated (a choice may name its option), in order: answers each control's value kept or its refusal, then the description. A refused control leaves the others applied.")]
    public DebugCommandResult SetMany(string pairs)
    {
        List<(string Id, string Value)> batch = [];
        foreach (string pair in pairs.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries))
        {
            int at = pair.IndexOf('=');
            if (at <= 0) return Invalid($"'{pair}' is not id=value.");
            batch.Add((pair[..at], pair[(at + 1)..]));
        }
        return Applying(batch);
    }

    [DebugCommand("tunables.diff", Description = "Per bound file, each control whose live value differs from the file's (floats at half a step) with its live and saved values and what Save writes, the file's SHA-256 for tunables.save, and the page shown.")]
    public DebugCommandResult Diff()
    {
        TunablesPage page = _describe();
        return DebugCommandResult.Success(Write(writer =>
        {
            writer.WriteStartObject();
            Page(writer, page);
            writer.WritePropertyName("files");
            writer.WriteStartArray();
            foreach (string path in BoundFiles(page))
            {
                writer.WriteStartObject();
                writer.WriteString("path", path);
                if (Read(path, out ContentSourceDocument document, out JsonDocument? json) is { } refusal)
                {
                    writer.WriteString("error", refusal);
                    writer.WriteEndObject();
                    continue;
                }
                using (json)
                {
                    writer.WriteString("sha256", Hex(document.Sha256));
                    writer.WritePropertyName("unsaved");
                    writer.WriteStartArray();
                    foreach (Unsaved unsaved in UnsavedIn(page, path, json!.RootElement))
                    {
                        writer.WriteStartObject();
                        writer.WriteString("id", unsaved.Control.Id);
                        writer.WriteString("label", unsaved.Control.Label);
                        writer.WriteString("pointer", unsaved.Control.Source!.Pointer);
                        writer.WriteNumber("live", unsaved.Control.Value);
                        writer.WriteNumber("saved", unsaved.Saved);
                        writer.WritePropertyName("save");
                        Written(unsaved.Control, unsaved.Previous).WriteTo(writer);
                        writer.WriteEndObject();
                    }
                    writer.WriteEndArray();
                }
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WriteEndObject();
        }));
    }

    [DebugCommand("tunables.save", Description = "Saves the file's unsaved controls (only their pointers, at each control's step) if it still has the SHA-256 its diff answered; refused with CONTENT_SOURCE_STALE after it changed. Answers the pointers saved and the new SHA-256.")]
    public DebugCommandResult Save(string path, string sha256)
    {
        if (!TryParseSha(sha256, out ContentSha256 expected)) return Invalid("sha256 must be the 64 hex digits tunables.diff answered.");
        TunablesPage page = _describe();
        if (Read(path, out ContentSourceDocument _, out JsonDocument? json) is { } refusal) return Failed(refusal);
        List<ContentSourceEdit> edits = [];
        using (json)
        {
            foreach (Unsaved unsaved in UnsavedIn(page, path, json!.RootElement))
                edits.Add(ContentSourceEdit.Replace(unsaved.Control.Source!.Pointer, Written(unsaved.Control, unsaved.Previous)));
        }
        ContentSha256 saved = expected;
        if (edits.Count > 0)
        {
            try
            {
                saved = _source.Edit(new ContentSourceEditRequest(path, expected, ContentSourceConvenience.Patch([.. edits])));
            }
            catch (EngineCallException exception)
            {
                return Failed(Refusal(exception));
            }
        }
        return DebugCommandResult.Success(Write(writer =>
        {
            writer.WriteStartObject();
            writer.WriteString("path", path);
            writer.WriteString("sha256", Hex(saved));
            writer.WritePropertyName("saved");
            writer.WriteStartArray();
            foreach (ContentSourceEdit edit in edits) writer.WriteStringValue(edit.Pointer);
            writer.WriteEndArray();
            writer.WriteEndObject();
        }));
    }

    [DebugCommand("tunables.reset", Description = "Sets every unsaved control bound to the file back to the file's value; answers each control's value kept or its refusal, then the description.")]
    public DebugCommandResult Reset(string path)
    {
        if (Read(path, out ContentSourceDocument _, out JsonDocument? json) is { } refusal) return Failed(refusal);
        List<(string Id, string Value)> batch = [];
        using (json)
        {
            foreach (Unsaved unsaved in UnsavedIn(_describe(), path, json!.RootElement))
                batch.Add((unsaved.Control.Id, unsaved.Saved.ToString(CultureInfo.InvariantCulture)));
        }
        return Applying(batch);
    }

    private readonly record struct Applied(string Id, float Kept, string? Refusal);

    private readonly record struct Unsaved(TunableControl Control, float Saved, JsonElement Previous);

    /// <summary>Applies a batch in order, describing again after a control that reshapes the page.</summary>
    private DebugCommandResult Applying(IReadOnlyList<(string Id, string Value)> batch)
    {
        TunablesPage page = _describe();
        List<Applied> results = [];
        foreach ((string id, string value) in batch)
        {
            Applied applied = Apply(page, id, value);
            results.Add(applied);
            if (applied.Refusal is null && Find(page, id) is { Control.Reshapes: true }) page = _describe();
        }
        page = _describe();
        return DebugCommandResult.Success(Write(writer =>
        {
            writer.WriteStartObject();
            writer.WritePropertyName("results");
            writer.WriteStartArray();
            foreach (Applied applied in results)
            {
                writer.WriteStartObject();
                writer.WriteString("id", applied.Id);
                if (applied.Refusal is { } refusal) writer.WriteString("refused", refusal);
                else writer.WriteNumber("value", applied.Kept);
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WriteNumber("refused", results.Count(applied => applied.Refusal is not null));
            writer.WritePropertyName("description");
            writer.WriteRawValue(Json(page));
            writer.WriteEndObject();
        }));
    }

    /// <summary>Applies one value, a number or a choice's option, clamped to the control's range.</summary>
    private static Applied Apply(TunablesPage page, string id, string value)
    {
        if (Find(page, id) is not { } tunable) return new(id, 0, $"No control '{id}' on this page.");
        TunableControl control = tunable.Control;
        float number;
        if (control.Options is { } options && OptionIndex(options, value) is >= 0 and var index) number = index;
        else if (!float.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out number))
            return new(id, 0, $"'{value}' is not a number{(control.Options is null ? "" : " or an option")} of '{id}'.");
        float kept = Math.Clamp(number, control.Min, control.Max);
        tunable.Set(kept);
        return new(id, kept, null);
    }

    private static int OptionIndex(IReadOnlyList<string> options, string value)
    {
        for (int index = 0; index < options.Count; index++)
            if (options[index] == value) return index;
        return -1;
    }

    private static Tunable? Find(TunablesPage page, string id)
    {
        foreach (TunableGroup group in page.Groups)
            foreach (Tunable tunable in group.Tunables)
                if (tunable.Control.Id == id) return tunable;
        return null;
    }

    /// <summary>The files the page's controls are bound to, in first-seen order.</summary>
    private static List<string> BoundFiles(TunablesPage page)
    {
        List<string> files = [];
        foreach (TunableGroup group in page.Groups)
            foreach (Tunable tunable in group.Tunables)
                if (tunable.Control.Source is { Path: var path } && !files.Contains(path)) files.Add(path);
        return files;
    }

    /// <summary>The page's controls bound to <paramref name="path"/> whose live value differs from the file's.</summary>
    private static IEnumerable<Unsaved> UnsavedIn(TunablesPage page, string path, JsonElement document)
    {
        foreach (TunableGroup group in page.Groups)
            foreach (Tunable tunable in group.Tunables)
            {
                TunableControl control = tunable.Control;
                if (control.Source is not { } bound || bound.Path != path) continue;
                if (!TryResolve(document, bound.Pointer, out JsonElement previous) || SavedValue(control, previous) is not { } saved) continue;
                if (Differs(control, control.Value, saved)) yield return new(control, saved, previous);
            }
    }

    /// <summary>The control value a source JSON value stands for: a number, a toggle's boolean or a choice's option.</summary>
    private static float? SavedValue(TunableControl control, JsonElement value) => value.ValueKind switch
    {
        JsonValueKind.Number => value.GetSingle(),
        JsonValueKind.True => 1,
        JsonValueKind.False => 0,
        JsonValueKind.String when control.Options is { } options && OptionIndex(options, value.GetString()!) is >= 0 and var index => index,
        _ => null,
    };

    /// <summary>Whether a live value differs from a saved one: floats at half a step.</summary>
    private static bool Differs(TunableControl control, float live, float saved) => control.Kind switch
    {
        TunableKind.Toggle => live >= 0.5f != saved >= 0.5f,
        TunableKind.Choice => MathF.Round(live) != MathF.Round(saved),
        _ => MathF.Abs(live - saved) > (control.Step > 0 ? control.Step / 2 : 0),
    };

    /// <summary>The JSON Save writes for a control's live value, in the saved value's own type and at its step.</summary>
    private static JsonNode Written(TunableControl control, JsonElement previous)
    {
        if (previous.ValueKind is JsonValueKind.True or JsonValueKind.False) return JsonValue.Create(control.Value >= 0.5f);
        if (previous.ValueKind == JsonValueKind.String && control.Options is { } options)
        {
            int index = (int)MathF.Round(control.Value);
            return JsonValue.Create(index >= 0 && index < options.Count ? options[index] : previous.GetString())!;
        }
        return JsonValue.Create(Math.Round((double)control.Value, StepDecimals(control.Step), MidpointRounding.AwayFromZero));
    }

    /// <summary>The decimals a step shows: 0.05 and 2.5 show two and one, 1 and 10 none.</summary>
    private static int StepDecimals(float step)
    {
        if (!(step > 0) || step == MathF.Floor(step)) return 0;
        string text = step.ToString("R", CultureInfo.InvariantCulture);
        int exponent = text.IndexOf("E-", StringComparison.Ordinal);
        string mantissa = exponent < 0 ? text : text[..exponent];
        int point = mantissa.IndexOf('.');
        int fraction = point < 0 ? 0 : mantissa.Length - point - 1;
        return fraction + (exponent < 0 ? 0 : int.Parse(text[(exponent + 2)..], CultureInfo.InvariantCulture));
    }

    /// <summary>The value an RFC 6901 pointer names in a document.</summary>
    private static bool TryResolve(JsonElement document, string pointer, out JsonElement value)
    {
        value = document;
        if (pointer.Length == 0) return true;
        if (pointer[0] != '/') return false;
        foreach (string raw in pointer[1..].Split('/'))
        {
            string token = raw.Replace("~1", "/", StringComparison.Ordinal).Replace("~0", "~", StringComparison.Ordinal);
            if (value.ValueKind == JsonValueKind.Array)
            {
                if (!int.TryParse(token, NumberStyles.None, CultureInfo.InvariantCulture, out int index) || index >= value.GetArrayLength()) return false;
                value = value[index];
            }
            else if (value.ValueKind != JsonValueKind.Object || !value.TryGetProperty(token, out value)) return false;
        }
        return true;
    }

    /// <summary>Reads and parses a bound file; the refusal when it cannot.</summary>
    private string? Read(string path, out ContentSourceDocument document, out JsonDocument? json)
    {
        json = null;
        try
        {
            document = _source.ReadDocument(path);
        }
        catch (EngineCallException exception)
        {
            document = default;
            return Refusal(exception);
        }
        try
        {
            json = JsonDocument.Parse(document.Bytes);
            return null;
        }
        catch (JsonException exception)
        {
            return $"{path} is not JSON: {exception.Message}";
        }
    }

    private static string Refusal(EngineCallException exception) =>
        exception.Diagnostics.Length > 0 && exception.Diagnostics.Span[0] is var diagnostic
            ? $"{diagnostic.Code}: {diagnostic.Message}"
            : exception.Message;

    private static string Hex(ContentSha256 sha256) => $"{sha256.Word0:x16}{sha256.Word1:x16}{sha256.Word2:x16}{sha256.Word3:x16}";

    private static bool TryParseSha(string hex, out ContentSha256 sha256)
    {
        sha256 = default;
        Span<ulong> words = stackalloc ulong[4];
        if (hex.Length != 64) return false;
        for (int index = 0; index < 4; index++)
            if (!ulong.TryParse(hex.AsSpan(index * 16, 16), NumberStyles.AllowHexSpecifier, CultureInfo.InvariantCulture, out words[index])) return false;
        sha256 = new(words[0], words[1], words[2], words[3]);
        return true;
    }

    /// <summary>The page a diff compares: its title, chooser values and held timeline key.</summary>
    private static void Page(Utf8JsonWriter writer, TunablesPage page)
    {
        writer.WritePropertyName("page");
        writer.WriteStartObject();
        writer.WriteString("title", page.Title);
        writer.WritePropertyName("choosers");
        writer.WriteStartObject();
        foreach (TunableChooser chooser in page.Choosers) writer.WriteString(chooser.Id, chooser.Value);
        writer.WriteEndObject();
        if (page.Timeline is { } timeline) writer.WriteNumber("key", timeline.Key);
        else writer.WriteNull("key");
        writer.WriteEndObject();
    }

    private static DebugCommandResult Failed(string message) => DebugCommandResult.Failure(DebugCommandStatus.Failed, message);

    private static DebugCommandResult Invalid(string message) => DebugCommandResult.Failure(DebugCommandStatus.InvalidArguments, message);

    private static string Write(Action<Utf8JsonWriter> write)
    {
        using MemoryStream stream = new();
        // Answers are read by people and agents as well as parsed: keep quotes readable.
        using (Utf8JsonWriter writer = new(stream, new JsonWriterOptions { Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping })) write(writer);
        return Encoding.UTF8.GetString(stream.ToArray());
    }

    private static string Json(TunablesPage page) => Write(writer =>
        {
            writer.WriteStartObject();
            writer.WriteString("contract", Contract);
            writer.WriteString("title", page.Title);
            writer.WritePropertyName("choosers");
            writer.WriteStartArray();
            foreach (TunableChooser chooser in page.Choosers)
            {
                writer.WriteStartObject();
                writer.WriteString("id", chooser.Id);
                writer.WriteString("label", chooser.Label);
                writer.WriteString("value", chooser.Value);
                Strings(writer, "options", chooser.Options);
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WritePropertyName("timeline");
            if (page.Timeline is { } timeline)
            {
                writer.WriteStartObject();
                writer.WritePropertyName("keys");
                writer.WriteStartArray();
                foreach (TunableKey key in timeline.Keys)
                {
                    writer.WriteStartObject();
                    writer.WriteNumber("index", key.Index);
                    writer.WriteString("label", key.Label);
                    writer.WriteNumber("time", key.Time);
                    writer.WriteEndObject();
                }
                writer.WriteEndArray();
                writer.WriteNumber("duration", timeline.Duration);
                writer.WriteNumber("key", timeline.Key);
                writer.WritePropertyName("speeds");
                writer.WriteStartArray();
                foreach (float speed in timeline.Speeds) writer.WriteNumberValue(speed);
                writer.WriteEndArray();
                writer.WriteEndObject();
            }
            else writer.WriteNullValue();
            writer.WritePropertyName("groups");
            writer.WriteStartArray();
            foreach (TunableGroup group in page.Groups)
            {
                writer.WriteStartObject();
                writer.WriteString("id", group.Id);
                writer.WriteString("title", group.Title);
                if (group.File is { } file) writer.WriteString("file", file);
                writer.WritePropertyName("controls");
                writer.WriteStartArray();
                foreach (Tunable tunable in group.Tunables) Control(writer, tunable.Control);
                writer.WriteEndArray();
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WritePropertyName("commands");
            writer.WriteStartObject();
            writer.WriteString("describe", "tunables.describe");
            writer.WriteString("select", "tunables.select {chooser} {value}");
            writer.WriteString("set", "tunables.set {id} {value}");
            writer.WriteString("setMany", "tunables.set-many {pairs}");
            writer.WriteString("diff", "tunables.diff");
            writer.WriteString("save", "tunables.save {path} {sha256}");
            writer.WriteString("reset", "tunables.reset {path}");
            foreach ((string name, string template) in page.Commands) writer.WriteString(name, template);
            writer.WriteEndObject();
            writer.WriteString("notice", page.Notice);
            writer.WriteEndObject();
        });

    private static void Control(Utf8JsonWriter writer, TunableControl control)
    {
        writer.WriteStartObject();
        writer.WriteString("id", control.Id);
        writer.WriteString("label", control.Label);
        writer.WriteString("kind", control.Kind switch
        {
            TunableKind.Slider => "slider",
            TunableKind.Toggle => "toggle",
            TunableKind.Choice => "choice",
            _ => throw new ArgumentOutOfRangeException(nameof(control)),
        });
        writer.WriteNumber("min", control.Min);
        writer.WriteNumber("max", control.Max);
        writer.WriteNumber("step", control.Step);
        writer.WriteNumber("value", control.Value);
        if (control.Options is { } options) Strings(writer, "options", options);
        writer.WriteBoolean("reshapes", control.Reshapes);
        if (control.Source is { } source)
        {
            writer.WritePropertyName("source");
            writer.WriteStartObject();
            writer.WriteString("path", source.Path);
            writer.WriteString("pointer", source.Pointer);
            writer.WriteEndObject();
        }
        writer.WriteEndObject();
    }

    private static void Strings(Utf8JsonWriter writer, string name, IReadOnlyList<string> values)
    {
        writer.WritePropertyName(name);
        writer.WriteStartArray();
        foreach (string value in values) writer.WriteStringValue(value);
        writer.WriteEndArray();
    }
}
