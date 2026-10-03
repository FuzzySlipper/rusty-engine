namespace Rusty.Engine;

public readonly partial record struct SpatialContentArtifactInstance
{
    /// <summary>An unrotated placement.</summary>
    public SpatialContentArtifactInstance(ulong id, ContentReference content, long columnOffset, long levelOffset, long rowOffset)
        : this(id, content, columnOffset, levelOffset, rowOffset, 0)
    {
    }
}
