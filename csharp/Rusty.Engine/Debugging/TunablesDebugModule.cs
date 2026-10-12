using System.Globalization;
using System.Text;
using System.Text.Json;

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
    /// <summary>Where Save writes the value and Reset reads it, if anywhere.</summary>
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
/// Serves the Engine tunables panel (<c>mountTunablesPanel</c>) from the product's own explicit
/// declarations: <c>tunables.describe</c> answers the <c>tunables.v1</c> description,
/// <c>tunables.select</c> changes a chooser and <c>tunables.set</c> applies one value. The panel
/// saves bound values through the Engine content source. Register one instance with the debug catalog.
/// </summary>
public sealed class TunablesDebugModule(Func<TunablesPage> describe, Func<string, string, string?>? select = null) : IDebugCommandModule
{
    public const string Contract = "tunables.v1";
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
        foreach (TunableGroup group in _describe().Groups)
            foreach (Tunable tunable in group.Tunables)
            {
                if (tunable.Control.Id != id) continue;
                float kept = Math.Clamp(value, tunable.Control.Min, tunable.Control.Max);
                tunable.Set(kept);
                return DebugCommandResult.Success(kept.ToString(CultureInfo.InvariantCulture));
            }
        return Invalid($"No control '{id}' on this page.");
    }

    private static DebugCommandResult Invalid(string message) => DebugCommandResult.Failure(DebugCommandStatus.InvalidArguments, message);

    private static string Json(TunablesPage page)
    {
        using MemoryStream stream = new();
        using (Utf8JsonWriter writer = new(stream))
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
            writer.WriteString("read", "engine.content.source.read {path}");
            writer.WriteString("edit", "engine.content.source.edit {path} {sha256} {edits}");
            foreach ((string name, string template) in page.Commands) writer.WriteString(name, template);
            writer.WriteEndObject();
            writer.WriteString("notice", page.Notice);
            writer.WriteEndObject();
        }
        return Encoding.UTF8.GetString(stream.ToArray());
    }

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
