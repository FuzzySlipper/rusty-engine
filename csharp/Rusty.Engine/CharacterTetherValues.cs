using System;
using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct CharacterMotion
{
    /// <summary>Creates the ordinary untethered value.</summary>
    public CharacterMotion(Vector3 ControlledVelocity, Vector3 ExternalVelocity, bool Grounded, CharacterStance Stance, float JumpBufferRemaining, float CoyoteRemaining, float LandingLockoutRemaining, bool SupportEntityPresent, ulong SupportEntity, Vector3 SupportLocalAnchor, Vector3 SupportPreviousTranslation, Quaternion SupportPreviousRotation, Vector3 SupportPointVelocity, float FallOriginY, float PeakY, ulong LastCommandSequence, ulong CollisionWorldHash)
        : this(false, 0, 0, false, 0, Vector3.Zero, Vector3.Zero, ControlledVelocity, ExternalVelocity, Grounded, Stance, JumpBufferRemaining, CoyoteRemaining, LandingLockoutRemaining, SupportEntityPresent, SupportEntity, SupportLocalAnchor, SupportPreviousTranslation, SupportPreviousRotation, SupportPointVelocity, FallOriginY, PeakY, LastCommandSequence, CollisionWorldHash)
    {
    }
}

public readonly partial record struct CharacterStepRequest
{
    /// <summary>Creates the ordinary untethered value.</summary>
    public CharacterStepRequest(SpatialSession Session, Vector3 Position, CharacterMotion Motion, CharacterSupport Support, ReadOnlyMemory<CharacterObstacle> Obstacles, ReadOnlyMemory<CharacterMeshInstance> MeshInstances, CharacterControllerConfig Config, CharacterControllerCommand Command)
        : this(default, Session, Position, Motion, Support, Obstacles, MeshInstances, Config, Command)
    {
    }
}

public readonly partial record struct CharacterStepReceipt
{
    /// <summary>Creates the ordinary untethered value.</summary>
    public CharacterStepReceipt(ulong Generation, ulong RevisionBefore, ulong RevisionAfter, ulong Entity, ulong CommandSequence, Transform TransformBefore, Transform Transform, CharacterMotion Motion, Vector3 WishVelocity, Vector3 Displacement, CharacterContact Contact, CharacterGround Ground, CharacterFloorProbe FloorProbe, CharacterStanceFact Stance, CharacterStep Step, CharacterPlatform Platform, CharacterBlockFlags BlockFlags, uint ContactCount, uint DynamicImpulseCount, uint CastCount, uint RecoveryPasses, float RecoveryDistance)
        : this(default, Generation, RevisionBefore, RevisionAfter, Entity, CommandSequence, TransformBefore, Transform, Motion, WishVelocity, Displacement, Contact, Ground, FloorProbe, Stance, Step, Platform, BlockFlags, ContactCount, DynamicImpulseCount, CastCount, RecoveryPasses, RecoveryDistance)
    {
    }
}

public readonly partial record struct CharacterTetherRequest
{
    public static CharacterTetherRequest AtFixedAnchor(ulong id, Vector3 anchor, float maximumLength, Vector3 localAnchor = default)
        => new(true, id, localAnchor, false, anchor, default, maximumLength, maximumLength, 0);

    public static CharacterTetherRequest AtDynamicAnchor(ulong id, DynamicsAnchorObservation anchor, float maximumLength, Vector3 localAnchor = default)
        => new(true, id, localAnchor, true, default, anchor, maximumLength, maximumLength, 0);
}
