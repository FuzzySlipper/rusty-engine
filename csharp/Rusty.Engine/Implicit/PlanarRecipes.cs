using System.Numerics;

namespace Rusty.Engine.Implicit;

/// <summary>Validated planar construction helpers over an existing implicit recipe.</summary>
public static class PlanarRecipes
{
    /// <summary>
    /// Creates a vertically bounded prism from a finite, strictly convex polygon.
    /// The vertices may be clockwise or counterclockwise. Expansion moves each
    /// side plane outward in its local planar normal direction.
    /// </summary>
    public static ImplicitNode ConvexPrism(
        ImplicitRecipe recipe,
        ReadOnlySpan<Vector2> vertices,
        float bottom,
        float top,
        float expansion = 0f)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        ValidatedPrism prism = ValidateConvexPrism(vertices, bottom, top, expansion);
        return BuildConvexPrism(recipe, prism, bottom, top);
    }

    /// <summary>
    /// Creates one square-capped raised walkway by unioning its finite,
    /// non-zero-length centerline segments. All input is validated before the
    /// recipe receives a field operation.
    /// </summary>
    public static ImplicitNode Walkway(
        ImplicitRecipe recipe,
        ReadOnlySpan<Vector2> centerline,
        float halfWidth,
        float bottom,
        float top)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        ValidateVerticalBounds(bottom, top);
        if (!float.IsFinite(halfWidth) || halfWidth <= 0f)
        {
            throw new ArgumentOutOfRangeException(nameof(halfWidth), "Walkway half-width must be finite and positive.");
        }
        if (centerline.Length < 2)
        {
            throw new ArgumentException("A walkway needs at least two centerline points.", nameof(centerline));
        }

        for (int i = 0; i < centerline.Length; i++)
        {
            if (!IsFinite(centerline[i]))
            {
                throw new ArgumentException("Walkway centerline points must be finite.", nameof(centerline));
            }
        }

        ValidatedPrism[] segments = new ValidatedPrism[centerline.Length - 1];
        for (int i = 0; i < segments.Length; i++)
        {
            Vector2 start = centerline[i];
            Vector2 end = centerline[i + 1];
            Vector2 direction = end - start;
            float lengthSquared = direction.LengthSquared();
            if (!IsFinite(direction) || !float.IsFinite(lengthSquared) || lengthSquared <= 0f)
            {
                throw new ArgumentException("Walkway segments must be finite and non-zero length.", nameof(centerline));
            }

            float length = MathF.Sqrt(lengthSquared);
            if (!float.IsFinite(length) || length <= 0f)
            {
                throw new ArgumentException("Walkway segments must have a finite length.", nameof(centerline));
            }

            direction /= length;
            Vector2 normal = new(-direction.Y, direction.X);
            Vector2 cappedStart = start - direction * halfWidth;
            Vector2 cappedEnd = end + direction * halfWidth;
            Vector2[] rectangle =
            [
                cappedStart - normal * halfWidth,
                cappedStart + normal * halfWidth,
                cappedEnd + normal * halfWidth,
                cappedEnd - normal * halfWidth,
            ];
            segments[i] = ValidateConvexPrism(rectangle, bottom, top, expansion: 0f);
        }

        ImplicitNode walkway = BuildConvexPrism(recipe, segments[0], bottom, top);
        for (int i = 1; i < segments.Length; i++)
        {
            walkway = recipe.Union(walkway, BuildConvexPrism(recipe, segments[i], bottom, top));
        }
        return walkway;
    }

    private static ValidatedPrism ValidateConvexPrism(
        ReadOnlySpan<Vector2> vertices,
        float bottom,
        float top,
        float expansion)
    {
        ValidateVerticalBounds(bottom, top);
        if (!float.IsFinite(expansion) || expansion < 0f)
        {
            throw new ArgumentOutOfRangeException(nameof(expansion), "Prism expansion must be finite and non-negative.");
        }
        if (vertices.Length < 3)
        {
            throw new ArgumentException("A prism needs at least three vertices.", nameof(vertices));
        }

        for (int i = 0; i < vertices.Length; i++)
        {
            if (!IsFinite(vertices[i]))
            {
                throw new ArgumentException("Prism vertices must be finite.", nameof(vertices));
            }
        }

        int winding = 0;
        for (int i = 0; i < vertices.Length; i++)
        {
            Vector2 edge = vertices[(i + 1) % vertices.Length] - vertices[i];
            Vector2 nextEdge = vertices[(i + 2) % vertices.Length] - vertices[(i + 1) % vertices.Length];
            float edgeLengthSquared = edge.LengthSquared();
            if (!IsFinite(edge) || !float.IsFinite(edgeLengthSquared) || edgeLengthSquared <= 0f)
            {
                throw new ArgumentException("Prism edges must be finite and non-zero length.", nameof(vertices));
            }

            float turn = Cross(edge, nextEdge);
            if (!float.IsFinite(turn) || turn == 0f)
            {
                throw new ArgumentException("Prism vertices must form a nondegenerate strictly convex polygon.", nameof(vertices));
            }

            int turnWinding = Math.Sign(turn);
            if (winding == 0)
            {
                winding = turnWinding;
            }
            else if (winding != turnWinding)
            {
                throw new ArgumentException("Prism vertices must form a convex polygon.", nameof(vertices));
            }
        }

        ValidateNoSelfIntersections(vertices);

        Vector3[] normals = new Vector3[vertices.Length];
        float[] offsets = new float[vertices.Length];
        for (int i = 0; i < vertices.Length; i++)
        {
            Vector2 start = vertices[i];
            Vector2 edge = vertices[(i + 1) % vertices.Length] - start;
            float length = edge.Length();
            Vector3 leftNormal = new(-edge.Y / length, 0f, edge.X / length);
            Vector3 normal = winding > 0 ? -leftNormal : leftNormal;
            float offset = normal.X * start.X + normal.Z * start.Y + expansion;
            if (!IsFinite(normal) || !float.IsFinite(offset))
            {
                throw new ArgumentException("Prism side planes must have finite coefficients.", nameof(vertices));
            }
            normals[i] = normal;
            offsets[i] = offset;
        }

        return new ValidatedPrism(normals, offsets);
    }

    private static ImplicitNode BuildConvexPrism(
        ImplicitRecipe recipe,
        ValidatedPrism prism,
        float bottom,
        float top)
    {
        ImplicitNode solid = recipe.Intersect(
            recipe.Plane(Vector3.UnitY, top),
            recipe.Plane(-Vector3.UnitY, -bottom));
        for (int i = 0; i < prism.Normals.Length; i++)
        {
            solid = recipe.Intersect(solid, recipe.Plane(prism.Normals[i], prism.Offsets[i]));
        }
        return solid;
    }

    private static void ValidateVerticalBounds(float bottom, float top)
    {
        if (!float.IsFinite(bottom) || !float.IsFinite(top) || bottom >= top)
        {
            throw new ArgumentOutOfRangeException(nameof(top), "Prism bottom and top must be finite with bottom below top.");
        }
    }

    private static void ValidateNoSelfIntersections(ReadOnlySpan<Vector2> vertices)
    {
        for (int first = 0; first < vertices.Length; first++)
        {
            int firstEnd = (first + 1) % vertices.Length;
            for (int second = first + 1; second < vertices.Length; second++)
            {
                int secondEnd = (second + 1) % vertices.Length;
                if (firstEnd == second || secondEnd == first)
                {
                    continue;
                }
                if (SegmentsIntersect(vertices[first], vertices[firstEnd], vertices[second], vertices[secondEnd]))
                {
                    throw new ArgumentException("Prism polygon must not self-intersect.", nameof(vertices));
                }
            }
        }
    }

    private static bool SegmentsIntersect(Vector2 a, Vector2 b, Vector2 c, Vector2 d)
    {
        float abc = Cross(b - a, c - a);
        float abd = Cross(b - a, d - a);
        float cda = Cross(d - c, a - c);
        float cdb = Cross(d - c, b - c);
        if (!float.IsFinite(abc) || !float.IsFinite(abd) || !float.IsFinite(cda) || !float.IsFinite(cdb))
        {
            throw new ArgumentException("Prism polygon intersections must have finite coordinates.", "vertices");
        }

        if (OppositeSigns(abc, abd) && OppositeSigns(cda, cdb))
        {
            return true;
        }
        return (abc == 0f && OnSegment(a, b, c))
            || (abd == 0f && OnSegment(a, b, d))
            || (cda == 0f && OnSegment(c, d, a))
            || (cdb == 0f && OnSegment(c, d, b));
    }

    private static bool OnSegment(Vector2 start, Vector2 end, Vector2 point) =>
        point.X >= MathF.Min(start.X, end.X)
        && point.X <= MathF.Max(start.X, end.X)
        && point.Y >= MathF.Min(start.Y, end.Y)
        && point.Y <= MathF.Max(start.Y, end.Y);

    private static bool OppositeSigns(float left, float right) =>
        (left < 0f && right > 0f) || (left > 0f && right < 0f);

    private static float Cross(Vector2 left, Vector2 right) => left.X * right.Y - left.Y * right.X;

    private static bool IsFinite(Vector2 value) => float.IsFinite(value.X) && float.IsFinite(value.Y);

    private static bool IsFinite(Vector3 value) =>
        float.IsFinite(value.X) && float.IsFinite(value.Y) && float.IsFinite(value.Z);

    private readonly record struct ValidatedPrism(Vector3[] Normals, float[] Offsets);
}
