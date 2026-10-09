namespace Rusty.Engine;

public readonly partial record struct CameraDescriptor
{
    /// <summary>A camera whose viewmodel layer draws with the camera's own projection.</summary>
    public CameraDescriptor(
        CameraPose Pose,
        CameraBasisMode BasisMode,
        CameraBasis Basis,
        CameraProjection Projection,
        CameraViewport Viewport)
        : this(Pose, BasisMode, Basis, Projection, Viewport, 0)
    {
    }
}

public readonly partial record struct CloudsRequest
{
    /// <summary>A cloud layer of the default kind (cumulus) and thickness (six tenths of its altitude).</summary>
    public CloudsRequest(float Coverage, System.Numerics.Vector2 Drift, float Altitude, float Scale, System.Numerics.Vector3 Color)
        : this(Coverage, Drift, Altitude, Scale, Color, 0, CloudKind.Cumulus)
    {
    }
}

public readonly partial record struct CloudRegionRequest
{
    /// <summary>A region of cumulus as thick as the layer.</summary>
    public CloudRegionRequest(uint Id, System.Numerics.Vector2 Center, float Radius, float Coverage, float Darkness, System.Numerics.Vector2 Drift)
        : this(Id, Center, Radius, Coverage, Darkness, Drift, CloudKind.Cumulus, 0)
    {
    }
}

public readonly partial record struct OptionalCamera
{
    /// <summary>The camera; <c>default</c> is none.</summary>
    public static implicit operator OptionalCamera(Camera camera) => new(camera);
}

public readonly partial record struct OptionalSpatialSession
{
    /// <summary>The session; <c>default</c> is none.</summary>
    public static implicit operator OptionalSpatialSession(SpatialSession session) => new(session);
}
