using System.Numerics;

namespace Rusty.Engine.Implicit;

/// <summary>The layer composition selected by a product for one architectural wall.</summary>
public enum WallFinish
{
    Masonry,
    BrokenPlaster,
    DressedStone,
}

/// <summary>Materials selected by the product for a wall's independent layers.</summary>
public readonly record struct WallPalette(Material Body, Material Mortar, Material Trim, Material Plaster);

/// <summary>Product-selected geometry for broad missing-plaster patches.</summary>
public readonly record struct BrokenPlasterTuning(
    float Depth,
    float Bottom,
    float TopInset,
    float CenterOffset,
    float CenterPhaseScale,
    float MinimumHeight,
    float HeightPhaseScale,
    float HalfWidth)
{
    public static BrokenPlasterTuning Default => new(0.16f, 0.8f, 0.35f, 1.3f, 1.4f, 1.5f, 1.7f, 1.3f);
}

/// <summary>Explicit, product-tunable dimensions for <see cref="ArchitecturalRecipes.Wall"/>.</summary>
public readonly record struct ArchitecturalWallTuning(
    float BayLength,
    float Relief,
    float TrimDepth,
    float TrimWidth,
    float TrimCellSize,
    float RevealClearance,
    MasonryOptions Masonry,
    MasonryOptions DressedStone,
    float CopingBottomInset,
    float CopingTopExtension,
    BrokenPlasterTuning BrokenPlaster)
{
    public static ArchitecturalWallTuning Default => new(
        BayLength: 4f,
        Relief: 0.10f,
        TrimDepth: 0.16f,
        TrimWidth: 0.25f,
        TrimCellSize: 0.12f,
        RevealClearance: 0.08f,
        Masonry: new MasonryOptions(1.05f, 0.55f, 0.045f),
        DressedStone: new MasonryOptions(1.35f, 0.8f, 0.045f),
        CopingBottomInset: 0.1f,
        CopingTopExtension: 0.22f,
        BrokenPlaster: BrokenPlasterTuning.Default);
}

/// <summary>Axis-aligned local bounds used to keep a carved volume contained.</summary>
public readonly record struct ImplicitBounds(Vector3 Minimum, Vector3 Maximum);

/// <summary>One straight passage that may taper from its start to its end.</summary>
public readonly record struct PassageOptions(
    Vector3 Start,
    Vector3 End,
    float StartRadius,
    float EndRadius,
    PassageCap Cap = PassageCap.Rounded);

/// <summary>How a passage terminates at its start and end stations.</summary>
public enum PassageCap
{
    Flat,
    Rounded,
}

/// <summary>One ellipsoidal chamber in a product-selected passage layout.</summary>
public readonly record struct ChamberOptions(Vector3 Center, Vector3 Radii);

/// <summary>Reusable CSG construction for walls and bounded architectural volumes.</summary>
public static class ArchitecturalRecipes
{
    /// <summary>
    /// Creates a passage field node. Equal radii retain the capsule path; a
    /// tapered rounded passage adds endpoint spheres over its frustum core.
    /// </summary>
    public static ImplicitNode Passage(ImplicitRecipe recipe, PassageOptions options)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        if (options.Cap is not (PassageCap.Flat or PassageCap.Rounded))
        {
            throw new ArgumentOutOfRangeException(nameof(options), options.Cap, "Passage cap must be declared.");
        }
        if (options.StartRadius == options.EndRadius && options.Cap == PassageCap.Rounded)
        {
            return recipe.Capsule(options.Start, options.End, options.StartRadius);
        }

        ImplicitNode passage = recipe.Frustum(
            options.Start,
            options.End,
            options.StartRadius,
            options.EndRadius);
        if (options.Cap == PassageCap.Flat)
        {
            return passage;
        }
        if (options.StartRadius > 0f)
        {
            passage = recipe.Union(passage, recipe.Sphere(options.Start, options.StartRadius));
        }
        if (options.EndRadius > 0f)
        {
            passage = recipe.Union(passage, recipe.Sphere(options.End, options.EndRadius));
        }
        return passage;
    }

    /// <summary>Creates a chamber field node; products choose how it joins other stages.</summary>
    public static ImplicitNode Chamber(ImplicitRecipe recipe, ChamberOptions options)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        return recipe.Ellipsoid(options.Center, options.Radii);
    }

    /// <summary>Unions or smoothly joins an additional passage/chamber stage.</summary>
    public static ImplicitNode Join(ImplicitRecipe recipe, ImplicitNode source, ImplicitNode addition, float blendRadius)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        return blendRadius == 0f ? recipe.Union(source, addition) : recipe.Blend(source, addition, blendRadius);
    }

    /// <summary>
    /// Carves bounded air from a shell. Air is clipped to <paramref name="carveBounds"/>
    /// before subtraction; opening nodes then subtract afterward, so callers
    /// retain an explicit protected envelope while declaring intentional portals.
    /// </summary>
    public static ImplicitNode Enclosure(
        ImplicitRecipe recipe,
        ImplicitBounds shell,
        ImplicitBounds carveBounds,
        ImplicitNode air,
        ReadOnlyMemory<ImplicitNode> openings = default)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        ImplicitNode boundedAir = recipe.Intersect(
            air,
            recipe.Box(carveBounds.Minimum, carveBounds.Maximum));
        ImplicitNode enclosure = recipe.Subtract(recipe.Box(shell.Minimum, shell.Maximum), boundedAir);
        foreach (ImplicitNode opening in openings.Span)
        {
            enclosure = recipe.Subtract(enclosure, opening);
        }
        return enclosure;
    }

    /// <summary>
    /// Emits independently extracted wall layers synchronously. The callback
    /// owns actual mesh, appearance, and collision publication.
    /// </summary>
    public static void Wall(
        RecipeWriter writer,
        string name,
        WallLayout wall,
        WallFinish finish,
        WallPalette palette,
        Func<int, float> wearAt,
        ArchitecturalWallTuning? tuning = null)
    {
        ArgumentNullException.ThrowIfNull(writer);
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(wearAt);
        ArchitecturalWallTuning settings = tuning ?? ArchitecturalWallTuning.Default;
        MasonryOptions courses = finish == WallFinish.DressedStone ? settings.DressedStone : settings.Masonry;
        courses.Validate();
        if (!float.IsFinite(settings.BayLength) || settings.BayLength <= 0f)
        {
            throw new ArgumentOutOfRangeException(nameof(tuning), "Wall bay length must be finite and positive.");
        }

        float half = wall.Thickness * 0.5f;
        for (float start = 0f; start < wall.Length; start += settings.BayLength)
        {
            float end = MathF.Min(wall.Length, start + settings.BayLength);
            float wearPhase = wearAt((int)(start / settings.BayLength));
            using ImplicitRecipe field = writer.Begin();
            ImplicitNode backing = CutOpening(
                field,
                field.Box(new Vector3(start, 0f, -half), new Vector3(end, wall.Height, half)),
                wall,
                settings.RevealClearance * 3f);
            writer.Surface(
                name + " mortar",
                field,
                backing,
                new Vector3(start, 0f, -half),
                new Vector3(end, wall.Height, half),
                palette.Mortar,
                wall.Placement);

            bool hasStone = false;
            ImplicitNode stones = default;
            foreach (MasonryCourse stone in MasonryLayout.Courses(start, end, wall.Height, courses))
            {
                ImplicitNode block = field.Box(
                    new Vector3(stone.Left, stone.Bottom, -half - settings.Relief),
                    new Vector3(stone.Right, stone.Top, half + settings.Relief));
                stones = hasStone ? field.Union(stones, block) : block;
                hasStone = true;
            }
            if (hasStone)
            {
                stones = CutOpening(field, stones, wall, settings.RevealClearance * 2f);
                writer.Surface(
                    name + " courses",
                    field,
                    stones,
                    new Vector3(start, 0f, -half - settings.Relief),
                    new Vector3(end, wall.Height, half + settings.Relief),
                    palette.Body,
                    wall.Placement);
            }

            if (finish == WallFinish.BrokenPlaster)
            {
                EmitBrokenPlaster(writer, name, field, wall, palette, settings, start, end, half, wearPhase);
            }
        }

        writer.Box(
            name + " coping",
            new Vector3(0f, wall.Height - settings.CopingBottomInset, -half - settings.TrimDepth),
            new Vector3(wall.Length, wall.Height + settings.CopingTopExtension, half + settings.TrimDepth),
            palette.Trim,
            wall.Placement);

        if (wall.Opening is { } opening)
        {
            using ImplicitRecipe field = writer.Begin();
            WallOpening outer = opening with { Width = opening.Width + settings.TrimWidth * 2f };
            ImplicitNode trim = field.Subtract(
                Opening(field, outer, half + settings.TrimDepth),
                Opening(field, opening, half + settings.TrimDepth + settings.Relief));
            trim = field.Intersect(
                trim,
                field.Box(
                    new Vector3(outer.Left, 0f, -half - settings.TrimDepth),
                    new Vector3(outer.Right, outer.Top, -half + settings.Relief)));
            writer.Surface(
                name + " arch surround",
                field,
                trim,
                new Vector3(outer.Left, 0f, -half - settings.TrimDepth),
                new Vector3(outer.Right, outer.Top, -half + settings.Relief),
                palette.Trim,
                wall.Placement,
                cellSize: settings.TrimCellSize);
        }
    }

    /// <summary>Subtracts the configured opening after expanding it by a layer clearance.</summary>
    public static ImplicitNode CutOpening(
        ImplicitRecipe recipe,
        ImplicitNode source,
        WallLayout wall,
        float clearance) => wall.Opening is { } opening
        ? recipe.Subtract(
            source,
            Opening(
                recipe,
                opening with
                {
                    Width = opening.Width + clearance * 2f,
                    SpringHeight = opening.SpringHeight + clearance,
                },
                wall.Thickness + 1f))
        : source;

    /// <summary>Builds a rectangular or round-topped opening in wall-local coordinates.</summary>
    public static ImplicitNode Opening(ImplicitRecipe recipe, WallOpening opening, float depth)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        ImplicitNode lower = recipe.Box(
            new Vector3(opening.Left, -1f, -depth),
            new Vector3(opening.Right, opening.SpringHeight, depth));
        if (!opening.Arched)
        {
            return lower;
        }

        ImplicitNode round = recipe.Capsule(
            new Vector3(opening.Center, opening.SpringHeight, -depth),
            new Vector3(opening.Center, opening.SpringHeight, depth),
            opening.Width * 0.5f);
        return recipe.Union(lower, round);
    }

    private static void EmitBrokenPlaster(
        RecipeWriter writer,
        string name,
        ImplicitRecipe field,
        WallLayout wall,
        WallPalette palette,
        ArchitecturalWallTuning settings,
        float start,
        float end,
        float half,
        float wearPhase)
    {
        BrokenPlasterTuning plaster = settings.BrokenPlaster;
        ImplicitNode shell = field.Box(
            new Vector3(start, plaster.Bottom, -half - settings.Relief - plaster.Depth),
            new Vector3(end, wall.Height - plaster.TopInset, -half));
        float center = start + plaster.CenterOffset + wearPhase * plaster.CenterPhaseScale;
        float height = plaster.MinimumHeight + wearPhase * plaster.HeightPhaseScale;
        Vector2[] outline =
        [
            new(center - plaster.HalfWidth, -1f),
            new(center - plaster.HalfWidth + 0.2f, height * 0.65f),
            new(center - 0.4f, height),
            new(center + 0.35f, height * 0.86f),
            new(center + plaster.HalfWidth - 0.05f, height * 0.3f),
            new(center + plaster.HalfWidth + 0.05f, -1f),
        ];
        ImplicitNode missing = field.Box(
            new Vector3(start - 1f, -1f, -1f),
            new Vector3(end + 1f, wall.Height, 1f));
        for (int edge = 0; edge < outline.Length; edge++)
        {
            Vector2 a = outline[edge];
            Vector2 b = outline[(edge + 1) % outline.Length];
            Vector3 normal = Vector3.Normalize(new Vector3(a.Y - b.Y, b.X - a.X, 0f));
            missing = field.Intersect(missing, field.Plane(normal, Vector3.Dot(normal, new Vector3(a, 0f))));
        }
        shell = field.Subtract(shell, missing);
        shell = CutOpening(field, shell, wall, settings.RevealClearance);
        writer.Surface(
            name + " broken plaster",
            field,
            shell,
            new Vector3(start, plaster.Bottom, -half - settings.Relief - plaster.Depth),
            new Vector3(end, wall.Height - plaster.TopInset, -half),
            palette.Plaster,
            wall.Placement);
    }
}
