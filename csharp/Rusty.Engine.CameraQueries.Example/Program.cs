using System.Numerics;
using Rusty.Engine;

const double Epsilon = 0.0001d;

ExercisePerspectiveRoundTrip();
ExerciseOrthographicRoundTrip();
ExerciseDerivedYawAndPose();
ExerciseExplicitBasisAndPose();
ExerciseVisibilityFacts();
ExerciseAspect();

static void ExercisePerspectiveRoundTrip()
{
    CameraDescriptor camera = DerivedCamera(CameraProjectionKind.Perspective, fovYDegrees: 70d, verticalSize: 0d);
    Vector3 point = new(2f, 1f, -8f);
    CameraProjectionResult projection = CameraQueries.Project(camera, 16d / 9d, point);
    CameraRay ray = CameraQueries.Ray(camera, 16d / 9d, projection.NormalizedPoint);

    Require(projection.InFront && projection.InClip, "perspective point was not classified inside its clip volume");
    RequireClose(projection.Depth, 8d, "perspective depth was not camera-forward distance");
    RequireRayContains(ray, point, "perspective projection did not round-trip through its ray");
}

static void ExerciseOrthographicRoundTrip()
{
    CameraDescriptor camera = DerivedCamera(CameraProjectionKind.Orthographic, fovYDegrees: 0d, verticalSize: 6d);
    Vector3 point = new(-3f, 1.5f, -5f);
    CameraProjectionResult projection = CameraQueries.Project(camera, 2d, point);
    CameraRay ray = CameraQueries.Ray(camera, 2d, projection.NormalizedPoint);

    Require(projection.InFront && projection.InClip, "orthographic point was not classified inside its clip volume");
    RequireClose(projection.Depth, 5d, "orthographic depth was not camera-forward distance");
    RequireClose(ray.Origin.Z, -0.1d, "orthographic ray did not begin on the near plane");
    RequireRayContains(ray, point, "orthographic projection did not round-trip through its ray");
}

static void ExerciseDerivedYawAndPose()
{
    CameraDescriptor camera = new(
        new CameraPose(new Vector3(5f, 2f, 1f), 0d, 90d),
        CameraBasisMode.Derived,
        default,
        new CameraProjection(CameraProjectionKind.Perspective, 70d, 0d, 0.1d, 100d),
        new CameraViewport(0d, 0d, 1d, 1d));
    Vector3 point = new(12f, 2f, 1f);
    CameraProjectionResult projection = CameraQueries.Project(camera, 1d, point);
    CameraRay ray = CameraQueries.Ray(camera, 1d, projection.NormalizedPoint);

    Require(projection.NormalizedPoint == new Vector2(0.5f, 0.5f),
        "positive derived yaw did not face canonical positive X");
    RequireClose(projection.Depth, 7d, "derived yaw did not preserve nonzero camera position");
    RequireRayContains(ray, point, "derived yaw ray did not match the renderer convention");
}

static void ExerciseExplicitBasisAndPose()
{
    CameraDescriptor camera = new(
        new CameraPose(new Vector3(5f, 2f, 1f), 12d, -25d),
        CameraBasisMode.Explicit,
        new CameraBasis(new Vector3(1f, 0f, 0f), Vector3.Zero, new Vector3(0f, 1f, 0f)),
        new CameraProjection(CameraProjectionKind.Perspective, 90d, 0d, 0.25d, 40d),
        new CameraViewport(0d, 0d, 1d, 1d));
    Vector3 point = new(11f, 2f, 1f);
    CameraProjectionResult projection = CameraQueries.Project(camera, 1d, point);
    CameraRay ray = CameraQueries.Ray(camera, 1d, projection.NormalizedPoint);

    Require(projection.NormalizedPoint == new Vector2(0.5f, 0.5f), "explicit basis did not take precedence over pose angles");
    RequireClose(projection.Depth, 6d, "explicit-basis depth did not use explicit forward");
    RequireRayContains(ray, point, "explicit basis ray did not preserve the nonzero pose");
}

static void ExerciseVisibilityFacts()
{
    CameraDescriptor camera = DerivedCamera(CameraProjectionKind.Perspective, fovYDegrees: 90d, verticalSize: 0d);
    CameraProjectionResult offscreen = CameraQueries.Project(camera, 1d, new Vector3(10f, 0f, -2f));
    CameraProjectionResult behind = CameraQueries.Project(camera, 1d, new Vector3(0f, 0f, 2f));

    Require(offscreen.InFront && !offscreen.InClip && offscreen.NormalizedPoint.X > 1f,
        "offscreen point did not preserve front and clip facts");
    Require(!behind.InFront && !behind.InClip && behind.Depth < 0d,
        "behind point did not preserve signed depth and clip facts");
}

static void ExerciseAspect()
{
    CameraDescriptor camera = DerivedCamera(CameraProjectionKind.Perspective, fovYDegrees: 90d, verticalSize: 0d);
    Vector3 point = new(2f, 0f, -4f);
    CameraProjectionResult square = CameraQueries.Project(camera, 1d, point);
    CameraProjectionResult wide = CameraQueries.Project(camera, 2d, point);

    RequireClose(square.NormalizedPoint.X, 0.75d, "square aspect projection was incorrect");
    RequireClose(wide.NormalizedPoint.X, 0.625d, "explicit wide aspect projection was incorrect");
}

static CameraDescriptor DerivedCamera(CameraProjectionKind kind, double fovYDegrees, double verticalSize) => new(
    new CameraPose(Vector3.Zero, 0d, 0d),
    CameraBasisMode.Derived,
    default,
    new CameraProjection(kind, fovYDegrees, verticalSize, 0.1d, 100d),
    new CameraViewport(0d, 0d, 1d, 1d));

static void RequireRayContains(CameraRay ray, Vector3 point, string message)
{
    Vector3 towardPoint = point - ray.Origin;
    float distance = Vector3.Dot(towardPoint, ray.Direction);
    Require(distance >= 0f && Vector3.Distance(ray.Origin + ray.Direction * distance, point) < Epsilon, message);
}

static void RequireClose(double actual, double expected, string message) =>
    Require(Math.Abs(actual - expected) < Epsilon, message);

static void Require(bool condition, string message)
{
    if (!condition) throw new InvalidOperationException(message);
}
