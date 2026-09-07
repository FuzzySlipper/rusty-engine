using System.Numerics;

namespace Rusty.Engine.Implicit;

/// <summary>
/// A scoped, managed vocabulary over one Engine implicit field. It owns no
/// evaluator, generated mesh, appearance, collision world, or product state.
/// </summary>
public sealed class ImplicitRecipe : IDisposable
{
    private readonly IImplicitSurfacesService _service;

    public ImplicitRecipe(IImplicitSurfacesService service)
    {
        _service = service ?? throw new ArgumentNullException(nameof(service));
        Field = _service.CreateField();
    }

    /// <summary>The field that remains alive until this recipe is disposed.</summary>
    public ImplicitField Field { get; }

    public ImplicitNode Box(Vector3 minimum, Vector3 maximum) =>
        _service.AddBox(new ImplicitBoxRequest(Field, minimum, maximum));

    public ImplicitNode Sphere(Vector3 center, float radius) =>
        _service.AddSphere(new ImplicitSphereRequest(Field, center, radius));

    public ImplicitNode Ellipsoid(Vector3 center, Vector3 radii) =>
        _service.AddEllipsoid(new ImplicitEllipsoidRequest(Field, center, radii));

    public ImplicitNode Capsule(Vector3 start, Vector3 end, float radius) =>
        _service.AddCapsule(new ImplicitCapsuleRequest(Field, start, end, radius));

    public ImplicitNode Frustum(Vector3 start, Vector3 end, float startRadius, float endRadius) =>
        _service.AddFrustum(new ImplicitFrustumRequest(Field, start, end, startRadius, endRadius));

    public ImplicitNode Plane(Vector3 normal, float offset) =>
        _service.AddPlane(new ImplicitPlaneRequest(Field, normal, offset));

    public ImplicitNode Union(ImplicitNode left, ImplicitNode right) =>
        _service.Union(new ImplicitBinaryRequest(Field, left, right));

    public ImplicitNode Intersect(ImplicitNode left, ImplicitNode right) =>
        _service.Intersection(new ImplicitBinaryRequest(Field, left, right));

    public ImplicitNode Subtract(ImplicitNode left, ImplicitNode right) =>
        _service.Difference(new ImplicitBinaryRequest(Field, left, right));

    public ImplicitNode Blend(ImplicitNode left, ImplicitNode right, float radius) =>
        _service.SmoothUnion(new ImplicitBlendRequest(Field, left, right, radius));

    public ImplicitNode Offset(ImplicitNode source, float amount) =>
        _service.Offset(new ImplicitOffsetRequest(Field, source, amount));

    public ImplicitNode Translate(ImplicitNode source, Vector3 translation) =>
        Place(source, new Transform(translation, Quaternion.Identity, Vector3.One));

    public ImplicitNode Place(ImplicitNode source, Transform placement) =>
        _service.Transform(new ImplicitTransformRequest(Field, source, placement));

    public ImplicitSample Sample(ImplicitNode source, Vector3 position) =>
        _service.Sample(new ImplicitSampleRequest(Field, source, position));

    /// <summary>Generates a mesh while the recipe's field is still retained.</summary>
    public MeshResource Generate(RecipeSurface surface) => _service.Generate(new ImplicitGenerateRequest(
        surface.Field,
        surface.Root,
        surface.Min,
        surface.Max,
        surface.Sampling.CellSize,
        surface.Sampling.CreaseDegrees,
        surface.Sampling.TextureRepeats,
        surface.Material,
        surface.Regions,
        surface.Sampling.MaterialBoundaries,
        surface.Sampling.MaterialSampleSpacing));

    public void Dispose() => Field.Dispose();
}

/// <summary>Product-selected sampling and material-boundary settings for one surface.</summary>
public readonly record struct RecipeSampling(
    float CellSize,
    float CreaseDegrees,
    float TextureRepeats,
    ImplicitMaterialBoundaryMode MaterialBoundaries,
    float MaterialSampleSpacing = 0f);

/// <summary>
/// A synchronous description of one field extraction. The receiving callback
/// must consume it before the recipe that owns <see cref="Field"/> is disposed.
/// </summary>
public sealed record RecipeSurface(
    string Name,
    ImplicitField Field,
    ImplicitNode Root,
    Vector3 Min,
    Vector3 Max,
    Material Material,
    ReadOnlyMemory<ImplicitMaterialRegion> Regions,
    RecipeSampling Sampling,
    Transform Placement);

/// <summary>
/// Creates short-lived recipes and emits synchronous surface descriptions.
/// Products decide where meshes, appearances, and collision are published.
/// </summary>
public sealed class RecipeWriter
{
    private readonly IImplicitSurfacesService _service;
    private readonly RecipeSampling _sampling;
    private readonly Action<RecipeSurface> _emit;

    public RecipeWriter(
        IImplicitSurfacesService service,
        RecipeSampling sampling,
        Action<RecipeSurface> emit)
    {
        _service = service ?? throw new ArgumentNullException(nameof(service));
        _sampling = sampling;
        _emit = emit ?? throw new ArgumentNullException(nameof(emit));
    }

    public ImplicitRecipe Begin() => new(_service);

    public void Surface(
        string name,
        ImplicitRecipe recipe,
        ImplicitNode root,
        Vector3 min,
        Vector3 max,
        Material material,
        Transform placement,
        ImplicitMaterialRegion[]? regions = null,
        float? cellSize = null)
    {
        ArgumentNullException.ThrowIfNull(recipe);
        _emit(new RecipeSurface(
            name,
            recipe.Field,
            root,
            min,
            max,
            material,
            regions ?? [],
            _sampling with { CellSize = cellSize ?? _sampling.CellSize },
            placement));
    }

    public void Box(string name, Vector3 min, Vector3 max, Material material, Transform placement)
    {
        using ImplicitRecipe recipe = Begin();
        Vector3 extent = max - min;
        float narrowest = MathF.Min(extent.X, MathF.Min(extent.Y, extent.Z));
        Surface(
            name,
            recipe,
            recipe.Box(min, max),
            min,
            max,
            material,
            placement,
            cellSize: MathF.Min(0.5f, narrowest * 0.75f));
    }
}
