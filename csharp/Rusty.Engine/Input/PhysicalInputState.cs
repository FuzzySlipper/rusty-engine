using System.Numerics;

namespace Rusty.Engine.Input;

/// <summary>
/// Retains physical keyboard, pointer, and controller facts between product updates.
/// Call <see cref="Consume"/> once for each raw <see cref="ProductInputEvent"/> batch.
/// Mapped and direct-intent events are deliberately ignored: products retain their selected mappings.
/// </summary>
public sealed class PhysicalInputState
{
    private readonly HashSet<KeyboardControl> heldKeys = [];
    private readonly HashSet<KeyboardControl> pressedKeys = [];
    private readonly HashSet<KeyboardControl> releasedKeys = [];
    private readonly HashSet<PointerButton> heldPointerButtons = [];
    private readonly HashSet<PointerButton> pressedPointerButtons = [];
    private readonly HashSet<PointerButton> releasedPointerButtons = [];
    private readonly HashSet<ControllerButton> heldControllerButtons = [];
    private readonly HashSet<ControllerButton> pressedControllerButtons = [];
    private readonly HashSet<ControllerButton> releasedControllerButtons = [];
    private readonly Dictionary<ControllerAxis, float> controllerAxes = [];
    private readonly Dictionary<ControllerButton, float> controllerButtonValues = [];
    private readonly HashSet<KeyboardControl> keysHeldAtBatchStart = [];
    private readonly HashSet<PointerButton> pointerButtonsHeldAtBatchStart = [];
    private readonly HashSet<ControllerButton> controllerButtonsHeldAtBatchStart = [];
    private Vector2 pointerDelta;

    /// <summary>Pointer displacement accumulated from the most recently consumed batch, in pointer units.</summary>
    public Vector2 PointerDelta => pointerDelta;

    /// <summary>
    /// Clears one-update edge facts and pointer displacement, then incorporates supported raw physical events.
    /// A press followed by a release in this batch reports both edges while leaving the control unheld.
    /// </summary>
    public void Consume(ReadOnlySpan<ProductInputEvent> events)
    {
        ClearEdges();
        pointerDelta = Vector2.Zero;
        CaptureBatchStart();

        foreach (ProductInputEvent input in events)
        {
            switch (input.Kind)
            {
                case InputEventKind.Clear:
                    ClearDuringConsume();
                    break;
                case InputEventKind.Key:
                    ApplyDigital(input.Keyboard, input.Edge, heldKeys, pressedKeys, releasedKeys);
                    break;
                case InputEventKind.PointerButton:
                    ApplyDigital(input.PointerButton, input.Edge, heldPointerButtons, pressedPointerButtons, releasedPointerButtons);
                    break;
                case InputEventKind.PointerDelta:
                    pointerDelta += new Vector2(input.X, input.Y);
                    break;
                case InputEventKind.ControllerButton:
                    ApplyDigital(input.ControllerButton, input.Edge, heldControllerButtons, pressedControllerButtons, releasedControllerButtons);
                    break;
                case InputEventKind.ControllerAxis:
                    Set(controllerAxes, input.ControllerAxis, input.X);
                    break;
                case InputEventKind.ControllerButtonValue:
                    Set(controllerButtonValues, input.ControllerButton, input.X);
                    break;
            }
        }
    }

    /// <summary>
    /// Neutralizes held controls and retained analog values. Controls that were held report Released for this update;
    /// pending presses and pointer displacement are discarded.
    /// </summary>
    public void Clear()
    {
        releasedKeys.UnionWith(heldKeys);
        releasedPointerButtons.UnionWith(heldPointerButtons);
        releasedControllerButtons.UnionWith(heldControllerButtons);
        NeutralizeDiscardingPresses();
    }

    public bool Held(KeyboardControl control) => heldKeys.Contains(control);
    public bool Pressed(KeyboardControl control) => pressedKeys.Contains(control);
    public bool Released(KeyboardControl control) => releasedKeys.Contains(control);

    public bool Held(PointerButton control) => heldPointerButtons.Contains(control);
    public bool Pressed(PointerButton control) => pressedPointerButtons.Contains(control);
    public bool Released(PointerButton control) => releasedPointerButtons.Contains(control);

    public bool Held(ControllerButton control) => heldControllerButtons.Contains(control);
    public bool Pressed(ControllerButton control) => pressedControllerButtons.Contains(control);
    public bool Released(ControllerButton control) => releasedControllerButtons.Contains(control);

    /// <summary>Returns the latest normalized controller-stick sample for <paramref name="axis"/>, or zero.</summary>
    public float Axis(ControllerAxis axis) => controllerAxes.GetValueOrDefault(axis);

    /// <summary>Returns the latest normalized analog button value for <paramref name="button"/>, or zero.</summary>
    public float ButtonValue(ControllerButton button) => controllerButtonValues.GetValueOrDefault(button);

    private void ClearEdges()
    {
        pressedKeys.Clear();
        releasedKeys.Clear();
        pressedPointerButtons.Clear();
        releasedPointerButtons.Clear();
        pressedControllerButtons.Clear();
        releasedControllerButtons.Clear();
    }

    private void CaptureBatchStart()
    {
        keysHeldAtBatchStart.Clear();
        keysHeldAtBatchStart.UnionWith(heldKeys);
        pointerButtonsHeldAtBatchStart.Clear();
        pointerButtonsHeldAtBatchStart.UnionWith(heldPointerButtons);
        controllerButtonsHeldAtBatchStart.Clear();
        controllerButtonsHeldAtBatchStart.UnionWith(heldControllerButtons);
    }

    private void ClearDuringConsume()
    {
        // A lifecycle clear must make pre-existing held controls observable as released,
        // but it should not turn a just-pressed control in this batch into a one-shot action.
        releasedKeys.UnionWith(keysHeldAtBatchStart);
        releasedPointerButtons.UnionWith(pointerButtonsHeldAtBatchStart);
        releasedControllerButtons.UnionWith(controllerButtonsHeldAtBatchStart);
        NeutralizeDiscardingPresses();
    }

    private void NeutralizeDiscardingPresses()
    {
        heldKeys.Clear();
        heldPointerButtons.Clear();
        heldControllerButtons.Clear();
        controllerAxes.Clear();
        controllerButtonValues.Clear();
        pointerDelta = Vector2.Zero;
        pressedKeys.Clear();
        pressedPointerButtons.Clear();
        pressedControllerButtons.Clear();
    }

    private static void ApplyDigital<TControl>(
        TControl control,
        InputEdge edge,
        HashSet<TControl> held,
        HashSet<TControl> pressed,
        HashSet<TControl> released)
        where TControl : notnull
    {
        if (EqualityComparer<TControl>.Default.Equals(control, default!))
        {
            return;
        }

        switch (edge)
        {
            case InputEdge.Held:
                held.Add(control);
                break;
            case InputEdge.Pressed:
                held.Add(control);
                pressed.Add(control);
                break;
            case InputEdge.Released:
                held.Remove(control);
                released.Add(control);
                break;
        }
    }

    private static void Set<TControl>(Dictionary<TControl, float> values, TControl control, float value)
        where TControl : notnull
    {
        if (!EqualityComparer<TControl>.Default.Equals(control, default!))
        {
            values[control] = value;
        }
    }
}
