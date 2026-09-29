using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct WorldOriginCommitReceipt
{
    /// <summary>
    /// How far existing local-frame values move: the previous origin cell minus
    /// the committed one, in world units.
    /// </summary>
    public Vector3 LocalDelta => new(
        OriginBeforeCellX - OriginAfterCellX,
        OriginBeforeCellY - OriginAfterCellY,
        OriginBeforeCellZ - OriginAfterCellZ);
}

public readonly partial record struct CharacterMotion
{
    /// <summary>
    /// The same motion expressed in a rebased local frame. Moves the values the
    /// controller compares against later positions: the support anchor, the
    /// fixed tether anchor, and the fall origin and peak heights.
    /// </summary>
    public CharacterMotion Rebased(Vector3 localDelta) => this with
    {
        SupportPreviousTranslation = SupportPreviousTranslation + localDelta,
        TetherAnchorPoint = TetherAnchorPoint + localDelta,
        FallOriginY = FallOriginY + localDelta.Y,
        PeakY = PeakY + localDelta.Y,
    };
}
