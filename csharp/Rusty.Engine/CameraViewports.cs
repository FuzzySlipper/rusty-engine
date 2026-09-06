namespace Rusty.Engine;

/// <summary>Common normalized destinations for retained camera composition.</summary>
public static class CameraViewports
{
    public static readonly CameraViewport Full = new(0.0, 0.0, 1.0, 1.0);
    public static readonly CameraViewport LeftHalf = new(0.0, 0.0, 0.5, 1.0);
    public static readonly CameraViewport RightHalf = new(0.5, 0.0, 0.5, 1.0);
    public static readonly CameraViewport UpperRightInset = new(0.7, 0.7, 0.25, 0.25);
}
