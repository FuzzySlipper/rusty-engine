using System;
using System.Linq;
using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Implicit;

internal static class ImplicitAuditChecks
{
    internal static void Run(IEngineContext engine)
    {
        using Material material = engine.Graphics.CreateMaterial(new MaterialRequest(
            new Color(1, 1, 1, 1), default, 1, new Color(1, 1, 1, 1), default, 0, false));
        using ImplicitAudit audit = engine.ImplicitSurfaces.CreateAudit();
        Transform identity = new(Vector3.Zero, Quaternion.Identity, Vector3.One);
        // Loading Bay's column/capital ownership pattern: independently extracted
        // boxes share an exposed top while lower parts intentionally intersect.
        void Capture(ImplicitAudit target, ulong id, Vector3 minimum, Vector3 maximum)
        {
            using ImplicitRecipe recipe = new(engine.ImplicitSurfaces);
            ImplicitNode root = recipe.Box(minimum, maximum);
            using MeshResource mesh = engine.ImplicitSurfaces.Generate(new ImplicitGenerateRequest(
                recipe.Field, root, minimum - new Vector3(.5f), maximum + new Vector3(.5f), .25f,
                0, 1, material, ReadOnlyMemory<ImplicitMaterialRegion>.Empty));
            engine.ImplicitSurfaces.CaptureAuditPiece(new(target, id, recipe.Field, root, mesh, identity, engine.ImplicitSurfaces.ReadGeneration(recipe.Field).SampleSpacing));
        }
        Capture(audit, 101, new(-.55f, 0, -.55f), new(.55f, 5, .55f));
        Capture(audit, 102, new(-.7f, 4.5f, -.7f), new(.7f, 5, .7f));
        // Both mesh resources and field handles are disposed before observation.
        var report = engine.ImplicitSurfaces.ReadAudit(new(audit, .1f));
        if (!report.Diagnostics.Span.ToArray().Any(d => d.PieceA == 101 && d.PieceB == 102
            && d.Classification == ImplicitAuditClassification.CoincidentExposed && d.ApproximateArea > 0))
            throw new InvalidOperationException("Implicit audit missed independently owned exposed column tops.");
        var copied = report.Diagnostics.ToArray();
        audit.Dispose();
        if (copied.Length == 0) throw new InvalidOperationException("Audit copied report did not survive collection disposal.");
        using ImplicitAudit corrected = engine.ImplicitSurfaces.CreateAudit();
        Capture(corrected, 101, new(-.55f, 0, -.55f), new(.55f, 4.5f, .55f));
        Capture(corrected, 102, new(-.7f, 4.5f, -.7f), new(.7f, 5, .7f));
        if (engine.ImplicitSurfaces.ReadAudit(new(corrected, .1f)).Diagnostics.ToArray().Any(d =>
            d.Classification != ImplicitAuditClassification.BuriedSurface))
            throw new InvalidOperationException("Trimmed single-owner column still has an exposed conflict.");
        var integrity = engine.ImplicitSurfaces.ReadMeshIntegrity(new(corrected,
            ReadOnlyMemory<ImplicitAuditOpenRegion>.Empty));
        if (integrity.Complete == 0 || integrity.Sampled == 0)
            throw new InvalidOperationException("Mesh integrity did not inspect captured geometry.");
        var join = engine.ImplicitSurfaces.ReadExpectedJoin(new(corrected, 101, 102,
            new Vector3(0, 4.5f, 0), new Vector3(.4f, 0, 0), new Vector3(0, 0, .4f),
            .3f, .5f, .1f, 256));
        if (join.Complete == 0 || join.Diagnostics.Length != 0)
            throw new InvalidOperationException("Expected touching column/capital join was not satisfied.");
        var insufficient = engine.ImplicitSurfaces.ReadExpectedJoin(new(corrected, 101, 102,
            new Vector3(0, 4.5f, 0), new Vector3(.4f, 0, 0), new Vector3(0, 0, .4f),
            .3f, .5f, .1f, 1));
        if (insufficient.Complete != 0 || !insufficient.Diagnostics.Span.ToArray().Any(d =>
            d.Classification == ImplicitAnalysisClassification.IncompleteCoverage))
            throw new InvalidOperationException("Analysis budget exhaustion was reported as clean.");
        using ImplicitAudit room = engine.ImplicitSurfaces.CreateAudit();
        using (ImplicitRecipe recipe = new(engine.ImplicitSurfaces))
        {
            var root = recipe.Subtract(recipe.Box(new(-1.5f), new(1.5f)), recipe.Box(new(-1), new(1)));
            using MeshResource mesh = engine.ImplicitSurfaces.Generate(new ImplicitGenerateRequest(
                recipe.Field, root, new(-2), new(2), .2f, 0, 1, material,
                ReadOnlyMemory<ImplicitMaterialRegion>.Empty));
            engine.ImplicitSurfaces.CaptureAuditPiece(new(room, 201, recipe.Field, root, mesh, identity,
                engine.ImplicitSurfaces.ReadGeneration(recipe.Field).SampleSpacing));
        }
        var enclosure = engine.ImplicitSurfaces.ReadEnclosure(new(room, new(-2), new(2), Vector3.Zero,
            ReadOnlyMemory<ImplicitEnclosureOpening>.Empty, .2f, 10_000));
        if (enclosure.Complete == 0 || enclosure.Diagnostics.Span.ToArray().Any(d =>
            d.Classification == ImplicitAnalysisClassification.EnclosureLeak))
            throw new InvalidOperationException("Extracted closed room leaked in the enclosure audit.");
        var exterior = engine.ImplicitSurfaces.ReadEnclosure(new(room, new(-2), new(2), new(1.8f),
            ReadOnlyMemory<ImplicitEnclosureOpening>.Empty, .2f, 10_000));
        room.Dispose();
        if (exterior.Path.Length < 2 || !exterior.Diagnostics.Span.ToArray().Any(d =>
            d.Classification == ImplicitAnalysisClassification.EnclosureLeak))
            throw new InvalidOperationException("Enclosure witness was not copied before lease release.");
        using ImplicitAudit empty = engine.ImplicitSurfaces.CreateAudit();
        if (engine.ImplicitSurfaces.ReadAudit(new(empty, .1f)).Diagnostics.Length != 0)
            throw new InvalidOperationException("Audit collection retained another collection's pieces.");
    }
}
