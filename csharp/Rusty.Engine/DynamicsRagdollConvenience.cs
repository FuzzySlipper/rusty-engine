namespace Rusty.Engine;

public readonly partial record struct DynamicsJointLimits
{
    /// <summary>Turns about the frames' X axis between <paramref name="min"/> and <paramref name="max"/> radians.</summary>
    public static DynamicsJointLimits Hinge(float min, float max, float damping = 0) =>
        new(DynamicsJointKind.Hinge, min, max, 0, damping);

    /// <summary>Swings within <paramref name="swing"/> radians of the X axis and twists about it between <paramref name="twistMin"/> and <paramref name="twistMax"/>.</summary>
    public static DynamicsJointLimits Cone(float swing, float twistMin, float twistMax, float damping = 0) =>
        new(DynamicsJointKind.Cone, twistMin, twistMax, swing, damping);
}

public readonly partial record struct DynamicsRagdollBone
{
    /// <summary>The <see cref="EndJoint"/> of a bone measured by <see cref="Length"/> instead.</summary>
    public const uint NoEndJoint = uint.MaxValue;

    /// <summary>A capsule from <paramref name="joint"/> to where <paramref name="endJoint"/> is at rest.</summary>
    public static DynamicsRagdollBone Capsule(uint joint, uint endJoint, float radius, float mass) =>
        new(joint, endJoint, 0, DynamicsRagdollShape.Capsule, radius, 0, mass);

    /// <summary>A capsule <paramref name="length"/> metres along <paramref name="joint"/>'s +Y, for a bone with no end joint (a hand, a head).</summary>
    public static DynamicsRagdollBone CapsuleAlong(uint joint, float length, float radius, float mass) =>
        new(joint, NoEndJoint, length, DynamicsRagdollShape.Capsule, radius, 0, mass);

    /// <summary>A box from <paramref name="joint"/> to where <paramref name="endJoint"/> is at rest, <paramref name="halfWidth"/> along the joint's X and <paramref name="halfDepth"/> along its Z.</summary>
    public static DynamicsRagdollBone Box(uint joint, uint endJoint, float halfWidth, float halfDepth, float mass) =>
        new(joint, endJoint, 0, DynamicsRagdollShape.Box, halfWidth, halfDepth, mass);
}
