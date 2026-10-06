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
