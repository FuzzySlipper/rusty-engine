using System.Globalization;
using System.Numerics;
using System.Text;
using System.Text.Json;
using Rusty.Engine.Interaction;

namespace Rusty.Engine.Debugging;

/// <summary>Discoverable commands over a product's ordinary WorldInteraction instance.
/// Reads and use execute on the existing serialized product debug boundary.</summary>
public sealed partial class InteractionDebugModule(WorldInteraction interaction, int maximumCandidates = 64) : IDebugCommandModule
{
    private readonly WorldInteraction _interaction = interaction ?? throw new ArgumentNullException(nameof(interaction));
    private readonly int _maximumCandidates = maximumCandidates is >= 1 and <= 256
        ? maximumCandidates : throw new ArgumentOutOfRangeException(nameof(maximumCandidates));

    [DebugCommand("interaction.help", Description = "World-object interaction green path: inspect candidate IDs/reasons, then interaction.use <id> <revision>. Target-ID assistance avoids pixel hunting; ordinary reach, visibility and product rules still apply.")]
    public DebugCommandResult Help() => DebugCommandResult.Success(
        "interaction.inspect lists current product candidates, focus/reach/visibility and exact use commands. " +
        "interaction.use <id> <revision> explicitly assists target selection and invokes the same product action as normal use. " +
        "It does not move, turn, bypass walls/range/locks, or prove physical clicking. Reinspect after moving or changing the scene. " +
        "For UI tests, verify the resulting product UI separately. Targeted use may be disabled by the product.");

    [DebugCommand("interaction.inspect", Description = "Read-only world targets with stable IDs, labels, world points, focus and assisted-use rejection reasons. Start here before repeatedly clicking a small world object.")]
    public DebugCommandResult Inspect()
    {
        WorldInteractionReadout readout = _interaction.Inspect();
        return DebugCommandResult.Success(Json(writer =>
        {
            writer.WriteString("stamp", readout.Scene.Stamp);
            writer.WriteString("action", readout.Scene.Action);
            writer.WriteString("assistance", "observation-only");
            writer.WriteBoolean("targetedUseEnabled", readout.TargetedUseEnabled);
            writer.WriteString("help", "interaction.help");
            writer.WriteString("focusReason", readout.Focus.Reason.ToString());
            writer.WritePropertyName("selected");
            if (readout.Focus.Selected is {} selected) WriteTarget(writer, selected); else writer.WriteNullValue();
            writer.WritePropertyName("query"); writer.WriteStartObject();
            Point(writer, "origin", readout.Scene.Query.Origin); Point(writer, "direction", readout.Scene.Query.Direction);
            writer.WriteNumber("acquireAngleRadians", readout.Scene.Query.AcquireAngleRadians);
            writer.WriteNumber("releaseAngleRadians", readout.Scene.Query.ReleaseAngleRadians);
            writer.WriteNumber("maximumDistance", readout.Scene.Query.MaximumDistance);
            writer.WriteNumber("releaseDistance", readout.Scene.Query.ReleaseDistance);
            if (readout.Scene.Query.DistanceOrigin is {} origin) Point(writer, "distanceOrigin", origin);
            writer.WriteEndObject();
            writer.WriteNumber("totalCandidates", readout.Focus.Candidates.Count);
            writer.WriteBoolean("truncated", readout.Focus.Candidates.Count > _maximumCandidates);
            writer.WritePropertyName("candidates"); writer.WriteStartArray();
            foreach (InteractionObservation row in readout.Focus.Candidates.Take(_maximumCandidates))
            {
                InteractionCandidate candidate = row.Candidate;
                writer.WriteStartObject();
                writer.WriteNumber("id", candidate.Target.Id); writer.WriteNumber("revision", candidate.Target.Revision);
                writer.WriteString("label", candidate.Label); Point(writer, "point", candidate.Point);
                writer.WriteNumber("distance", row.Distance); writer.WriteNumber("reachDistance", candidate.ReachDistance);
                writer.WriteNumber("angleRadians", row.AngleRadians); writer.WriteBoolean("selected", row.Selected);
                writer.WriteString("visibility", candidate.Visibility.ToString());
                writer.WriteString("availability", candidate.Availability.ToString());
                writer.WriteString("route", candidate.Route.ToString());
                writer.WriteString("focusReason", row.Reason.ToString());
                InteractionReason reason = readout.TargetedUseEnabled
                    ? InteractionFocus.RevalidateTarget(candidate.Target, readout.Scene.Candidates.Span, readout.Scene.Query)
                    : InteractionReason.Unavailable;
                writer.WriteString("targetedUseReason", reason.ToString());
                writer.WriteString("useCommand", string.Create(CultureInfo.InvariantCulture,
                    $"interaction.use {candidate.Target.Id} {candidate.Target.Revision}"));
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
        }));
    }

    [DebugCommand("interaction.use", Description = "MUTATES via ordinary product use action for a fresh target ID/revision. Explicit target-ID assistance bypasses only reticle precision, never reach, visibility, locks or product action rules. Inspect first.")]
    public DebugCommandResult Use(ulong id, ulong revision)
    {
        InteractionUseReceipt receipt = _interaction.UseTarget(new(id, revision));
        // Transport success carries the domain outcome; callers inspect performed/reason.
        return DebugCommandResult.Success(Json(writer =>
        {
            writer.WritePropertyName("target");
            if (receipt.Target is {} target) WriteTarget(writer, target); else writer.WriteNullValue();
            writer.WriteString("reason", receipt.Reason.ToString());
            writer.WriteBoolean("performed", receipt.Performed);
            writer.WriteString("message", receipt.Message);
            writer.WriteString("assistance", receipt.Assistance);
            writer.WriteString("stamp", receipt.Stamp);
        }));
    }

    private static string Json(Action<Utf8JsonWriter> write)
    {
        using MemoryStream stream = new();
        using (Utf8JsonWriter writer = new(stream)) { writer.WriteStartObject(); write(writer); writer.WriteEndObject(); }
        return Encoding.UTF8.GetString(stream.ToArray());
    }
    private static void WriteTarget(Utf8JsonWriter writer, InteractionTarget target)
    {
        writer.WriteStartObject(); writer.WriteNumber("id", target.Id); writer.WriteNumber("revision", target.Revision); writer.WriteEndObject();
    }
    private static void Point(Utf8JsonWriter writer, string name, Vector3 point)
    {
        writer.WritePropertyName(name); writer.WriteStartObject();
        writer.WriteNumber("x", point.X); writer.WriteNumber("y", point.Y); writer.WriteNumber("z", point.Z); writer.WriteEndObject();
    }
}
