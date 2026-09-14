using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Implicit;

VerifyConvexPrisms();
VerifyWalkways();
Console.WriteLine("Planar recipe validation and construction passed.");
ContinuityExercise.Run();

static void VerifyConvexPrisms()
{
    Vector2[] counterclockwise =
    [
        new(-2f, -1f),
        new(2f, -1f),
        new(2f, 1f),
        new(-2f, 1f),
    ];
    Vector2[] clockwise = counterclockwise.Reverse().ToArray();
    var service = new RecordingImplicitSurfacesService();
    using var recipe = new ImplicitRecipe(service);

    PlanarRecipes.ConvexPrism(recipe, counterclockwise, -2f, 3f, expansion: 0.25f);
    PlaneSignature[] counterclockwisePlanes = service.Planes.Select(PlaneSignature.From).Order().ToArray();
    Require(service.PlaneCount == 6 && service.IntersectionCount == 5,
        "a four-sided prism did not use two height planes and four side planes");

    PlanarRecipes.ConvexPrism(recipe, clockwise, -2f, 3f, expansion: 0.25f);
    PlaneSignature[] clockwisePlanes = service.Planes.Skip(6).Select(PlaneSignature.From).Order().ToArray();
    Require(counterclockwisePlanes.SequenceEqual(clockwisePlanes),
        "clockwise and counterclockwise prisms produced different half-space constraints");
    Require(service.FieldOperationCount == 22,
        "prism construction did not compose its existing field operations predictably");

    RequireRejectedWithoutMutation(service, "concave polygon", () =>
        PlanarRecipes.ConvexPrism(recipe,
        [new(0f, 0f), new(3f, 0f), new(1f, 1f), new(3f, 3f), new(0f, 3f)], 0f, 1f));
    RequireRejectedWithoutMutation(service, "self-crossing polygon", () =>
        PlanarRecipes.ConvexPrism(recipe,
        [new(0f, 0f), new(3f, 3f), new(0f, 3f), new(3f, 0f)], 0f, 1f));
    RequireRejectedWithoutMutation(service, "self-crossing same-turn polygon", () =>
        PlanarRecipes.ConvexPrism(recipe,
        [
            new(0f, -1f),
            new(0.58778524f, 0.809017f),
            new(-0.95105654f, -0.309017f),
            new(0.95105654f, -0.309017f),
            new(-0.58778524f, 0.809017f),
        ], 0f, 1f));
    RequireRejectedWithoutMutation(service, "repeated vertex", () =>
        PlanarRecipes.ConvexPrism(recipe,
        [new(0f, 0f), new(3f, 0f), new(3f, 0f), new(0f, 3f)], 0f, 1f));
    RequireRejectedWithoutMutation(service, "non-finite vertex", () =>
        PlanarRecipes.ConvexPrism(recipe,
        [new(0f, 0f), new(float.NaN, 0f), new(0f, 3f)], 0f, 1f));
    RequireRejectedWithoutMutation(service, "invalid vertical range", () =>
        PlanarRecipes.ConvexPrism(recipe, counterclockwise, 1f, 1f));
    RequireRejectedWithoutMutation(service, "negative expansion", () =>
        PlanarRecipes.ConvexPrism(recipe, counterclockwise, 0f, 1f, expansion: -0.01f));
}

static void VerifyWalkways()
{
    var service = new RecordingImplicitSurfacesService();
    using var recipe = new ImplicitRecipe(service);

    PlanarRecipes.Walkway(recipe,
    [
        new(0f, 0f),
        new(4f, 0f),
        new(4f, 3f),
    ], halfWidth: 0.5f, bottom: -1f, top: 0f);
    Require(service.PlaneCount == 12 && service.IntersectionCount == 10 && service.UnionCount == 1,
        "two square-capped walkway segments were not prism-unioned");

    var invalidService = new RecordingImplicitSurfacesService();
    using var invalidRecipe = new ImplicitRecipe(invalidService);
    RequireRejectedWithoutMutation(invalidService, "late zero-length walkway segment", () =>
        PlanarRecipes.Walkway(invalidRecipe,
        [
            new(0f, 0f),
            new(4f, 0f),
            new(4f, 0f),
            new(4f, 3f),
        ], halfWidth: 0.5f, bottom: -1f, top: 0f));
    RequireRejectedWithoutMutation(invalidService, "non-positive walkway width", () =>
        PlanarRecipes.Walkway(invalidRecipe, [new(0f, 0f), new(1f, 0f)], 0f, -1f, 0f));
    RequireRejectedWithoutMutation(invalidService, "non-finite walkway point", () =>
        PlanarRecipes.Walkway(invalidRecipe,
        [new(0f, 0f), new(float.PositiveInfinity, 0f)], 0.5f, -1f, 0f));
}

static void RequireRejectedWithoutMutation(
    RecordingImplicitSurfacesService service,
    string description,
    Action action)
{
    int before = service.FieldOperationCount;
    try
    {
        action();
        throw new InvalidOperationException($"{description} was accepted");
    }
    catch (ArgumentException)
    {
        Require(service.FieldOperationCount == before,
            $"{description} changed the field before validation completed");
    }
}

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}

readonly record struct PlaneSignature(float X, float Y, float Z, float Offset) : IComparable<PlaneSignature>
{
    public static PlaneSignature From(ImplicitPlaneRequest plane) =>
        new(plane.Normal.X, plane.Normal.Y, plane.Normal.Z, plane.Offset);

    public int CompareTo(PlaneSignature other)
    {
        int result = X.CompareTo(other.X);
        result = result != 0 ? result : Y.CompareTo(other.Y);
        result = result != 0 ? result : Z.CompareTo(other.Z);
        return result != 0 ? result : Offset.CompareTo(other.Offset);
    }
}

sealed class RecordingImplicitSurfacesService : IImplicitSurfacesService
{
    private ulong _nextNode;

    public List<ImplicitPlaneRequest> Planes { get; } = [];
    public List<ImplicitBoxRequest> Boxes { get; } = [];
    public int PlaneCount => Planes.Count;
    public int BoxCount => Boxes.Count;
    public int IntersectionCount { get; private set; }
    public int UnionCount { get; private set; }
    public int DifferenceCount { get; private set; }
    public int FieldOperationCount => PlaneCount + BoxCount + IntersectionCount + UnionCount + DifferenceCount;

    public ImplicitField CreateField() => new(new ImplicitFieldHandle(1), static () => { });
    public ImplicitNode AddPlane(ImplicitPlaneRequest arg0)
    {
        Planes.Add(arg0);
        return NextNode();
    }
    public ImplicitNode AddBox(ImplicitBoxRequest arg0)
    {
        Boxes.Add(arg0);
        return NextNode();
    }
    public ImplicitNode Intersection(ImplicitBinaryRequest arg0)
    {
        IntersectionCount++;
        return NextNode();
    }
    public ImplicitNode Union(ImplicitBinaryRequest arg0)
    {
        UnionCount++;
        return NextNode();
    }
    public ImplicitNode Difference(ImplicitBinaryRequest arg0)
    {
        DifferenceCount++;
        return NextNode();
    }

    public SampledVolume CreateSampledVolume(SampledVolumeCreateRequest arg0) => Unsupported<SampledVolume>();
    public SampledVolumeDescriptor DescribeSampledVolume(SampledVolume arg0) => Unsupported<SampledVolumeDescriptor>();
    public void WriteSampledVolume(SampledVolumeWriteRequest arg0) => Unsupported();
    public DensitySnapshotLeaseReceipt ReadSampledVolume(SampledVolumeReadRequest arg0) => Unsupported<DensitySnapshotLeaseReceipt>();
    public DensitySample SampleSampledVolume(SampledVolumeSampleRequest arg0) => Unsupported<DensitySample>();
    public void RasterizeSampledVolume(SampledVolumeRasterizeRequest arg0) => Unsupported();
    public MeshResource GenerateSampledVolume(SampledVolumeGenerateRequest arg0) => Unsupported<MeshResource>();
    public ImplicitGenerationReadout ReadSampledVolumeGeneration(SampledVolume arg0) => Unsupported<ImplicitGenerationReadout>();
    public ImplicitNode AddSphere(ImplicitSphereRequest arg0) => Unsupported<ImplicitNode>();
    public ImplicitNode AddEllipsoid(ImplicitEllipsoidRequest arg0) => Unsupported<ImplicitNode>();
    public ImplicitNode AddCapsule(ImplicitCapsuleRequest arg0) => Unsupported<ImplicitNode>();
    public ImplicitNode SmoothUnion(ImplicitBlendRequest arg0) => Unsupported<ImplicitNode>();
    public ImplicitNode Offset(ImplicitOffsetRequest arg0) => Unsupported<ImplicitNode>();
    public ImplicitNode Transform(ImplicitTransformRequest arg0) => Unsupported<ImplicitNode>();
    public ImplicitSample Sample(ImplicitSampleRequest arg0) => Unsupported<ImplicitSample>();
    public MeshResource Generate(ImplicitGenerateRequest arg0) => Unsupported<MeshResource>();
    public ImplicitGenerationReadout ReadGeneration(ImplicitField arg0) => Unsupported<ImplicitGenerationReadout>();
    public ImplicitNode AddFrustum(ImplicitFrustumRequest arg0) => Unsupported<ImplicitNode>();
    public ImplicitNode DisplaceWaves(ImplicitWaveRequest arg0) => Unsupported<ImplicitNode>();
    public ImplicitAudit CreateAudit() => Unsupported<ImplicitAudit>();
    public void CaptureAuditPiece(ImplicitAuditPieceRequest arg0) => Unsupported();
    public ImplicitAuditReportLeaseReceipt ReadAudit(ImplicitAuditRequest arg0) => Unsupported<ImplicitAuditReportLeaseReceipt>();
    public ImplicitAnalysisReportLeaseReceipt ReadMeshIntegrity(ImplicitIntegrityRequest arg0) => Unsupported<ImplicitAnalysisReportLeaseReceipt>();
    public ImplicitAnalysisReportLeaseReceipt ReadExpectedJoin(ImplicitJoinRequest arg0) => Unsupported<ImplicitAnalysisReportLeaseReceipt>();
    public ImplicitAnalysisReportLeaseReceipt ReadEnclosure(ImplicitEnclosureRequest arg0) => Unsupported<ImplicitAnalysisReportLeaseReceipt>();

    private ImplicitNode NextNode() => new(++_nextNode);
    private static void Unsupported() => throw new NotSupportedException("This focused helper harness only records recipe composition.");
    private static T Unsupported<T>() => throw new NotSupportedException("This focused helper harness only records recipe composition.");
}
