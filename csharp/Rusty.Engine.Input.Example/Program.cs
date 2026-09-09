using System.Numerics;
using Rusty.Engine;
using Rusty.Engine.Input;

ExercisePhysicalState();
ExerciseAnalogShaping();
ExerciseFpsComposition();

static void ExercisePhysicalState()
{
    var input = new PhysicalInputState();
    input.Consume([
        Event(InputEventKind.Key, InputEdge.Pressed, key: KeyboardControl.KeyW),
        Event(InputEventKind.Key, InputEdge.Released, key: KeyboardControl.KeyW),
        Event(InputEventKind.PointerDelta, x: 2f, y: -3f),
        Event(InputEventKind.PointerDelta, x: 1f, y: 4f),
        Event(InputEventKind.ControllerButtonValue, button: ControllerButton.Button7, x: 0.7f),
        Event(InputEventKind.MappedDigital, InputEdge.Pressed, key: KeyboardControl.KeyA),
    ]);
    Require(input.Pressed(KeyboardControl.KeyW) && input.Released(KeyboardControl.KeyW) && !input.Held(KeyboardControl.KeyW),
        "press and release in one batch did not preserve both edges");
    Require(input.PointerDelta == new Vector2(3f, 1f), "pointer samples were not summed");
    Require(input.ButtonValue(ControllerButton.Button7) == 0.7f, "analog trigger value was not retained");
    Require(!input.Held(KeyboardControl.KeyA), "mapped duplicate entered physical state");

    input.Consume([
        Event(InputEventKind.Key, InputEdge.Pressed, key: KeyboardControl.KeyW),
        Event(InputEventKind.Key, InputEdge.Pressed, key: KeyboardControl.KeyS),
        Event(InputEventKind.ControllerButton, InputEdge.Pressed, button: ControllerButton.Button0),
    ]);
    input.Consume([
        Event(InputEventKind.Key, InputEdge.Released, key: KeyboardControl.KeyS),
        Event(InputEventKind.Key, InputEdge.Pressed, key: KeyboardControl.KeyA),
        Event(InputEventKind.PointerDelta, x: 9f),
        Event(InputEventKind.Clear),
    ]);
    Require(input.Released(KeyboardControl.KeyW) && input.Released(KeyboardControl.KeyS)
            && input.Released(ControllerButton.Button0) && !input.Pressed(KeyboardControl.KeyA)
            && !input.Held(KeyboardControl.KeyW) && input.PointerDelta == Vector2.Zero,
        "clear did not retain releases for previously held controls while discarding pending input");
    input.Consume([]);
    Require(!input.Released(KeyboardControl.KeyW) && !input.Released(ControllerButton.Button0),
        "the next consume did not reset lifecycle release edges");
}

static void ExerciseAnalogShaping()
{
    Vector2 remapped = AnalogInput.ApplyRadialDeadzone(new Vector2(0.575f, 0f), new RadialDeadzone(0.15f, 1f, 1f));
    RequireClose(remapped.X, 0.5f, "radial dead zone did not remap its inner/outer range");
    Require(AnalogInput.ApplyRadialDeadzone(new Vector2(0.1f, 0.1f), RadialDeadzone.Standard) == Vector2.Zero,
        "radial dead zone admitted a small diagonal");
    RequireClose(AnalogInput.ApplyTriggerCurve(0.5f, new TriggerCurve(0.2f, 1f, 2f)), 0.140625f,
        "trigger curve did not apply its exponent");
    RequireClose(AnalogInput.CombineMovement(Vector2.Zero, new Vector2(0.3f, 0.4f)).Length(), 0.5f,
        "movement combine normalized partial analog movement");
    RequireClose(AnalogInput.CombineMovement(new Vector2(1f, 1f), Vector2.Zero).Length(), 1f,
        "movement combine allowed a faster keyboard diagonal");
}

static void ExerciseFpsComposition()
{
    var fps = new FpsInput();
    FpsInputFrame frame = fps.Consume([
        Event(InputEventKind.Key, InputEdge.Pressed, key: KeyboardControl.KeyW),
        Event(InputEventKind.Key, InputEdge.Pressed, key: KeyboardControl.Space),
        Event(InputEventKind.Key, InputEdge.Pressed, key: KeyboardControl.KeyE),
        Event(InputEventKind.ControllerButton, InputEdge.Pressed, button: ControllerButton.Button0),
        Event(InputEventKind.ControllerAxis, axis: ControllerAxis.Axis2, x: 1f),
        Event(InputEventKind.ControllerAxis, axis: ControllerAxis.Axis3, x: 1f),
        Event(InputEventKind.PointerDelta, x: 5f, y: 1f),
    ], 0.25f);
    Require(frame.JumpHeld && frame.JumpPressed && frame.UseHeld && frame.UsePressed
            && frame.Movement.Y > 0f && frame.PointerDelta.X == 5f,
        "FPS bindings did not compose keyboard and controller facts");
    RequireClose(frame.ControllerLookRadians.X, MathF.PI * 0.15f / MathF.Sqrt(2f),
        "right stick was not integrated in radians per second");

    FpsInputFrame anotherRate = fps.Read(0.5f);
    RequireClose(anotherRate.ControllerLookRadians.X, frame.ControllerLookRadians.X * 2f,
        "right-stick look was not frame-rate independent");
    fps.Consume([Event(InputEventKind.Key, InputEdge.Released, key: KeyboardControl.Space)], 0.25f);
    Require(fps.Read(0.25f).JumpHeld, "releasing one device incorrectly released the other device's action");

    LookReceipt look = fps.IntegrateLook(new LookState(0f, 0f), frame);
    Require(look.After.YawRadians > frame.ControllerLookRadians.X && look.After.PitchRadians < 0f,
        "default pointer/controller vertical input was not inverted through Look.IntegrateClamped");
}

static ProductInputEvent Event(
    InputEventKind kind,
    InputEdge edge = InputEdge.None,
    KeyboardControl key = KeyboardControl.None,
    ControllerButton button = ControllerButton.None,
    ControllerAxis axis = ControllerAxis.None,
    float x = 0f,
    float y = 0f) => default(ProductInputEvent) with
{
    Kind = kind,
    Edge = edge,
    Keyboard = key,
    ControllerButton = button,
    ControllerAxis = axis,
    X = x,
    Y = y,
};

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidOperationException(message);
    }
}

static void RequireClose(float actual, float expected, string message)
{
    if (MathF.Abs(actual - expected) > 0.0001f)
    {
        throw new InvalidOperationException($"{message}: expected {expected}, got {actual}.");
    }
}
