using System.Numerics;

namespace Rusty.Engine.Implicit;

public readonly record struct RoomMaterials(Material Floor, Material Walls, Material Ceiling);

/// <summary>Rectangular interior and wall/slab thickness. Portal boxes cut the shell and become explicit audit caps.</summary>
public readonly record struct RoomShellOptions(ImplicitBounds Interior, float Thickness,
    ReadOnlyMemory<RecipeOpening> Openings = default, float ExtractionMargin = .25f,
    ReadOnlyMemory<ImplicitBounds> FloorPlatforms = default, ReadOnlyMemory<ImplicitBounds> CeilingSoffits = default);

/// <summary>Small architectural compositions with geometry and continuity derived from the same dimensions.</summary>
public static class RoomRecipes
{
    /// <summary>
    /// Emits three named solids synchronously in local coordinates, then returns
    /// their declared joins and portals. Keep the returned declarations with the
    /// emitted meshes. Moving doors and publication remain product-owned.
    /// </summary>
    public static RecipeRoomContinuity Shell(RecipeWriter writer, string name, RoomShellOptions options,
        RoomMaterials materials)
    {
        ArgumentNullException.ThrowIfNull(writer);
        ArgumentException.ThrowIfNullOrWhiteSpace(name);
        Vector3 min = options.Interior.Minimum, max = options.Interior.Maximum;
        float t = options.Thickness, margin = options.ExtractionMargin;
        static bool Finite(Vector3 p) => float.IsFinite(p.X) && float.IsFinite(p.Y) && float.IsFinite(p.Z);
        if (!Finite(min) || !Finite(max) || min.X >= max.X || min.Y >= max.Y || min.Z >= max.Z
            || !float.IsFinite(t) || t <= 0 || !float.IsFinite(margin) || margin <= 0)
            throw new ArgumentException("Room bounds, thickness and extraction margin must be finite and positive.", nameof(options));
        RecipeOpening[] openings = options.Openings.ToArray();
        foreach (var opening in openings)
            if (string.IsNullOrWhiteSpace(opening.Name) || !Finite(opening.Bounds.Minimum) || !Finite(opening.Bounds.Maximum)
                || opening.Bounds.Minimum.X >= opening.Bounds.Maximum.X || opening.Bounds.Minimum.Y >= opening.Bounds.Maximum.Y
                || opening.Bounds.Minimum.Z >= opening.Bounds.Maximum.Z)
                throw new ArgumentException("Portal bounds must have positive finite extent and a name.", nameof(options));
        ImplicitBounds[] platforms = options.FloorPlatforms.ToArray(), soffits = options.CeilingSoffits.ToArray();
        void ValidateRelief(ImplicitBounds b, bool floor)
        {
            if (!Finite(b.Minimum) || !Finite(b.Maximum) || b.Minimum.X >= b.Maximum.X || b.Minimum.Y >= b.Maximum.Y || b.Minimum.Z >= b.Maximum.Z
                || b.Minimum.X < min.X || b.Maximum.X > max.X || b.Minimum.Z < min.Z || b.Maximum.Z > max.Z
                || b.Minimum.Y < min.Y || b.Maximum.Y > max.Y || (floor ? b.Minimum.Y != min.Y : b.Maximum.Y != max.Y))
                throw new ArgumentException("Relief must lie inside the room and meet its floor or ceiling.", nameof(options));
        }
        foreach (var b in platforms) ValidateRelief(b, true);
        foreach (var b in soffits) ValidateRelief(b, false);
        Vector3 outerMin = min - new Vector3(t), outerMax = max + new Vector3(t);
        Transform identity = new(Vector3.Zero, Quaternion.Identity, Vector3.One);
        void Emit(string suffix, Vector3 lo, Vector3 hi, Material material, bool hollow)
        {
            using ImplicitRecipe field = writer.Begin();
            ImplicitNode solid = field.Box(lo, hi);
            if (hollow) solid = field.Subtract(solid, field.Box(min - Vector3.UnitY * t, max + Vector3.UnitY * t));
            ImplicitBounds[] relief = suffix == "/floor" ? platforms : suffix == "/ceiling" ? soffits : [];
            foreach (var b in relief)
            {
                solid = field.Union(solid, field.Box(b.Minimum, b.Maximum));
                lo = Vector3.Min(lo, b.Minimum); hi = Vector3.Max(hi, b.Maximum);
            }
            foreach (var opening in openings)
                solid = field.Subtract(solid, field.Box(opening.Bounds.Minimum, opening.Bounds.Maximum));
            writer.Surface(name + suffix, field, solid, lo - new Vector3(margin), hi + new Vector3(margin), material, identity);
        }
        Emit("/floor", outerMin, new(outerMax.X, min.Y, outerMax.Z), materials.Floor, false);
        Emit("/walls", new(outerMin.X, min.Y, outerMin.Z), new(outerMax.X, max.Y, outerMax.Z), materials.Walls, true);
        Emit("/ceiling", new(outerMin.X, max.Y, outerMin.Z), outerMax, materials.Ceiling, false);
        List<RecipeJoin> joins = [];
        // Every wall/slab contact excludes the portal intervals cut at that height.
        void Edge(bool xFixed, float fixedCoordinate, float start, float end, float y, string slab, string side)
        {
            List<(float A, float B)> spans = [(start, end)];
            foreach (var opening in openings)
            {
                var b = opening.Bounds;
                if (b.Minimum.Y > y || b.Maximum.Y < y) continue;
                float crossMin = xFixed ? b.Minimum.X : b.Minimum.Z, crossMax = xFixed ? b.Maximum.X : b.Maximum.Z;
                if (crossMax < fixedCoordinate - t / 2 || crossMin > fixedCoordinate + t / 2) continue;
                float a = xFixed ? b.Minimum.Z : b.Minimum.X, z = xFixed ? b.Maximum.Z : b.Maximum.X;
                List<(float A, float B)> remaining = [];
                foreach (var span in spans)
                {
                    if (z <= span.A || a >= span.B) { remaining.Add(span); continue; }
                    if (a > span.A) remaining.Add((span.A, a));
                    if (z < span.B) remaining.Add((z, span.B));
                }
                spans = remaining;
            }
            int index = 0;
            foreach (var span in spans)
            {
                float center = (span.A + span.B) / 2, half = (span.B - span.A) / 2;
                joins.Add(new($"{name}/{side}-{slab}-{index++}", name + "/walls", name + "/" + slab,
                    xFixed ? new(fixedCoordinate, y, center) : new(center, y, fixedCoordinate),
                    xFixed ? new(0, 0, half) : new(half, 0, 0),
                    xFixed ? new(t * .4f, 0, 0) : new(0, 0, t * .4f)));
            }
        }
        foreach (var (y, slab) in new[] { (min.Y, "floor"), (max.Y, "ceiling") })
        {
            Edge(true, min.X - t / 2, min.Z, max.Z, y, slab, "west");
            Edge(true, max.X + t / 2, min.Z, max.Z, y, slab, "east");
            Edge(false, min.Z - t / 2, min.X, max.X, y, slab, "south");
            Edge(false, max.Z + t / 2, min.X, max.X, y, slab, "north");
        }
        return new(name, new(outerMin - new Vector3(margin), outerMax + new Vector3(margin)),
            (min + max) / 2, joins.ToArray(), openings);
    }
}
