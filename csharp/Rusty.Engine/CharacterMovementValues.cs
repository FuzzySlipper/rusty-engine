using System.Numerics;

namespace Rusty.Engine;

public readonly partial record struct CharacterControllerCommand
{
    /// <summary>Creates an ordinary walking/airborne command.</summary>
    public CharacterControllerCommand(Vector2 PlanarIntent, float HeadingYawRadians, bool JumpPressed, bool JumpHeld, bool CrouchRequested, Vector3 ExternalVelocity, Vector3 ExternalImpulse, float StepSeconds, ulong Sequence)
        : this(default, PlanarIntent, HeadingYawRadians, JumpPressed, JumpHeld, CrouchRequested, ExternalVelocity, ExternalImpulse, StepSeconds, Sequence) { }
}
