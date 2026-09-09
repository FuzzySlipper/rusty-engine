using System.Numerics;

namespace Rusty.Engine.Input;

/// <summary>Physical controls selected by an FPS product. These are product policy, not runtime input mappings.</summary>
public readonly record struct FpsInputBindings(
    KeyboardControl ForwardKey,
    KeyboardControl BackwardKey,
    KeyboardControl RightKey,
    KeyboardControl LeftKey,
    KeyboardControl JumpKey,
    KeyboardControl CrouchKey,
    KeyboardControl SprintKey,
    KeyboardControl UseKey,
    ControllerButton JumpButton,
    ControllerButton CrouchButton,
    ControllerButton SprintButton,
    ControllerButton UseButton,
    ControllerAxis MoveHorizontalAxis,
    ControllerAxis MoveVerticalAxis,
    ControllerAxis LookHorizontalAxis,
    ControllerAxis LookVerticalAxis,
    bool InvertMoveVerticalAxis)
{
    /// <summary>WASD and common standard-gamepad layout (left stick move, right stick look).</summary>
    public static FpsInputBindings Standard => new(
        KeyboardControl.KeyW, KeyboardControl.KeyS, KeyboardControl.KeyD, KeyboardControl.KeyA,
        KeyboardControl.Space, KeyboardControl.ControlLeft, KeyboardControl.ShiftLeft, KeyboardControl.KeyE,
        ControllerButton.Button0, ControllerButton.Button1, ControllerButton.Button10, ControllerButton.Button2,
        ControllerAxis.Axis0, ControllerAxis.Axis1, ControllerAxis.Axis2, ControllerAxis.Axis3,
        InvertMoveVerticalAxis: true);
}

/// <summary>FPS input tuning. Pointer sensitivity is radians per pointer unit; controller sensitivity is radians per second at full stick deflection.</summary>
public sealed record FpsInputConfig(
    FpsInputBindings Bindings,
    RadialDeadzone MoveDeadzone,
    RadialDeadzone LookDeadzone,
    LookConfig PointerLookConfig,
    float ControllerHorizontalRadiansPerSecond,
    float ControllerVerticalRadiansPerSecond,
    bool InvertControllerHorizontal,
    bool InvertControllerVertical)
{
    /// <summary>Conservative defaults with radians explicitly named; products should retain an explicit selected configuration.</summary>
    public static FpsInputConfig Standard { get; } = new(
        FpsInputBindings.Standard,
        RadialDeadzone.Standard,
        RadialDeadzone.Standard,
        new LookConfig(0.002f, 0.002f, -MathF.PI / 2f + 0.0001f, MathF.PI / 2f - 0.0001f, MathF.PI, false, true, true),
        ControllerHorizontalRadiansPerSecond: MathF.PI * 0.6f,
        ControllerVerticalRadiansPerSecond: MathF.PI * 0.6f,
        InvertControllerHorizontal: false,
        InvertControllerVertical: true);
}

/// <summary>One FPS product input readout. PointerDelta remains in pointer units; ControllerLookRadians is already time-integrated radians.</summary>
public readonly record struct FpsInputFrame(
    Vector2 Movement,
    bool JumpHeld,
    bool JumpPressed,
    bool CrouchHeld,
    bool SprintHeld,
    bool UseHeld,
    bool UsePressed,
    Vector2 PointerDelta,
    Vector2 ControllerLookRadians);

/// <summary>
/// Product-side composition of persistent physical input, controller shaping, and existing <see cref="Look"/> math.
/// It does not declare or modify runtime bindings.
/// </summary>
public sealed class FpsInput
{
    public FpsInput(FpsInputConfig? config = null, PhysicalInputState? physical = null)
    {
        Config = config ?? FpsInputConfig.Standard;
        Physical = physical ?? new PhysicalInputState();
        ValidateConfig(Config);
    }

    public FpsInputConfig Config { get; }
    public PhysicalInputState Physical { get; }

    /// <summary>Consumes raw physical facts then reads the selected FPS controls for this simulation step.</summary>
    public FpsInputFrame Consume(ReadOnlySpan<ProductInputEvent> events, float simulationDeltaSeconds)
    {
        Physical.Consume(events);
        return Read(simulationDeltaSeconds);
    }

    /// <summary>Reads retained controls. Controller look is integrated by simulation seconds; pointer input is not.</summary>
    public FpsInputFrame Read(float simulationDeltaSeconds)
    {
        if (!float.IsFinite(simulationDeltaSeconds) || simulationDeltaSeconds < 0f)
        {
            throw new ArgumentOutOfRangeException(nameof(simulationDeltaSeconds), "Simulation delta must be finite and non-negative seconds.");
        }

        FpsInputBindings bindings = Config.Bindings;
        Vector2 keyboardMovement = new(
            Axis(Physical.Held(bindings.RightKey), Physical.Held(bindings.LeftKey)),
            Axis(Physical.Held(bindings.ForwardKey), Physical.Held(bindings.BackwardKey)));
        Vector2 controllerMovement = new(
            Physical.Axis(bindings.MoveHorizontalAxis),
            Physical.Axis(bindings.MoveVerticalAxis) * (bindings.InvertMoveVerticalAxis ? -1f : 1f));
        Vector2 controllerLook = AnalogInput.ApplyRadialDeadzone(new(
            Physical.Axis(bindings.LookHorizontalAxis), Physical.Axis(bindings.LookVerticalAxis)), Config.LookDeadzone);
        Vector2 controllerLookRadians = new(
            controllerLook.X * Config.ControllerHorizontalRadiansPerSecond * simulationDeltaSeconds,
            controllerLook.Y * Config.ControllerVerticalRadiansPerSecond * simulationDeltaSeconds);

        return new FpsInputFrame(
            AnalogInput.CombineMovement(keyboardMovement, AnalogInput.ApplyRadialDeadzone(controllerMovement, Config.MoveDeadzone)),
            Physical.Held(bindings.JumpKey) || Physical.Held(bindings.JumpButton),
            Physical.Pressed(bindings.JumpKey) || Physical.Pressed(bindings.JumpButton),
            Physical.Held(bindings.CrouchKey) || Physical.Held(bindings.CrouchButton),
            Physical.Held(bindings.SprintKey) || Physical.Held(bindings.SprintButton),
            Physical.Held(bindings.UseKey) || Physical.Held(bindings.UseButton),
            Physical.Pressed(bindings.UseKey) || Physical.Pressed(bindings.UseButton),
            Physical.PointerDelta,
            controllerLookRadians);
    }

    /// <summary>
    /// Applies pointer and controller look separately through <see cref="Look.IntegrateClamped"/>.
    /// Pointer sensitivity remains radians per pointer unit; right-stick values remain radians per second before frame integration.
    /// </summary>
    public LookReceipt IntegrateLook(LookState state, FpsInputFrame frame)
    {
        LookReceipt pointer = Look.IntegrateClamped(new LookRequest(state, frame.PointerDelta, Config.PointerLookConfig));
        LookConfig controllerLookConfig = new(
            1f, 1f,
            Config.PointerLookConfig.MinimumPitchRadians,
            Config.PointerLookConfig.MaximumPitchRadians,
            Config.PointerLookConfig.MaximumDeltaRadians,
            Config.InvertControllerHorizontal,
            Config.InvertControllerVertical,
            Config.PointerLookConfig.WrapYaw);
        return Look.IntegrateClamped(new LookRequest(pointer.After, frame.ControllerLookRadians, controllerLookConfig)) with { Before = state };
    }

    private static float Axis(bool positive, bool negative) => positive == negative ? 0f : positive ? 1f : -1f;

    private static void ValidateConfig(FpsInputConfig config)
    {
        _ = AnalogInput.ApplyRadialDeadzone(Vector2.Zero, config.MoveDeadzone);
        _ = AnalogInput.ApplyRadialDeadzone(Vector2.Zero, config.LookDeadzone);
        if (Look.Diagnose(new LookRequest(default, Vector2.Zero, config.PointerLookConfig)) != LookDiagnostic.Accepted)
        {
            throw new ArgumentOutOfRangeException(nameof(config), "Pointer look configuration must be accepted by Look.");
        }
        if (!float.IsFinite(config.ControllerHorizontalRadiansPerSecond)
            || !float.IsFinite(config.ControllerVerticalRadiansPerSecond)
            || config.ControllerHorizontalRadiansPerSecond < 0f
            || config.ControllerVerticalRadiansPerSecond < 0f)
        {
            throw new ArgumentOutOfRangeException(nameof(config), "Controller look sensitivity must be finite radians per second and non-negative.");
        }
    }
}
