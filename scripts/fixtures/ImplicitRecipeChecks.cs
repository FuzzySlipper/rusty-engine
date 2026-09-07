using System;
using System.Linq;
using System.Numerics;
using Rusty.Engine.Implicit;

namespace SdkPackageConsumer;

internal static class ImplicitRecipeChecks
{
    internal static void Run()
    {
        static void Require(bool condition, string message)
        {
            if (!condition) throw new InvalidOperationException(message);
        }

        // Sectioning must not restart the brick phase. Compare occupied intervals in
        // a long wall with the same wall emitted as independently meshed bays.
        foreach (float length in new[] { 20f, 24f, 26f })
        {
            var whole = MasonryLayout.Courses(0, length, 4.8f, 1.05f, 0.55f, 0.045f).ToArray();
            var bays = Enumerable.Range(0, (int)MathF.Ceiling(length / 4f))
                .SelectMany(i => MasonryLayout.Courses(i * 4f, MathF.Min(length, (i + 1) * 4f), 4.8f, 1.05f, 0.55f, 0.045f)).ToArray();
            for (float x = 0.013f; x < length; x += 0.037f)
            for (float y = 0.017f; y < 4.8f; y += 0.091f)
            {
                bool Occupied(MasonryCourse c) => x >= c.Left && x <= c.Right && y >= c.Bottom && y <= c.Top;
                Require(whole.Any(Occupied) == bays.Any(Occupied), $"bay changed brick phase at {x},{y}");
            }
        }

        foreach (float yaw in new[] { 0f, 45f, 90f, 180f, -90f })
        foreach (float width in new[] { 2.4f, 3.4f, 4.2f })
        {
            WallOpening opening = new(12.25f, width, 4f, true);
            WallLayout wall = new(new(-12, 3, 10), yaw, 24, 6.7f, 0.6f, opening);
            Vector3 left = wall.ToWorld(new(opening.Left, opening.SpringHeight, 0));
            Vector3 right = wall.ToWorld(new(opening.Right, opening.SpringHeight, 0));
            Vector3 center = wall.ToWorld(new(opening.Center, opening.SpringHeight, 0));
            Require(MathF.Abs(Vector3.Distance(left, right) - width) < 0.00001f, "rotation changed opening width");
            Require(Vector3.Distance((left + right) * 0.5f, center) < 0.00001f, "opening lost its shared center");
            Require(opening.Top < wall.Height, "opening escapes coping");
            Require(Vector3.Distance(wall.ToWorld(Vector3.Zero), wall.Origin) < 0.00001f, "frame origin drift");
        }
    }
}
