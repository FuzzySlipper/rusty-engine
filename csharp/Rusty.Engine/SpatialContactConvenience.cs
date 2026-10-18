using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct SpatialShape
{
    /// <summary>A box of <paramref name="halfExtents"/> centred on <paramref name="center"/> in its frame.</summary>
    public static SpatialShape Box(Vector3 halfExtents, Vector3 center = default) =>
        new(SpatialShapeKind.Box, center, halfExtents, default, default, 0);

    /// <summary>
    /// The box between two world corners, in either order: placed at
    /// <see cref="SpatialPose.Identity"/>, it is that axis-aligned box.
    /// </summary>
    public static SpatialShape Aabb(Vector3 min, Vector3 max) =>
        Box(Vector3.Abs(max - min) * 0.5f, (min + max) * 0.5f);

    /// <summary>A capsule from <paramref name="a"/> to <paramref name="b"/> in its frame.</summary>
    public static SpatialShape Capsule(Vector3 a, Vector3 b, double radius) =>
        new(SpatialShapeKind.Capsule, default, default, a, b, radius);

    public static SpatialShape Sphere(double radius, Vector3 center = default) =>
        new(SpatialShapeKind.Sphere, center, default, default, default, radius);

    /// <summary>A segment from <paramref name="a"/> to <paramref name="b"/> in its frame: a blade edge or a thrust.</summary>
    public static SpatialShape Segment(Vector3 a, Vector3 b) =>
        new(SpatialShapeKind.Segment, default, default, a, b, 0);
}

public readonly partial record struct SpatialPose
{
    public static SpatialPose Identity => new(Vector3.Zero, Quaternion.Identity);

    public static SpatialPose At(Vector3 translation) => new(translation, Quaternion.Identity);

    /// <summary>A joint's placement, as <see cref="JointPose.World"/> gives it; its scale is not part of a pose.</summary>
    public static SpatialPose Of(Transform transform) => new(transform.Translation, transform.Rotation);

    /// <summary>A frame given in this pose's frame, placed in the world: a hit shape's offset from its bone.</summary>
    public SpatialPose Then(SpatialPose local) =>
        new(Translation + Vector3.Transform(local.Translation, Rotation), Quaternion.Normalize(Rotation * local.Rotation));
}

public readonly partial record struct SpatialEntityPart
{
    /// <summary>An enabled, unfiltered part.</summary>
    public SpatialEntityPart(ulong entity, uint part, SpatialShape shape, SpatialPose pose, ulong holder = 0)
        : this(entity, part, holder, shape, pose, 0, 0, true)
    {
    }

    /// <summary>
    /// A part that follows a joint: <paramref name="shape"/> placed at
    /// <paramref name="offset"/> in the joint's frame, with the joint where
    /// <paramref name="joint"/> puts it in the world (from
    /// <c>SampleJointPose</c> or <c>ReadJointPose</c>).
    /// </summary>
    public static SpatialEntityPart OnJoint(
        ulong entity,
        uint part,
        JointPose joint,
        SpatialShape shape,
        SpatialPose offset,
        ulong holder = 0) =>
        new(entity, part, shape, SpatialPose.Of(joint.World).Then(offset), holder);
}

public readonly partial record struct SpatialContactQueryRequest
{
    /// <summary>
    /// Every collider, part and (unless <paramref name="excludeWorld"/>)
    /// world source that <paramref name="shape"/> placed at
    /// <paramref name="pose"/> overlaps, for <c>OverlapContacts</c>.
    /// </summary>
    public static SpatialContactQueryRequest Overlap(
        SpatialSession session,
        SpatialShape shape,
        SpatialPose pose,
        ReadOnlyMemory<SpatialEntityCollider> entities = default,
        ReadOnlyMemory<SpatialEntityPart> parts = default,
        ReadOnlyMemory<ulong> ignoredEntities = default,
        SpatialQueryFilter filter = default,
        bool excludeWorld = false,
        uint maxContacts = 0) =>
        new(session, shape, pose, pose, Vector3.Zero, filter, entities, parts, ignoredEntities, false, excludeWorld, maxContacts);

    /// <summary>
    /// Everything <paramref name="shape"/> meets moving from
    /// <paramref name="start"/> to <paramref name="end"/>, turning about
    /// <paramref name="pivot"/> (in its frame: the grip, or a shoulder), in
    /// order of impact, for <c>SweepContacts</c>.
    /// </summary>
    public static SpatialContactQueryRequest Sweep(
        SpatialSession session,
        SpatialShape shape,
        SpatialPose start,
        SpatialPose end,
        ReadOnlyMemory<SpatialEntityCollider> entities = default,
        ReadOnlyMemory<SpatialEntityPart> parts = default,
        ReadOnlyMemory<ulong> ignoredEntities = default,
        SpatialQueryFilter filter = default,
        Vector3 pivot = default,
        bool excludeWorld = false,
        uint maxContacts = 0) =>
        new(session, shape, start, end, pivot, filter, entities, parts, ignoredEntities, false, excludeWorld, maxContacts);
}
