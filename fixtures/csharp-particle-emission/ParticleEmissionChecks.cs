using System;
using System.Numerics;
using Rusty.Engine;

namespace SdkPackageConsumer;

internal static class ParticleEmissionChecks
{
    internal static void Run(IEngineContext engine)
    {
        PresentationParticleDescriptor minimal = new()
        {
            SignalId = "fixture.particle.minimal",
            Visible = true,
            Anchor = new() { Kind = PresentationAnchorKind.World, Position = new Vector3(2, 3, 4) },
            BurstCount = 8,
        };
        ExpectRefusal(engine, minimal);
        ExpectRefusal(engine, minimal with { Visual = PresentationParticleVisual.Cube });
        PresentationParticleDescriptor valid = minimal with
        {
            Visual = PresentationParticleVisual.Cube,
            MaxParticles = 8,
            LifetimeMinSeconds = 0.5f,
            LifetimeMaxSeconds = 1,
            SizeCurve = new PresentationParticleScalarKey[] { new(0, 0.2f), new(1, 0.1f) },
            ColorCurve = new PresentationParticleColorKey[] { new(0, new Color(1, 1, 1, 1)), new(1, new Color(1, 1, 1, 0)) },
        };
        Require(engine.Presentation.EmitParticles(valid).Outcome == PresentationParticleEmissionOutcome.Admitted,
            "valid cube burst was not admitted after refusal");
        ExpectRefusal(engine, valid with { Anchor = default });
        ExpectRefusal(engine, valid with { Visual = (PresentationParticleVisual)999 });
        ExpectRefusal(engine, valid with { HasCollision = true });
        ExpectRefusal(engine, valid with { SignalId = "fixture.particle.bad-seed", Seed = ulong.MaxValue });
        Require(engine.Presentation.EmitParticles(valid).Outcome == PresentationParticleEmissionOutcome.Admitted,
            "a repeated signal label was not admitted as a distinct burst");
        ExpectRefusal(engine, valid with { SignalId = "fixture.particle.no-color", ColorCurve = default });
        RenderResourceInfo texture = engine.Graphics.OpenResource(new RenderResourceRequest("spatial-particle.png"));
        PresentationParticleDescriptor billboard = valid with
        {
            SignalId = "fixture.particle.billboard",
            Visual = PresentationParticleVisual.Billboard,
            Sprite = texture.Handle,
        };
        ExpectRefusal(engine, billboard); // A nonanimated sprite still needs frame count one.
        Require(engine.Presentation.EmitParticles(billboard with { SpriteFrameCount = 1 }).Outcome == PresentationParticleEmissionOutcome.Admitted,
            "valid billboard burst was not admitted after missing-frame refusal");
        Require(engine.Presentation.EmitParticles(billboard with
        {
            SignalId = "fixture.particle.world-billboard",
            SpriteFrameCount = 1,
            SizeMode = PresentationParticleSizeMode.World,
        }).Outcome == PresentationParticleEmissionOutcome.Admitted, "world-size billboard burst was not admitted");
        Require(engine.Presentation.EmitParticles(valid with { SignalId = "fixture.particle.after-errors" }).Outcome == PresentationParticleEmissionOutcome.Admitted,
            "emission did not recover after caught descriptor failures");
        Require(engine.Presentation.EmitParticles(valid with
        {
            SignalId = "fixture.particle.debris",
            HasCollision = true,
            Collision = new(0.1f, 0.2f, 0.5f, 4, 0.01f, PresentationParticleCollisionLimitBehavior.Sleep),
            CollisionVolumes = new PresentationParticleCollisionVolume[]
            {
                new(PresentationParticleCollisionVolumeKind.Aabb, default, 0, new Vector3(-3), new Vector3(3)),
            },
        }).Outcome == PresentationParticleEmissionOutcome.Admitted, "colliding cube debris was not admitted");
        Console.WriteLine("PARTICLE_EMISSION_CHECKS_PASSED");
    }

    private static void ExpectRefusal(IEngineContext engine, PresentationParticleDescriptor descriptor)
    {
        try { engine.Presentation.EmitParticles(descriptor); }
        catch (EngineCallException error)
        {
            Require(error.Service == "Presentation" && error.Operation == "EmitParticles"
                && error.Diagnostics.Length > 0 && !string.IsNullOrWhiteSpace(error.Diagnostics.Span[0].Message),
                "particle refusal lost its named operation diagnostic");
            return;
        }
        throw new InvalidOperationException("invalid particle descriptor was accepted");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
