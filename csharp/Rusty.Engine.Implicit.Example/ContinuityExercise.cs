using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Implicit;

internal static class ContinuityExercise
{
    internal static void Run()
    {
        static void Require(bool condition, string message) { if (!condition) throw new Exception(message); }
        List<RecipeSurface> surfaces = [];
        var service = new RecordingImplicitSurfacesService();
        var writer = new RecipeWriter(service, new(.125f, 0, 1, ImplicitMaterialBoundaryMode.Centroid), surfaces.Add);
        var portal = new RecipeOpening("door", new(new(1, -.1f, -1), new(3, 2, .1f)));
        var room = RoomRecipes.Shell(writer, "room", new(new(new(0, 0, 0), new(4, 3, 6)), .5f, new[] { portal }), default);
        Require(surfaces.Select(s => s.Name).SequenceEqual(new[] { "room/floor", "room/walls", "room/ceiling" }), "Room surface identities drifted");
        Require(room.Joins.Length == 9, "Floor doorway must split the southern wall contact");
        Require(room.Joins.Span.ToArray().Where(j => j.Name.Contains("south-floor")).All(j => j.Center.X < 1 || j.Center.X > 3), "Door opening was declared solid");
        Require(room.Openings.Span[0] == portal, "Cut portal and enclosure declaration differ");
        Matrix4x4 placement = Matrix4x4.CreateScale(1, 1, -1) * Matrix4x4.CreateTranslation(10, 2, 8);
        var placed = room.Placed(placement);
        Require(placed.Openings.Span[0].Bounds == new ImplicitBounds(new(11, 1.9f, 7.9f), new(13, 4, 9)), "Reflected portal bounds incorrect");
        Require(placed.Joins.Span[0].Center == Vector3.Transform(room.Joins.Span[0].Center, placement), "Join lost room placement");
        var changed = RoomRecipes.Shell(writer, "larger", new(new(new(0, 0, 0), new(8, 4, 10)), .5f), default);
        Require(changed.Joins.Span.ToArray().Where(j => j.Name.Contains("ceiling")).All(j => j.Center.Y == 4), "Ceiling checks did not follow authored height");
        Require(changed.Interior == new Vector3(4, 2, 5), "Interior seed did not follow room dimensions");
        bool rejected = false;
        try { room.Placed(Matrix4x4.CreateRotationY(.3f)); } catch (ArgumentException) { rejected = true; }
        Require(rejected, "Rotated portal must not silently enlarge its enclosure cap");
        Transform roomPlacement = new(new Vector3(12, 1, -3), Quaternion.Identity, Vector3.One);
        int firstPlacedSurface = surfaces.Count;
        var jointlyPlaced = RoomRecipes.Shell(writer, "placed", new(new(new(0, 0, 0), new(4, 3, 6)), .5f), default, roomPlacement);
        Require(surfaces.Skip(firstPlacedSurface).All(surface => surface.Placement == roomPlacement), "Room geometry did not receive declaration placement");
        Require(jointlyPlaced.Interior == new Vector3(14, 2.5f, 0), "Room declarations did not receive geometry placement");
        Matrix4x4 shear = Matrix4x4.Identity; shear.M31 = .5f;
        rejected = false;
        try { room.Joins.Span[0].Placed(shear); } catch (ArgumentException) { rejected = true; }
        Require(rejected, "Sheared join cannot be represented by rectangular native contact axes");
        Console.WriteLine("Recipe continuity dimensions, portal exclusions and placement passed.");
    }
}
