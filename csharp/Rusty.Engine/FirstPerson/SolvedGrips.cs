using System.Numerics;
using System.Text.Json.Serialization;

namespace Rusty.Engine.FirstPerson;

/// <summary>
/// A grip solved by contact around a held look (asset-pipeline's <c>grip.py</c> writes one): the right hand's digits on
/// the look (<see cref="Shape"/>, over the arms' canonical holding pose as a look's shape is) and where the look sits in
/// the hand (<see cref="Object"/>). A hand holding by it is turned and placed so the look sits in it where the solve put
/// it, in place of a hold turn and the rig's palm offset; the rig's <see cref="ArmRig.Grip"/> still brings its clip to
/// the holding pose under the shape.
/// </summary>
public sealed record SolvedGrip(HandShape Shape, SolvedObject Object)
{
    /// <summary>The hand's canonical frame turned from the look's, as a hold is.</summary>
    [JsonIgnore] public Quaternion Hold => Quaternion.Inverse(Object.InCanonical.Turn);

    /// <summary>Where the look's grip point sits from the hand joint, in the hand's canonical frame (metres), as a rig's palm offset is.</summary>
    [JsonIgnore] public Vector3 Palm => Object.InCanonical.Position;

    /// <summary>What is wrong with this grip, if anything.</summary>
    public IEnumerable<HeldProblem> Problems() =>
        Shape.Problems().Select(p => p.Under("shape")).Concat(Object.InCanonical.Problems().Select(p => p.Under("object.in_canonical")));
}

/// <summary>Where a solved grip's look sits: in the hand's canonical frame (<see cref="HandFrame"/>), from the hand joint.</summary>
public sealed record SolvedObject([property: JsonPropertyName("in_canonical")] SolvedPlacement InCanonical);

/// <summary>
/// A look's placement in the hand's canonical frame: where its grip point (the point the solve held it at) sits from
/// the hand joint (metres), and its turn as a quaternion <c>[w, x, y, z]</c>, which takes the look's own axes (its
/// length along -Z, its up side +Y) into the hand's.
/// </summary>
public sealed record SolvedPlacement(Vector3 Position, float[] Rotation)
{
    /// <summary>The placement's turn.</summary>
    [JsonIgnore] public Quaternion Turn => Rotation.Length == 4 ? Quaternion.Normalize(new(Rotation[1], Rotation[2], Rotation[3], Rotation[0])) : Quaternion.Identity;

    /// <summary>What is wrong with this placement, if anything.</summary>
    public IEnumerable<HeldProblem> Problems()
    {
        if (Rotation.Length != 4 || new Vector4(Rotation[0], Rotation[1], Rotation[2], Rotation[3]).Length() < 0.5f)
            yield return new("rotation", "gives a unit quaternion as [w, x, y, z].");
    }
}
