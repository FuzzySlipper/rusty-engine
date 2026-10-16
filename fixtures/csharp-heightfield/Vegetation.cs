using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.VegetationTiles;

namespace CsharpHeightfield;

/// <summary>
/// Grass and trees over the hills (#10016): two Engine vegetation fields sampled from the same
/// ground the heightfield draws, with no voxels or collision behind them. The near field's 1 m
/// lattice is the heightfield's finest ring; the far field's 4 m lattice is its middle ring, so the
/// grass stands on the triangles drawn there. Grass is one layer in both fields with the same key,
/// seed and density: the near field's copies shrink away over the band where the far field's grow,
/// one for one, and thin with distance, the kept clumps growing so the hills stay as green (#10017).
/// Trees are placement records the product chooses per far tile: the full cone near, a cheaper cone
/// from <see cref="TreeMidMetres"/>, and from <see cref="TreeImpostorMetres"/> a card showing the full
/// cone's baked view, crossfading between each.
/// </summary>
internal sealed class Vegetation : IDisposable
{
    private const uint GrassLayer = 1, TreeLayer = 2;
    private const uint GrassSeed = 7;
    private const double NearTileMetres = 32, FarTileMetres = 128;
    private const uint NearCells = 32, FarCells = 32;
    private const float HandOver = 40, HandOverBand = 4;
    private const float DefaultGrassDensity = 1.5f, DefaultReach = 400, TreeReach = 800;
    private const uint MaximumGrass = 1_000_000, MaximumTrees = 50_000;
    private const double TreeCell = 14, TreeSink = .3;
    /// <summary>Where trees take their cheaper cone and then their impostor card, and the band they crossfade over.</summary>
    private const float TreeMidMetres = 150, TreeImpostorMetres = 320, LodBand = 10;
    /// <summary>The far grass keeps every clump to the first distance, a third by the second, growing up to 1.7 times.</summary>
    private const float ThinFrom = 80, ThinTo = 400, ThinShare = .33f, ThinGrowth = 1.7f;
    private const uint Grassland = 0;

    private readonly IEngineContext engine;
    private readonly Camera camera;
    private readonly Material grassPaint, treePaint;
    private readonly MeshResource grassMesh, treeMesh, treeMidMesh;
    private readonly Appearance grassLook, treeLook, treeMidLook;
    private VegetationField? near, far;
    private VegetationStreamer? nearStreamer, farStreamer;

    internal Vegetation(IEngineContext engine, Camera camera)
    {
        this.engine = engine;
        this.camera = camera;
        Color one = new(1, 1, 1, 1);
        grassPaint = engine.Graphics.CreateMaterial(new MaterialRequest(new(.2f, .42f, .1f, 1), default, .9f, one, Vector3.Zero, 0, true) with
        {
            WindBend = .08f,
            WindFlutter = .06f,
        });
        treePaint = engine.Graphics.CreateMaterial(new MaterialRequest(new(.08f, .2f, .07f, 1), default, .9f, one, Vector3.Zero, 0, false) with
        {
            FlatShading = true,
        });
        grassMesh = engine.Graphics.CreateMeshResource(Clump(grassPaint));
        treeMesh = engine.Graphics.CreateMeshResource(Tree(treePaint, 6));
        treeMidMesh = engine.Graphics.CreateMeshResource(Tree(treePaint, 3));
        grassLook = engine.Graphics.CreateMeshAppearance(grassMesh);
        treeLook = engine.Graphics.CreateMeshAppearance(treeMesh);
        treeMidLook = engine.Graphics.CreateMeshAppearance(treeMidMesh);
    }

    internal float GrassDensity { get; private set; } = DefaultGrassDensity;
    internal float Reach { get; private set; } = DefaultReach;
    internal bool Shown => near is not null;

    /// <summary>Makes both fields, or remakes them with the current density and reach.</summary>
    internal void Show(bool show)
    {
        Hide();
        if (!show) return;
        VegetationFocus focus = VegetationFocus.Following(camera);
        near = engine.Vegetation.Create(new VegetationFieldCreateRequest(default, NearTileMetres, NearCells, focus));
        far = engine.Vegetation.Create(new VegetationFieldCreateRequest(default, FarTileMetres, FarCells, focus));
        Layers();
        nearStreamer = new VegetationStreamer(engine, near, new Cover(false));
        farStreamer = new VegetationStreamer(engine, far, new Cover(true));
    }

    internal void Tune(float grassDensity, float reach)
    {
        GrassDensity = grassDensity;
        Reach = Math.Max(reach, HandOver + HandOverBand + 1);
        if (Shown) Layers();
    }

    private void Layers()
    {
        VegetationLayerRequest grass = new VegetationLayerRequest(near!, GrassLayer, grassLook, grassPaint, GrassDensity, HandOver + HandOverBand) with
        {
            Grounds = VegetationLayerRequest.Ground(Grassland),
            Seed = GrassSeed,
            Fade = HandOverBand,
            ScaleMin = .7f,
            ScaleMax = 1.3f,
            TintLow = new(.8f, .85f, .7f),
            TintHigh = new(1.1f, 1.05f, .9f),
            Align = .4f,
            MaximumInstances = MaximumGrass,
        };
        engine.Vegetation.SetLayer(grass);
        // The same spots farther out, growing in as the near field's shrink away.
        engine.Vegetation.SetLayer(grass with
        {
            Field = far!,
            Reach = Reach,
            Fade = Reach * .2f,
            Near = HandOver,
            NearFade = HandOverBand,
        });
        engine.Vegetation.SetLayerDetail(new VegetationLayerDetailRequest(far!, GrassLayer, ReadOnlyMemory<VegetationLod>.Empty, 0,
            new VegetationThinningPoint[] { new(ThinFrom, 1), new(ThinTo, ThinShare) }, ThinGrowth));
        engine.Vegetation.SetLayer(new VegetationLayerRequest(far!, TreeLayer, treeLook, default(OptionalMaterial), 0, TreeReach) with
        {
            Fade = TreeReach * .1f,
            TintLow = new(.8f, .9f, .8f),
            TintHigh = new(1.1f, 1.1f, 1f),
            MaximumInstances = MaximumTrees,
            CastsShadows = true,
        });
        engine.Vegetation.SetLayerDetail(new VegetationLayerDetailRequest(far!, TreeLayer,
            new[] { VegetationLod.Mesh(treeMidLook, TreeMidMetres), VegetationLod.ImpostorOf(treeLook, TreeImpostorMetres) }, LodBand));
    }

    internal void Update()
    {
        nearStreamer?.Update();
        farStreamer?.Update();
    }

    internal void Invalidate(double x, double z, double metres)
    {
        if (!Shown) return;
        foreach (VegetationField field in new[] { near!, far! })
            engine.Vegetation.Invalidate(new(field, x - metres, z - metres, x + metres, z + metres));
    }

    internal string Inspect()
    {
        if (!Shown) return "vegetation=off";
        static string Field(string name, VegetationReadout r, VegetationStreamer streamer) => FormattableString.Invariant(
            $"{name}: tiles={r.Tiles} pending={r.PendingTiles} patches={r.Patches} copies={r.Instances} overBudget={r.OverBudget} deferred={r.Deferred} placedLast={r.Placed}/{r.PlacedInstances} placeMs={r.PlacementMicroseconds / 1e3:F2} selectMs={r.SelectionMicroseconds / 1e3:F2} publishMs={r.PublicationMicroseconds / 1e3:F2} invalidated={r.InvalidatedTiles} replaced={r.ReplacedPatches} sampleMB={r.SampleBytes / 1e6:F2} committed={streamer.Committed} sampleMs={streamer.SampleMilliseconds:F0}");
        return FormattableString.Invariant($"grass={GrassDensity} reach={Reach} ")
            + Field("near", engine.Vegetation.Read(near!), nearStreamer!) + " | "
            + Field("far", engine.Vegetation.Read(far!), farStreamer!);
    }

    private void Hide()
    {
        nearStreamer?.Dispose();
        farStreamer?.Dispose();
        near?.Dispose();
        far?.Dispose();
        (near, far, nearStreamer, farStreamer) = (null, null, null, null);
    }

    public void Dispose()
    {
        Hide();
        grassLook.Dispose();
        treeLook.Dispose();
        treeMidLook.Dispose();
        grassMesh.Dispose();
        treeMesh.Dispose();
        treeMidMesh.Dispose();
        grassPaint.Dispose();
        treePaint.Dispose();
    }

    /// <summary>Three crossed cards, their tips weighted for the wind.</summary>
    private static MeshResourceCreateRequest Clump(Material material)
    {
        const int Cards = 3;
        const float Width = .5f, Height = .55f;
        List<Vector3> positions = [], normals = [];
        List<Color> colors = [];
        List<uint> indices = [];
        for (int card = 0; card < Cards; card++)
        {
            float angle = card * MathF.PI / Cards;
            Vector3 across = new(MathF.Cos(angle) * Width / 2, 0, MathF.Sin(angle) * Width / 2), up = new(0, Height, 0);
            Vector3 normal = new(-MathF.Sin(angle), 0, MathF.Cos(angle));
            uint first = (uint)positions.Count;
            positions.AddRange([-across, across, across * .3f + up, -across * .3f + up]);
            normals.AddRange([normal, normal, normal, normal]);
            colors.AddRange([new(1, 1, 1, 0), new(1, 1, 1, 0), new(1, 1, 1, 1), new(1, 1, 1, 1)]);
            indices.AddRange([first, first + 1, first + 2, first, first + 2, first + 3]);
        }

        return new MeshResourceCreateRequest(positions.ToArray(), normals.ToArray(), new Vector2[positions.Count], colors.ToArray(),
            indices.ToArray(), new MeshGroup[] { new(0, 0, (uint)indices.Count) }, new MeshMaterialBinding[] { new(0, material) });
    }

    /// <summary>A cone of <paramref name="sides"/> sides on a short trunk, 9 m tall.</summary>
    private static MeshResourceCreateRequest Tree(Material material, int sides)
    {
        int Sides = sides;
        const float Radius = 2.4f, Base = 1.5f, Top = 9f;
        List<Vector3> positions = [], normals = [];
        List<uint> indices = [];
        for (int side = 0; side < Sides; side++)
        {
            float a = side * MathF.Tau / Sides, b = (side + 1) * MathF.Tau / Sides;
            Vector3 p = new(MathF.Cos(a) * Radius, Base, MathF.Sin(a) * Radius), q = new(MathF.Cos(b) * Radius, Base, MathF.Sin(b) * Radius);
            Vector3 tip = new(0, Top, 0), normal = Vector3.Normalize(Vector3.Cross(tip - p, q - p) * -1);
            uint first = (uint)positions.Count;
            positions.AddRange([p, tip, q]);
            normals.AddRange([normal, normal, normal]);
            indices.AddRange([first, first + 1, first + 2]);
            // The trunk below the cone.
            Vector3 t0 = new(MathF.Cos(a) * .3f, 0, MathF.Sin(a) * .3f), t1 = new(MathF.Cos(b) * .3f, 0, MathF.Sin(b) * .3f);
            Vector3 side0 = Vector3.Normalize(new Vector3(MathF.Cos((a + b) / 2), 0, MathF.Sin((a + b) / 2)));
            first = (uint)positions.Count;
            positions.AddRange([t0, t1, t1 + new Vector3(0, Base, 0), t0 + new Vector3(0, Base, 0)]);
            normals.AddRange([side0, side0, side0, side0]);
            indices.AddRange([first, first + 2, first + 1, first, first + 3, first + 2]);
        }

        Color[] colors = new Color[positions.Count];
        Array.Fill(colors, new Color(1, 1, 1, 1));
        return new MeshResourceCreateRequest(positions.ToArray(), normals.ToArray(), new Vector2[positions.Count], colors,
            indices.ToArray(), new MeshGroup[] { new(0, 0, (uint)indices.Count) }, new MeshMaterialBinding[] { new(0, material) });
    }

    /// <summary>
    /// The hills as the heightfield draws them, grassland where it is low, gentle and dry; for the far
    /// field, a tree in some 14 m cells of the woods (broad patches of low-frequency noise).
    /// </summary>
    private sealed class Cover(bool trees) : IVegetationSampler
    {
        private const uint Bare = 1;
        private const double WoodsScale = 600, WoodsFrom = .55;

        public void Sample(VegetationTileRequest tile, VegetationTileSamples samples)
        {
            for (int j = -1; j <= (int)tile.TileCells + 1; j++)
            for (int i = -1; i <= (int)tile.TileCells + 1; i++)
                samples.SetHeight(i, j, (float)Math.Max(Ground.Lift1(tile.SampleX(i), tile.SampleZ(j)), Ground.SeaLevelMetres));

            for (int j = 0; j <= tile.TileCells; j++)
            for (int i = 0; i <= tile.TileCells; i++)
            {
                double x = tile.SampleX(i), z = tile.SampleZ(j);
                if (!Ground.Grassland(x, z)) samples.SetGround(i, j, Bare);
            }

            if (!trees) return;
            double size = tile.TileCells * tile.CellSize;
            long first = (long)Math.Floor(tile.OriginX / TreeCell), last = (long)Math.Floor((tile.OriginX + size) / TreeCell);
            long firstZ = (long)Math.Floor(tile.OriginZ / TreeCell), lastZ = (long)Math.Floor((tile.OriginZ + size) / TreeCell);
            for (long cz = firstZ; cz <= lastZ; cz++)
            for (long cx = first; cx <= last; cx++)
            {
                double x = (cx + Ground.Unit(cx, cz, 11)) * TreeCell, z = (cz + Ground.Unit(cx, cz, 12)) * TreeCell;
                double localX = x - tile.OriginX, localZ = z - tile.OriginZ;
                if (localX < 0 || localX >= size || localZ < 0 || localZ >= size) continue;
                if (Ground.Woods(x / WoodsScale, z / WoodsScale) < WoodsFrom || !Ground.Grassland(x, z)) continue;
                float y = (float)(Ground.Lift1(x, z) - TreeSink);
                samples.Records.Add(new VegetationRecord(TreeLayer, new((float)localX, y, (float)localZ),
                    (float)(Ground.Unit(cx, cz, 13) * Math.Tau), (float)(.7 + (.6 * Ground.Unit(cx, cz, 14)))));
            }
        }
    }
}
