using System;
using System.Numerics;

namespace Rusty.Engine;

/// <summary>
/// Where an overlay item's values are: the world, the node an entity's
/// appearance published, one of its rig joints as drawn, or the primary
/// camera's space, where a viewmodel's parts are placed.
/// </summary>
public readonly record struct OverlayPlace(OverlaySpace Space, ulong Entity = 0, uint Joint = 0)
{
    public static OverlayPlace World => new(OverlaySpace.World);

    public static OverlayPlace Camera => new(OverlaySpace.Camera);

    public static OverlayPlace Model(ulong entity) => new(OverlaySpace.Model, entity);

    /// <summary>A rig joint (its index in <see cref="IAnimationService.ReadJoints"/>) of the entity's animated mesh.</summary>
    public static OverlayPlace AtJoint(ulong entity, uint joint) => new(OverlaySpace.Joint, entity, joint);
}

/// <summary>
/// Overlay items: inspection drawing on the Debug layer, drawn only while
/// inspection shows it (<c>engine.inspect.overlay show</c>). Publish a named
/// set with <see cref="IGraphicsService.PublishOverlaySet"/>; it stays until
/// replaced, and an empty set removes it.
/// </summary>
public readonly partial record struct OverlayItem
{
    public static OverlayItem Segment(OverlayPlace place, Vector3 from, Vector3 to, Color color, bool onTop = false) =>
        Make(OverlayShape.Segment, place, from, to, Quaternion.Identity, 0, default, color, onTop);

    /// <summary>Three crossed lines <paramref name="size"/> long, centred on <paramref name="at"/>.</summary>
    public static OverlayItem Marker(OverlayPlace place, Vector3 at, float size, Color color, bool onTop = false) =>
        Make(OverlayShape.Marker, place, at, default, Quaternion.Identity, size, default, color, onTop);

    /// <summary>The frame's x, y and z axes in red, green and blue.</summary>
    public static OverlayItem Axes(OverlayPlace place, Vector3 at, Quaternion rotation, float length, bool onTop = false) =>
        Make(OverlayShape.Axes, place, at, default, rotation, length, default, new(1, 1, 1, 1), onTop);

    public static OverlayItem WireBox(OverlayPlace place, Vector3 center, Vector3 halfExtents, Quaternion rotation, Color color, bool onTop = false) =>
        Make(OverlayShape.WireBox, place, center, halfExtents, rotation, 0, default, color, onTop);

    public static OverlayItem WireSphere(OverlayPlace place, Vector3 center, float radius, Color color, bool onTop = false) =>
        Make(OverlayShape.WireSphere, place, center, default, Quaternion.Identity, radius, default, color, onTop);

    /// <summary>A perspective frustum from <paramref name="at"/>, looking down its -z.</summary>
    public static OverlayItem Frustum(OverlayPlace place, Vector3 at, Quaternion rotation, float fovYDegrees, float aspect, float near, float far, Color color, bool onTop = false) =>
        Make(OverlayShape.Frustum, place, at, default, rotation, 0, new(fovYDegrees, aspect, near, far), color, onTop);

    /// <summary>Text facing the viewer, anchored at <paramref name="at"/>.</summary>
    public static OverlayItem Label(OverlayPlace place, Vector3 at, string text, Color color, bool onTop = false) =>
        Make(OverlayShape.Label, place, at, default, Quaternion.Identity, 0, default, color, onTop, text);

    private static OverlayItem Make(
        OverlayShape shape,
        OverlayPlace place,
        Vector3 a,
        Vector3 b,
        Quaternion rotation,
        float size,
        Vector4 frustum,
        Color color,
        bool onTop,
        string text = "") =>
        new(shape, place.Space, place.Entity, place.Joint, a, b, rotation, size, frustum, color, onTop, text);
}

public readonly partial record struct OverlaySetRequest
{
    public OverlaySetRequest(string Name, OverlayItem[] Items)
        : this(Name, Items, ReadOnlyMemory<LandmarkPair>.Empty)
    {
    }

    /// <summary>A set with landmark pairs the renderer measures as drawn (<c>engine.inspect.landmarks</c>), and any items beside them.</summary>
    public OverlaySetRequest(string Name, OverlayItem[] Items, LandmarkPair[] Pairs)
        : this(Name, (ReadOnlyMemory<OverlayItem>)Items, (ReadOnlyMemory<LandmarkPair>)Pairs)
    {
    }
}
