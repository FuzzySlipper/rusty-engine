using System.Globalization;
using Rusty.Engine;

namespace CsharpCloakComparison;

/// <summary>
/// The settings each mode's cloak runs with, and the drag both feel. One
/// record, so a run reports exactly what it was tuned with and
/// <c>cloak.tune</c> can change one setting for the next run.
/// </summary>
internal sealed record CloakTuning
{
    /// <summary>A cape bone's plate: this share of its column's width.</summary>
    public float PlateShare { get; init; } = .36f;
    /// <summary>How far a cape bone swings from its parent and twists (radians).</summary>
    public float CapeSwing { get; init; } = .8f;
    public float CapeTwist { get; init; } = .4f;
    /// <summary>Damping of the twist (N·m·s per radian).</summary>
    public float CapeTwistDamping { get; init; } = .02f;
    /// <summary>The cape bones' angular damping: the cloth's resistance to swinging.</summary>
    public float CapeAngularDamping { get; init; } = 3;
    /// <summary>The soft cloak's contact thickness (m).</summary>
    public float SoftThickness { get; init; } = .02f;
    /// <summary>Substeps the soft cloak adds for itself and what it touches.</summary>
    public uint SoftExtraSubsteps { get; init; }
    /// <summary>The soft cloak's edge and bending stiffness (Hz).</summary>
    public float SoftEdge { get; init; } = 60;
    public float SoftBend { get; init; } = 8;
    /// <summary>Substeps every step takes while the world holds a soft body.</summary>
    public uint SoftSubsteps { get; init; } = 2;
    /// <summary>Drag of moving air on both cloaks (half the air's density times a drag coefficient): half a flag's, for heavy wool that lets some air through.</summary>
    public float Drag { get; init; } = DynamicsSoftBodyForcesRequest.FlagDrag / 2;

    public DynamicsSoftBodyMaterial SoftMaterial => DynamicsSoftBodyMaterial.Cloth with
    {
        Edge = DynamicsSoftness.Of(SoftEdge),
        Bend = DynamicsSoftness.Of(SoftBend),
    };

    /// <summary>This tuning with <paramref name="name"/> set to <paramref name="value"/>.</summary>
    public CloakTuning With(string name, string value)
    {
        if (name == "reset") return new CloakTuning();
        float number = float.Parse(value, CultureInfo.InvariantCulture);
        return name switch
        {
            "plate" => this with { PlateShare = number },
            "swing" => this with { CapeSwing = number },
            "twist" => this with { CapeTwist = number },
            "twistDamping" => this with { CapeTwistDamping = number },
            "angularDamping" => this with { CapeAngularDamping = number },
            "thickness" => this with { SoftThickness = number },
            "extraSubsteps" => this with { SoftExtraSubsteps = (uint)number },
            "edge" => this with { SoftEdge = number },
            "bend" => this with { SoftBend = number },
            "substeps" => this with { SoftSubsteps = (uint)number },
            "drag" => this with { Drag = number },
            _ => throw new ArgumentException("cloak.tune takes reset 0, plate, swing, twist, twistDamping, angularDamping, thickness, extraSubsteps, edge, bend, substeps or drag"),
        };
    }

    public override string ToString() => string.Create(CultureInfo.InvariantCulture,
        $"plate={PlateShare} swing={CapeSwing} twist={CapeTwist} twistDamping={CapeTwistDamping} angularDamping={CapeAngularDamping} " +
        $"thickness={SoftThickness} extraSubsteps={SoftExtraSubsteps} edge={SoftEdge} bend={SoftBend} substeps={SoftSubsteps} drag={Drag}");
}
