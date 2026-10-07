using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct PresentationParticleDescriptor
{
    /// <summary>A descriptor whose billboards keep a fixed screen size (<see cref="PresentationParticleSizeMode.Screen"/>).</summary>
    public PresentationParticleDescriptor(
        ulong logicalId,
        string signalId,
        PresentationAnchor anchor,
        PresentationParticleVisual visual,
        RenderResourceReference sprite,
        ushort spriteFrameCount,
        float ratePerSecond,
        uint burstCount,
        float lifetimeMinSeconds,
        float lifetimeMaxSeconds,
        Vector3 velocityMin,
        Vector3 velocityMax,
        Vector3 acceleration,
        ReadOnlyMemory<PresentationParticleScalarKey> sizeCurve,
        ReadOnlyMemory<PresentationParticleColorKey> colorCurve,
        float flipbookFramesPerSecond,
        ulong seed,
        uint maxParticles,
        bool visible,
        bool hasCollision,
        PresentationParticleCollision collision,
        ReadOnlyMemory<PresentationParticleCollisionVolume> collisionVolumes)
        : this(
            logicalId,
            signalId,
            anchor,
            visual,
            sprite,
            spriteFrameCount,
            ratePerSecond,
            burstCount,
            lifetimeMinSeconds,
            lifetimeMaxSeconds,
            velocityMin,
            velocityMax,
            acceleration,
            sizeCurve,
            colorCurve,
            flipbookFramesPerSecond,
            seed,
            maxParticles,
            visible,
            hasCollision,
            collision,
            collisionVolumes,
            PresentationParticleSizeMode.Screen)
    {
    }

    /// <summary>A descriptor whose billboards blend by alpha (<see cref="PresentationParticleBlendMode.Alpha"/>) with a hard depth edge (<see cref="SoftnessMetres"/> 0).</summary>
    public PresentationParticleDescriptor(
        ulong logicalId,
        string signalId,
        PresentationAnchor anchor,
        PresentationParticleVisual visual,
        RenderResourceReference sprite,
        ushort spriteFrameCount,
        float ratePerSecond,
        uint burstCount,
        float lifetimeMinSeconds,
        float lifetimeMaxSeconds,
        Vector3 velocityMin,
        Vector3 velocityMax,
        Vector3 acceleration,
        ReadOnlyMemory<PresentationParticleScalarKey> sizeCurve,
        ReadOnlyMemory<PresentationParticleColorKey> colorCurve,
        float flipbookFramesPerSecond,
        ulong seed,
        uint maxParticles,
        bool visible,
        bool hasCollision,
        PresentationParticleCollision collision,
        ReadOnlyMemory<PresentationParticleCollisionVolume> collisionVolumes,
        PresentationParticleSizeMode sizeMode)
        : this(
            logicalId,
            signalId,
            anchor,
            visual,
            sprite,
            spriteFrameCount,
            ratePerSecond,
            burstCount,
            lifetimeMinSeconds,
            lifetimeMaxSeconds,
            velocityMin,
            velocityMax,
            acceleration,
            sizeCurve,
            colorCurve,
            flipbookFramesPerSecond,
            seed,
            maxParticles,
            visible,
            hasCollision,
            collision,
            collisionVolumes,
            sizeMode,
            PresentationParticleBlendMode.Alpha,
            0f)
    {
    }
}
