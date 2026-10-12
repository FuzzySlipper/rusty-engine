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

public readonly partial record struct AtmosphereRequest
{
    /// <summary>An atmosphere whose sky's sun is the brightest directional light, its disc and halo drawn at the light's intensity.</summary>
    public AtmosphereRequest(float FogBaseHeight, float FogFalloffHeight, Color HazeColor, float HazeExponent, float SunRadiusDegrees, float SunHalo)
        : this(FogBaseHeight, FogFalloffHeight, HazeColor, HazeExponent, SunRadiusDegrees, SunHalo, 0, default, default, 0)
    {
    }
}

public readonly partial record struct MoonRequest
{
    /// <summary>A moon that lights no clouds.</summary>
    public MoonRequest(System.Numerics.Vector3 Direction, float RadiusDegrees, System.Numerics.Vector3 Color, float Radiance, float Phase, float Earthshine, float Halo)
        : this(Direction, RadiusDegrees, Color, Radiance, Phase, Earthshine, Halo, 0)
    {
    }
}

public readonly partial record struct CloudsRequest
{
    /// <summary>A cloud layer of the default kind (cumulus) and thickness (six tenths of its altitude), lit only while the sun is up.</summary>
    public CloudsRequest(float Coverage, System.Numerics.Vector2 Drift, float Altitude, float Scale, System.Numerics.Vector3 Color)
        : this(Coverage, Drift, Altitude, Scale, Color, 0, CloudKind.Cumulus, 0)
    {
    }

    /// <summary>A cloud layer lit only while the sun is up.</summary>
    public CloudsRequest(float Coverage, System.Numerics.Vector2 Drift, float Altitude, float Scale, System.Numerics.Vector3 Color, float Thickness, CloudKind Kind)
        : this(Coverage, Drift, Altitude, Scale, Color, Thickness, Kind, 0)
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
