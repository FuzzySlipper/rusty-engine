using System.Numerics;
using Rusty.Engine;

namespace CsharpWaterField;

/// <summary>
/// The product's water owner: it decides the region, its edges and its forcing (the river's
/// discharge, the hollow's soak), and answers what the rest of the product asks of water from
/// the Engine field's latest snapshot. Nothing else in the product reads the field.
/// </summary>
internal sealed class ValleyWater : IDisposable
{
    public const float BaseFlow = 6, StormFlow = 90;
    /// <summary>How fast standing water soaks into the hollow's floor, metres a second.</summary>
    private const float HollowSoak = -2e-5f;
    /// <summary>Half the swim box's width about the swimmer; it only has to contain them.</summary>
    private const float SwimBoxHalfWidth = 1;

    private readonly IWaterFieldService water;
    private readonly WaterField field;

    public ValleyWater(IWaterFieldService water, Valley valley)
    {
        this.water = water;
        field = water.Create(new WaterFieldCreateRequest(Valley.Width, Valley.Length, Valley.Cell, valley.Bed, Vector3.Zero));
        water.SetBoundary(WaterFieldBoundaryRequest.Open(field, WaterFieldSide.XMax));
        Discharge = BaseFlow;
        water.SetBoundary(WaterFieldBoundaryRequest.Inflow(field, WaterFieldSide.XMin, Discharge, Valley.ChannelFirst, Valley.ChannelCells));
        var soak = new List<WaterFieldCellValue>();
        for (uint z = 0; z < Valley.Length; z++)
        for (uint x = 0; x < Valley.Width; x++)
            if (Valley.InHollow(x, z)) soak.Add(new(x, z, HollowSoak));
        water.SetSources(new WaterFieldSourcesRequest(field, 0, soak.ToArray()));
    }

    public float Discharge { get; private set; }

    public void SetDischarge(float discharge)
    {
        Discharge = discharge;
        water.SetBoundary(WaterFieldBoundaryRequest.Inflow(field, WaterFieldSide.XMin, discharge, Valley.ChannelFirst, Valley.ChannelCells));
    }

    public WaterFieldReadout EditBed(WaterFieldCellValue[] cells) => water.EditBed(new WaterFieldBedRequest(field, cells));

    public WaterFieldReadout Present(Material material) => water.Present(field, material);

    public WaterFieldReadout Read() => water.Read(field);

    public WaterFieldSample Sample(Vector3 point) => water.Sample(field, point);

    public ReadOnlyMemory<WaterFieldCell> Cells() => water.ReadCells(field);

    /// <summary>
    /// The swim box for a body at <paramref name="position"/>: from the ground to the water's
    /// surface around it, or none on dry ground.
    /// </summary>
    public bool TrySwimBox(Vector3 position, out Vector3 minimum, out Vector3 maximum, out WaterFieldSample sample)
    {
        sample = Sample(position);
        minimum = new(position.X - SwimBoxHalfWidth, sample.Bed, position.Z - SwimBoxHalfWidth);
        maximum = new(position.X + SwimBoxHalfWidth, sample.Surface, position.Z + SwimBoxHalfWidth);
        return sample.Wet;
    }

    public void Dispose() => field.Dispose();
}
