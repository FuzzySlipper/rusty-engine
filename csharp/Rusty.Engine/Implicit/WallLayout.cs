using System.Numerics;

namespace Rusty.Engine.Implicit;

/// <summary>
/// Product-authored local wall coordinates: X follows the wall, Y is up, and
/// the decorated face points toward negative Z.
/// </summary>
public readonly record struct WallLayout(
    Vector3 Origin,
    float YawDegrees,
    float Length,
    float Height,
    float Thickness,
    WallOpening? Opening = null)
{
    public Quaternion Rotation => Quaternion.CreateFromAxisAngle(
        Vector3.UnitY,
        YawDegrees * MathF.PI / 180f);

    public Transform Placement => new(Origin, Rotation, Vector3.One);

    public Vector3 ToWorld(Vector3 local) => Origin + Vector3.Transform(local, Rotation);
}

/// <summary>One rectangular or round-topped opening in a wall's local frame.</summary>
public readonly record struct WallOpening(float Center, float Width, float SpringHeight, bool Arched)
{
    public float Left => Center - Width * 0.5f;
    public float Right => Center + Width * 0.5f;
    public float Top => SpringHeight + (Arched ? Width * 0.5f : 0f);
}

/// <summary>One occupied local-space masonry rectangle.</summary>
public readonly record struct MasonryCourse(float Bottom, float Top, float Left, float Right);

/// <summary>Dimensions and joint clearance for a globally phased masonry pattern.</summary>
public readonly record struct MasonryOptions(float UnitLength, float CourseHeight, float Joint)
{
    internal void Validate()
    {
        if (!float.IsFinite(UnitLength) || UnitLength <= 0f)
        {
            throw new ArgumentOutOfRangeException(nameof(UnitLength), "Masonry unit length must be finite and positive.");
        }
        if (!float.IsFinite(CourseHeight) || CourseHeight <= 0f)
        {
            throw new ArgumentOutOfRangeException(nameof(CourseHeight), "Masonry course height must be finite and positive.");
        }
        if (!float.IsFinite(Joint) || Joint < 0f)
        {
            throw new ArgumentOutOfRangeException(nameof(Joint), "Masonry joint must be finite and non-negative.");
        }
    }
}

/// <summary>Pure, globally phased masonry layout helpers.</summary>
public static class MasonryLayout
{
    public static IEnumerable<MasonryCourse> Courses(
        float start,
        float end,
        float height,
        MasonryOptions options)
    {
        options.Validate();
        for (int row = 0; row * options.CourseHeight < height; row++)
        {
            float phase = (row % 2) * options.UnitLength * 0.5f;
            for (float x = MathF.Floor((start - phase) / options.UnitLength) * options.UnitLength + phase;
                 x < end;
                 x += options.UnitLength)
            {
                float left = MathF.Max(start, x + options.Joint);
                float right = MathF.Min(end, x + options.UnitLength - options.Joint);
                float bottom = row * options.CourseHeight + options.Joint;
                float top = MathF.Min(height, (row + 1) * options.CourseHeight - options.Joint);
                if (right > left && top > bottom)
                {
                    yield return new MasonryCourse(bottom, top, left, right);
                }
            }
        }
    }

    /// <summary>Convenient parameter form retained for direct migration.</summary>
    public static IEnumerable<MasonryCourse> Courses(
        float start,
        float end,
        float height,
        float unitLength,
        float courseHeight,
        float joint) => Courses(start, end, height, new MasonryOptions(unitLength, courseHeight, joint));
}
