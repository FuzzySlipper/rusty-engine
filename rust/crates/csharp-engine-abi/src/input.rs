//! Typed input values crossing the trusted NativeAOT product boundary.
//!
//! The browser/application host still owns DOM collection and normalization.
//! These values are the copied, host-neutral facts that the Rust runtime has
//! admitted for one product call.  The enums deliberately describe the
//! vocabulary instead of making downstream products decode numeric tags.

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputEventKind {
    Key = 1,
    PointerButton = 2,
    PointerDelta = 3,
    Wheel = 4,
    ControllerButton = 5,
    ControllerAxis = 6,
    Clear = 7,
    DirectDigital = 8,
    DirectAxis = 9,
    DirectProductPayload = 10,
    MappedDigital = 11,
    MappedAxis = 12,
    MappedProductPayload = 13,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputEdge {
    None = 0,
    Held = 1,
    Pressed = 2,
    Released = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputDevice {
    None = 0,
    Keyboard = 1,
    Pointer = 2,
    Controller = 3,
    Product = 4,
    Runtime = 5,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputChannel {
    None = 0,
    Key = 1,
    Button = 2,
    PointerDelta = 3,
    Wheel = 4,
    Axis = 5,
    Clear = 6,
    Intent = 7,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputAxis {
    None = 0,
    X = 1,
    Y = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputClearReason {
    None = 0,
    FocusLoss = 1,
    InteractionModeLoss = 2,
    PointerLockLoss = 3,
    Restart = 4,
    ControlRevisionChange = 5,
    Dispose = 6,
    IngressOverflow = 7,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputValueKind {
    None = 0,
    Digital = 1,
    Axis = 2,
    ProductPayload = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputPhase {
    None = 0,
    Held = 1,
    Pressed = 2,
    Released = 3,
    Axis = 4,
    DirectUi = 5,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputProvenance {
    None = 0,
    Physical = 1,
    DirectUi = 2,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativeKeyboardControl {
    None = 0,
    KeyA = 1,
    KeyB = 2,
    KeyC = 3,
    KeyD = 4,
    KeyE = 5,
    KeyF = 6,
    KeyG = 7,
    KeyH = 8,
    KeyI = 9,
    KeyJ = 10,
    KeyK = 11,
    KeyL = 12,
    KeyM = 13,
    KeyN = 14,
    KeyO = 15,
    KeyP = 16,
    KeyQ = 17,
    KeyR = 18,
    KeyS = 19,
    KeyT = 20,
    KeyU = 21,
    KeyV = 22,
    KeyW = 23,
    KeyX = 24,
    KeyY = 25,
    KeyZ = 26,
    Digit0 = 27,
    Digit1 = 28,
    Digit2 = 29,
    Digit3 = 30,
    Digit4 = 31,
    Digit5 = 32,
    Digit6 = 33,
    Digit7 = 34,
    Digit8 = 35,
    Digit9 = 36,
    Space = 37,
    Enter = 38,
    Escape = 39,
    ShiftLeft = 40,
    ShiftRight = 41,
    ControlLeft = 42,
    ControlRight = 43,
    AltLeft = 44,
    AltRight = 45,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativePointerButton {
    None = 0,
    Primary = 1,
    Secondary = 2,
    Middle = 3,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativeControllerButton {
    None = 0,
    Button0 = 1,
    Button1 = 2,
    Button2 = 3,
    Button3 = 4,
    Button4 = 5,
    Button5 = 6,
    Button6 = 7,
    Button7 = 8,
    Button8 = 9,
    Button9 = 10,
    Button10 = 11,
    Button11 = 12,
    Button12 = 13,
    Button13 = 14,
    Button14 = 15,
    Button15 = 16,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NativeControllerAxis {
    None = 0,
    Axis0 = 1,
    Axis1 = 2,
    Axis2 = 3,
    Axis3 = 4,
}

/// Typed physical trigger family used by the standard runtime's create-time
/// mapping configuration.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeInputTriggerKind {
    Key = 1,
    PointerButton = 2,
    PointerAxis = 3,
    Wheel = 4,
    ControllerButton = 5,
    ControllerAxis = 6,
}

/// Runtime/control identity carried with every admitted product input fact.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeInputBinding {
    pub instance_id: u64,
    pub generation: u64,
    pub control_revision: u64,
}

/// A typed sequence value. The runtime remains the sole source of admission;
/// this wrapper only prevents downstream code from confusing it with a tag.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeInputSequence {
    pub value: u64,
}

/// One admitted intent descriptor made available to the product at creation.
/// All pointers are borrowed for the duration of product creation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeInputDescriptor {
    pub id: *const u8,
    pub id_len: usize,
    pub value_kind: NativeInputValueKind,
    pub payload_contract: *const u8,
    pub payload_contract_len: usize,
}

/// One typed physical mapping admitted by the standard runtime. The scalar
/// control fields are interpreted according to `trigger_kind`; all strings and
/// the optional keyboard chord are borrowed for product creation only.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeInputMapping {
    pub id: *const u8,
    pub id_len: usize,
    pub intent: *const u8,
    pub intent_len: usize,
    pub trigger_kind: NativeInputTriggerKind,
    pub edge: NativeInputEdge,
    pub axis: NativeInputAxis,
    pub keyboard: NativeKeyboardControl,
    pub pointer_button: NativePointerButton,
    pub controller_button: NativeControllerButton,
    pub controller_axis: NativeControllerAxis,
    pub chord: *const NativeKeyboardControl,
    pub chord_len: usize,
    pub context: *const u8,
    pub context_len: usize,
}

/// Static input facts selected by the Engine-owned standard runtime. Products
/// may inspect these descriptors but cannot alter the admitted runtime lane.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeInputConfiguration {
    pub binding: NativeInputBinding,
    pub context: *const u8,
    pub context_len: usize,
    pub direct_intents: *const NativeInputDescriptor,
    pub direct_intents_len: usize,
    pub physical_mappings: *const NativeInputMapping,
    pub physical_mappings_len: usize,
}

/// One admitted host-neutral input fact copied into a product turn.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeInputEvent {
    pub kind: NativeInputEventKind,
    pub edge: NativeInputEdge,
    pub device: NativeInputDevice,
    pub channel: NativeInputChannel,
    pub axis: NativeInputAxis,
    pub keyboard: NativeKeyboardControl,
    pub pointer_button: NativePointerButton,
    pub controller_button: NativeControllerButton,
    pub controller_axis: NativeControllerAxis,
    pub clear_reason: NativeInputClearReason,
    pub value_kind: NativeInputValueKind,
    pub phase: NativeInputPhase,
    pub provenance: NativeInputProvenance,
    pub binding: NativeInputBinding,
    pub sequence: NativeInputSequence,
    pub x: f32,
    pub y: f32,
    /// Physical control label retained for compatibility and diagnostics.
    pub label: *const u8,
    pub label_len: usize,
    /// Product mapping identity when the runtime emits a mapped intent.
    pub mapping_id: *const u8,
    pub mapping_id_len: usize,
    /// Intent identity for direct or mapped product values.
    pub intent: *const u8,
    pub intent_len: usize,
    /// Product input context for this fact.
    pub context: *const u8,
    pub context_len: usize,
    /// Contract and bounded bytes for an already-admitted direct product
    /// payload. Both slices are borrowed only for this product turn.
    pub payload_contract: *const u8,
    pub payload_contract_len: usize,
    pub payload_data: *const u8,
    pub payload_data_len: usize,
}
