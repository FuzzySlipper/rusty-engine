using System.Numerics;
using System.Text;
using System.Text.Json;

namespace Rusty.Engine.Debugging;

/// <summary>Explicit body-sized collision queries. Does not move a character or plan a route.</summary>
public static class SpatialClearanceSnapshot
{
    public static string Capture(ISpatialService spatial, SpatialSession session, Vector3 center,
        Vector3 targetFeet, float bodyHeight, CharacterControllerConfig config,
        ReadOnlyMemory<SpatialEntityCollider> entities)
    {
        Vector3 target = targetFeet + Vector3.UnitY * (bodyHeight * .5f);
        Vector3 translation = target - center;
        if (!float.IsFinite(translation.LengthSquared()) || translation.Length() > 8)
            throw new ArgumentOutOfRangeException(nameof(targetFeet), "Target must be within 8 world units of the current body.");
        double halfHeight = Math.Max(0, bodyHeight * .5 - config.Shape.Radius);
        SpatialCapsuleQueryRequest Request(Vector3 at, Vector3 delta, double skin) => new(session, at,
            halfHeight, config.Shape.Radius, delta, skin, new(1, uint.MaxValue), entities, ReadOnlyMemory<ulong>.Empty);
        var current = spatial.OverlapCapsule(Request(center, Vector3.Zero, 0));
        var sweep = translation.LengthSquared() > 0 ? spatial.CastCapsule(Request(center, translation, config.Shape.ContactSkin)) : default;
        var destination = spatial.OverlapCapsule(Request(target, Vector3.Zero, 0));
        float drop = Math.Max(config.Surface.MaximumStepHeight, config.Shape.ContactSkin) + .1f;
        var support = spatial.CastCapsule(Request(target, -Vector3.UnitY * drop, 0));
        using var stream = new MemoryStream();
        using (var w = new Utf8JsonWriter(stream))
        {
            w.WriteStartObject();
            w.WriteString("meaning", "read-only capsule overlap and straight sweep; no stepping, jumping, route search or guaranteed landing");
            w.WriteString("sources", "retained static collision plus product-supplied dynamic colliders");
            Point(w, "center", center); Point(w, "targetFeet", targetFeet); Point(w, "translation", translation);
            w.WriteNumber("bodyHeight", bodyHeight); w.WriteNumber("radius", config.Shape.Radius);
            w.WriteNumber("contactSkin", config.Shape.ContactSkin); w.WriteNumber("maximumStepHeight", config.Surface.MaximumStepHeight);
            w.WriteString("translationStatus", !sweep.Present ? "no-contact" : sweep.StartSolid || sweep.TimeOfImpact <= 0 ? "contact-at-start" : "obstructed");
            w.WriteString("targetStatus", destination.Present ? "overlapping" : "no-overlap");
            Hit(w, "currentOverlap", current); Hit(w, "sweep", sweep); Hit(w, "targetOverlap", destination); Hit(w, "supportBelowTarget", support);
            w.WriteNumber("supportProbeDistance", drop);
            w.WriteBoolean("supportNormalWithinSlopeLimit", support.Present && support.Normal.Y >= Math.Cos(config.Surface.MaximumSlopeRadians));
            w.WriteString("supportCaution", "A suitable normal is one support fact, not proof that the body can reach or stand at the target.");
            w.WriteEndObject();
        }
        return Encoding.UTF8.GetString(stream.ToArray());
    }

    private static void Point(Utf8JsonWriter w, string name, Vector3 value)
    {
        w.WritePropertyName(name); w.WriteStartArray();
        w.WriteNumberValue(value.X); w.WriteNumberValue(value.Y); w.WriteNumberValue(value.Z); w.WriteEndArray();
    }
    private static void Hit(Utf8JsonWriter w, string name, SpatialHit hit)
    {
        w.WritePropertyName(name); w.WriteStartObject(); w.WriteBoolean("present", hit.Present);
        if (hit.Present)
        {
            w.WriteString("kind", hit.Kind.ToString()); w.WriteString("entity", hit.Entity.ToString()); w.WriteString("instance", hit.Instance.ToString());
            w.WriteNumber("timeOfImpact", hit.TimeOfImpact); w.WriteNumber("penetrationDepth", hit.PenetrationDepth);
            w.WriteBoolean("startSolid", hit.StartSolid); w.WriteBoolean("converged", hit.Converged);
            Point(w, "point", hit.Point); Point(w, "normal", hit.Normal);
        }
        w.WriteEndObject();
    }
}

public sealed class SpatialClearanceDebugModule(Func<double, double, double, DebugCommandResult> inspect) : IDebugCommandModule
{
    [DebugCommand("spatial.clearance", Description = "Read-only current/target body overlap, direct capsule sweep and support below target. XYZ target feet, within 8 units. Does not plan a route or advance time.")]
    public DebugCommandResult Inspect(double x, double y, double z) => inspect(x, y, z);
}
