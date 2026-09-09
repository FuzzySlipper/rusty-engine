using System.Numerics;

namespace Rusty.Engine.Interaction;

/// <summary>Line-of-sight composition over the ordinary retained Spatial projection.
/// Products supply their existing session and active entity colliders, not another spatial world.</summary>
public static class InteractionVisibilityQuery
{
    public static InteractionVisibility Cast(ISpatialService spatial, SpatialSession session,
        Vector3 origin, Vector3 target, SpatialQueryFilter filter,
        ReadOnlyMemory<SpatialEntityCollider> entities, ReadOnlyMemory<ulong> ignoredEntities,
        float endpointTolerance = 0.001f)
    {
        if (!float.IsFinite(endpointTolerance) || endpointTolerance < 0)
            throw new ArgumentOutOfRangeException(nameof(endpointTolerance));
        Vector3 offset = target - origin;
        float distance = offset.Length();
        if (distance <= endpointTolerance) return InteractionVisibility.Visible;
        SpatialHit hit = spatial.CastRay(new SpatialRaycastRequest(session, origin, offset / distance,
            distance, filter, entities, ignoredEntities, ReadOnlyMemory<SpatialEntityCollider>.Empty));
        return hit.Present && hit.Distance < distance - endpointTolerance
            ? InteractionVisibility.Occluded : InteractionVisibility.Visible;
    }
}
