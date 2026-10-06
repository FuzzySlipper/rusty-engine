namespace Rusty.Engine;

public readonly partial record struct CameraDescriptor
{
    /// <summary>A camera whose viewmodel layer draws with the camera's own projection.</summary>
    public CameraDescriptor(
        CameraPose pose,
        CameraBasisMode basisMode,
        CameraBasis basis,
        CameraProjection projection,
        CameraViewport viewport)
        : this(pose, basisMode, basis, projection, viewport, 0)
    {
    }
}
