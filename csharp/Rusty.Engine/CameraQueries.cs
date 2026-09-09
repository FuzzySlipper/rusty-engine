using System.Numerics;

namespace Rusty.Engine;

/// <summary>
/// The viewport-local projection of one world point. <see cref="Depth"/> is the signed distance
/// along the camera's forward axis: positive values are in front of the camera.
/// </summary>
public readonly record struct CameraProjectionResult(
    Vector2 NormalizedPoint,
    double Depth,
    bool InFront,
    bool InClip);

/// <summary>A copied world-space ray derived from one camera and viewport-local point.</summary>
public readonly record struct CameraRay(Vector3 Origin, Vector3 Direction);

/// <summary>
/// Stateless camera projection and unprojection matching Rusty Engine camera presentation.
/// The caller supplies the aspect ratio of its selected viewport; this helper never reads a live
/// host surface, renderer, or camera handle. Normalized points use a bottom-left origin, where
/// <c>(0, 0)</c> is bottom-left and <c>(1, 1)</c> is top-right.
/// </summary>
public static class CameraQueries
{
    private const double DegreesToRadians = Math.PI / 180d;
    private const double MinimumBasisLengthSquared = 1e-12d;

    /// <summary>
    /// Derives a <see cref="CameraBasisMode.Derived"/> pose that faces <paramref name="target"/>
    /// from <paramref name="position"/>. The result uses the same degrees, yaw direction, and
    /// pitch convention as camera presentation: zero yaw faces negative Z and positive yaw faces
    /// positive X. When the target is directly above or below the position,
    /// <paramref name="verticalYawDegrees"/> supplies the otherwise indeterminate yaw.
    /// Returns <see langword="false"/> with a default pose for non-finite inputs or coincident
    /// points.
    /// </summary>
    public static bool TryLookAtPose(
        Vector3 position,
        Vector3 target,
        double verticalYawDegrees,
        out CameraPose pose)
    {
        pose = default;
        if (!IsFinite(position) || !IsFinite(target) || !double.IsFinite(verticalYawDegrees))
        {
            return false;
        }

        double x = (double)target.X - position.X;
        double y = (double)target.Y - position.Y;
        double z = (double)target.Z - position.Z;
        double lengthSquared = x * x + y * y + z * z;
        if (lengthSquared == 0d)
        {
            return false;
        }

        double inverseLength = 1d / Math.Sqrt(lengthSquared);
        double horizontalLengthSquared = x * x + z * z;
        double pitchDegrees = Math.Asin(Math.Clamp(y * inverseLength, -1d, 1d)) / DegreesToRadians;
        double yawDegrees = horizontalLengthSquared == 0d
            ? verticalYawDegrees
            : Math.Atan2(x, -z) / DegreesToRadians;
        pose = new CameraPose(position, pitchDegrees, yawDegrees);
        return true;
    }

    /// <summary>
    /// Projects <paramref name="worldPoint"/> into the caller-selected viewport aspect.
    /// <see cref="CameraProjectionResult.InClip"/> includes both the camera near/far range and
    /// viewport bounds; offscreen and behind points still return their mathematical projection.
    /// A point on the perspective camera plane has no finite normalized projection and is marked
    /// outside the clip volume.
    /// </summary>
    public static CameraProjectionResult Project(
        CameraDescriptor camera,
        double aspect,
        Vector3 worldPoint)
    {
        CameraFrame frame = Resolve(camera, aspect);
        RequireFinite(worldPoint, nameof(worldPoint));

        Vector3 local = worldPoint - camera.Pose.Position;
        double horizontal = Dot(local, frame.Right);
        double vertical = Dot(local, frame.Up);
        double depth = Dot(local, frame.Forward);
        Vector2 normalized = frame.Projection.Kind switch
        {
            CameraProjectionKind.Perspective => PerspectiveProjection(
                horizontal, vertical, depth, frame.Aspect, frame.Projection.FovYDegrees),
            CameraProjectionKind.Orthographic => OrthographicProjection(
                horizontal, vertical, frame.Aspect, frame.Projection.VerticalSize),
            _ => throw new ArgumentException("Camera projection kind is not supported.", nameof(camera)),
        };

        bool inFront = depth > 0d;
        bool inClip = inFront
            && depth >= frame.Projection.Near
            && depth <= frame.Projection.Far
            && normalized.X >= 0f
            && normalized.X <= 1f
            && normalized.Y >= 0f
            && normalized.Y <= 1f;
        return new CameraProjectionResult(normalized, depth, inFront, inClip);
    }

    /// <summary>
    /// Returns the world-space ray through a normalized viewport-local point. Perspective rays
    /// originate at the camera pose; orthographic rays originate on its near plane.
    /// </summary>
    public static CameraRay Ray(CameraDescriptor camera, double aspect, Vector2 normalizedPoint)
    {
        CameraFrame frame = Resolve(camera, aspect);
        if (!float.IsFinite(normalizedPoint.X) || !float.IsFinite(normalizedPoint.Y))
        {
            throw new ArgumentException("Normalized point must be finite.", nameof(normalizedPoint));
        }

        double x = normalizedPoint.X * 2d - 1d;
        double y = normalizedPoint.Y * 2d - 1d;
        return frame.Projection.Kind switch
        {
            CameraProjectionKind.Perspective => PerspectiveRay(frame, x, y),
            CameraProjectionKind.Orthographic => OrthographicRay(frame, x, y),
            _ => throw new ArgumentException("Camera projection kind is not supported.", nameof(camera)),
        };
    }

    private static CameraRay PerspectiveRay(CameraFrame frame, double x, double y)
    {
        double tangent = Math.Tan(frame.Projection.FovYDegrees * DegreesToRadians * 0.5d);
        Vector3 direction = Normalize(
            Add(frame.Forward,
                Scale(frame.Right, x * frame.Aspect * tangent),
                Scale(frame.Up, y * tangent)));
        return new CameraRay(frame.Position, direction);
    }

    private static CameraRay OrthographicRay(CameraFrame frame, double x, double y)
    {
        double halfHeight = frame.Projection.VerticalSize * 0.5d;
        double halfWidth = halfHeight * frame.Aspect;
        Vector3 origin = Add(
            frame.Position,
            Scale(frame.Right, x * halfWidth),
            Scale(frame.Up, y * halfHeight),
            Scale(frame.Forward, frame.Projection.Near));
        return new CameraRay(origin, frame.Forward);
    }

    private static Vector2 PerspectiveProjection(
        double horizontal,
        double vertical,
        double depth,
        double aspect,
        double fovYDegrees)
    {
        double tangent = Math.Tan(fovYDegrees * DegreesToRadians * 0.5d);
        return ToVector2(
            0.5d + horizontal / (2d * depth * aspect * tangent),
            0.5d + vertical / (2d * depth * tangent));
    }

    private static Vector2 OrthographicProjection(
        double horizontal,
        double vertical,
        double aspect,
        double verticalSize)
    {
        double halfHeight = verticalSize * 0.5d;
        return ToVector2(
            0.5d + horizontal / (2d * halfHeight * aspect),
            0.5d + vertical / (2d * halfHeight));
    }

    private static CameraFrame Resolve(CameraDescriptor camera, double aspect)
    {
        RequireFinite(aspect, nameof(aspect));
        if (aspect <= 0d) throw new ArgumentOutOfRangeException(nameof(aspect), "Aspect must be greater than zero.");
        RequireFinite(camera.Pose.Position, nameof(camera));
        RequireFinite(camera.Pose.PitchDegrees, nameof(camera));
        RequireFinite(camera.Pose.YawDegrees, nameof(camera));
        ValidateProjection(camera.Projection);

        return camera.BasisMode switch
        {
            CameraBasisMode.Derived => DerivedFrame(camera.Pose.Position, camera.Pose.PitchDegrees, camera.Pose.YawDegrees, camera.Projection, aspect),
            CameraBasisMode.Explicit => ExplicitFrame(camera.Pose.Position, camera.Basis, camera.Projection, aspect),
            _ => throw new ArgumentException("Camera basis mode is not supported.", nameof(camera)),
        };
    }

    private static CameraFrame DerivedFrame(
        Vector3 position,
        double pitchDegrees,
        double yawDegrees,
        CameraProjection projection,
        double aspect)
    {
        double pitch = pitchDegrees * DegreesToRadians;
        double yaw = yawDegrees * DegreesToRadians;
        double sinPitch = Math.Sin(pitch);
        double cosPitch = Math.Cos(pitch);
        double sinYaw = Math.Sin(yaw);
        double cosYaw = Math.Cos(yaw);
        return new CameraFrame(
            position,
            ToVector3(sinYaw * cosPitch, sinPitch, -cosYaw * cosPitch),
            ToVector3(cosYaw, 0d, sinYaw),
            ToVector3(-sinYaw * sinPitch, cosPitch, cosYaw * sinPitch),
            projection,
            aspect);
    }

    private static CameraFrame ExplicitFrame(
        Vector3 position,
        CameraBasis basis,
        CameraProjection projection,
        double aspect)
    {
        RequireFinite(basis.Forward, nameof(basis));
        RequireFinite(basis.Right, nameof(basis));
        RequireFinite(basis.Up, nameof(basis));

        // This matches the renderer's explicit-basis realization: Three consumes Forward and Up
        // through lookAt, then derives a perpendicular right axis. The descriptor's Right is
        // still checked for a valid copied descriptor, but does not override that realization.
        Vector3 forward = Normalize(basis.Forward);
        Vector3 right = Normalize(Cross(forward, basis.Up));
        Vector3 up = Normalize(Cross(right, forward));
        return new CameraFrame(position, forward, right, up, projection, aspect);
    }

    private static void ValidateProjection(CameraProjection projection)
    {
        RequireFinite(projection.Near, nameof(projection));
        RequireFinite(projection.Far, nameof(projection));
        if (projection.Near <= 0d || projection.Far <= projection.Near)
        {
            throw new ArgumentException("Camera near/far values must be positive and ordered.", nameof(projection));
        }

        switch (projection.Kind)
        {
            case CameraProjectionKind.Perspective
                when double.IsFinite(projection.FovYDegrees)
                && projection.FovYDegrees > 0d
                && projection.FovYDegrees < 180d:
            return;
            case CameraProjectionKind.Orthographic
                when double.IsFinite(projection.VerticalSize)
                && projection.VerticalSize > 0d:
            return;
            case CameraProjectionKind.Perspective:
                throw new ArgumentException("Perspective vertical field of view must be between zero and 180 degrees.", nameof(projection));
            case CameraProjectionKind.Orthographic:
                throw new ArgumentException("Orthographic vertical size must be greater than zero.", nameof(projection));
            default:
                throw new ArgumentException("Camera projection kind is not supported.", nameof(projection));
        }
    }

    private static Vector3 Add(Vector3 first, Vector3 second, Vector3 third = default, Vector3 fourth = default) =>
        ToVector3(
            first.X + second.X + third.X + fourth.X,
            first.Y + second.Y + third.Y + fourth.Y,
            first.Z + second.Z + third.Z + fourth.Z);

    private static Vector3 Scale(Vector3 value, double scale) =>
        ToVector3(value.X * scale, value.Y * scale, value.Z * scale);

    private static Vector3 Cross(Vector3 first, Vector3 second) =>
        ToVector3(
            (double)first.Y * second.Z - (double)first.Z * second.Y,
            (double)first.Z * second.X - (double)first.X * second.Z,
            (double)first.X * second.Y - (double)first.Y * second.X);

    private static double Dot(Vector3 first, Vector3 second) =>
        (double)first.X * second.X + (double)first.Y * second.Y + (double)first.Z * second.Z;

    private static Vector3 Normalize(Vector3 value)
    {
        double lengthSquared = Dot(value, value);
        if (!double.IsFinite(lengthSquared) || lengthSquared <= MinimumBasisLengthSquared)
        {
            throw new ArgumentException("Camera basis must provide non-parallel, non-zero forward and up vectors.");
        }
        return Scale(value, 1d / Math.Sqrt(lengthSquared));
    }

    private static Vector2 ToVector2(double x, double y)
    {
        return new Vector2((float)x, (float)y);
    }

    private static Vector3 ToVector3(double x, double y, double z)
    {
        if (!double.IsFinite(x) || !double.IsFinite(y) || !double.IsFinite(z)
            || Math.Abs(x) > float.MaxValue || Math.Abs(y) > float.MaxValue || Math.Abs(z) > float.MaxValue)
        {
            throw new ArgumentException("Camera query result is not representable as a world vector.");
        }
        return new Vector3((float)x, (float)y, (float)z);
    }

    private static void RequireFinite(double value, string parameterName)
    {
        if (!double.IsFinite(value)) throw new ArgumentException("Camera query inputs must be finite.", parameterName);
    }

    private static void RequireFinite(Vector3 value, string parameterName)
    {
        if (!IsFinite(value))
        {
            throw new ArgumentException("Camera query inputs must be finite.", parameterName);
        }
    }

    private static bool IsFinite(Vector3 value) =>
        float.IsFinite(value.X) && float.IsFinite(value.Y) && float.IsFinite(value.Z);

    private readonly record struct CameraFrame(
        Vector3 Position,
        Vector3 Forward,
        Vector3 Right,
        Vector3 Up,
        CameraProjection Projection,
        double Aspect);
}
