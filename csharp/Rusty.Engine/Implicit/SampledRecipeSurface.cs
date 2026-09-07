namespace Rusty.Engine.Implicit;

/// <summary>
/// A synchronous extraction description for retained density data. The volume
/// defines geometry and lattice spacing; the request's field only defines
/// material regions. Both owners must remain alive while the receiver generates
/// the mesh. The resulting mesh is an independent retained snapshot.
/// </summary>
public sealed record SampledRecipeSurface(
    string Name,
    SampledVolumeGenerateRequest Request,
    Transform Placement);
