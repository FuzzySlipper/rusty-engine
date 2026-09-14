using System.Numerics;

namespace Rusty.Engine.Implicit;

/// <summary>Authored contact between named recipe surfaces; Engine performs the actual analysis.</summary>
public readonly record struct RecipeJoin(string Name, string SurfaceA, string SurfaceB,
    Vector3 Center, Vector3 HalfU, Vector3 HalfV)
{
    public RecipeJoin Placed(Matrix4x4 placement)
    {
        Vector3 center = Vector3.Transform(Center, placement), u = Vector3.TransformNormal(HalfU, placement), v = Vector3.TransformNormal(HalfV, placement);
        float uLength = u.Length(), vLength = v.Length();
        if (!float.IsFinite(center.X) || !float.IsFinite(center.Y) || !float.IsFinite(center.Z)
            || !float.IsFinite(uLength) || !float.IsFinite(vLength) || uLength <= 0 || vLength <= 0
            || MathF.Abs(Vector3.Dot(u / uLength, v / vLength)) > 1e-5f
            || placement.M14 != 0 || placement.M24 != 0 || placement.M34 != 0 || placement.M44 != 1)
            throw new ArgumentException("Join placement must preserve a finite rectangular contact patch.", nameof(placement));
        return this with { Center = center, HalfU = u, HalfV = v };
    }

    /// <summary>Resolve names against meshes captured by the caller in this audit.</summary>
    public ImplicitJoinRequest Request(ImplicitAudit audit, Func<string, ulong> resolve,
        float searchDistance, float toleranceCells, float sampleSpacing, ulong maxSamples)
        => new(audit, resolve(SurfaceA), resolve(SurfaceB), Center, HalfU, HalfV,
            searchDistance, toleranceCells, sampleSpacing, maxSamples);
}

/// <summary>An intentional portal, kept beside the geometry that cuts it.</summary>
public readonly record struct RecipeOpening(string Name, ImplicitBounds Bounds)
{
    public ImplicitEnclosureOpening EnclosureOpening => new(Bounds.Minimum, Bounds.Maximum);
}

/// <summary>Room-local audit declarations. Products select analysis budgets and acceptance policy.</summary>
public sealed record RecipeRoomContinuity(string Name, ImplicitBounds Bounds, Vector3 Interior,
    ReadOnlyMemory<RecipeJoin> Joins, ReadOnlyMemory<RecipeOpening> Openings)
{
    /// <summary>
    /// Joins support affine placements that preserve rectangular contact patches. Enclosure caps are axis-aligned in the
    /// Engine, so room placement must preserve coordinate axes (including reflections).
    /// Arbitrarily rotated rooms can still use their individual placed joins.
    /// </summary>
    public RecipeRoomContinuity Placed(Matrix4x4 placement)
    {
        static bool Axis(Vector3 value) => float.IsFinite(value.X) && float.IsFinite(value.Y) && float.IsFinite(value.Z) && ((MathF.Abs(value.X) > 1e-5f ? 1 : 0)
            + (MathF.Abs(value.Y) > 1e-5f ? 1 : 0) + (MathF.Abs(value.Z) > 1e-5f ? 1 : 0) == 1);
        if (!float.IsFinite(placement.M41) || !float.IsFinite(placement.M42) || !float.IsFinite(placement.M43)
            || !Matrix4x4.Invert(placement, out _) || placement.M14 != 0 || placement.M24 != 0 || placement.M34 != 0 || placement.M44 != 1
            || !Axis(Vector3.TransformNormal(Vector3.UnitX, placement))
            || !Axis(Vector3.TransformNormal(Vector3.UnitY, placement))
            || !Axis(Vector3.TransformNormal(Vector3.UnitZ, placement)))
            throw new ArgumentException("Enclosure openings require a finite, invertible axis-preserving affine placement.", nameof(placement));
        ImplicitBounds Place(ImplicitBounds b)
        {
            Vector3 a = Vector3.Transform(b.Minimum, placement), z = Vector3.Transform(b.Maximum, placement);
            return new(Vector3.Min(a, z), Vector3.Max(a, z));
        }
        return this with { Bounds = Place(Bounds), Interior = Vector3.Transform(Interior, placement),
            Joins = Joins.ToArray().Select(j => j.Placed(placement)).ToArray(),
            Openings = Openings.ToArray().Select(o => o with { Bounds = Place(o.Bounds) }).ToArray() };
    }

    public ImplicitEnclosureRequest Request(ImplicitAudit audit, float sampleSpacing, ulong maxSamples)
        => new(audit, Bounds.Minimum, Bounds.Maximum, Interior,
            Openings.ToArray().Select(o => o.EnclosureOpening).ToArray(), sampleSpacing, maxSamples);
}
