use std::{
    env, fs,
    io::{Read, Write},
    net::{Ipv4Addr, TcpStream},
    path::{Path, PathBuf},
};

use csharp_product_runtime::{
    CsharpProductContent, CsharpProductRuntime, CsharpProductRuntimeConfig,
};
use product_dev_host::{
    product_dev_renderer_preload_entries, ProductDevBundle, ProductDevBundleEntry, ProductDevHost,
    ProductDevHostConfig, ProductDevRendererResource,
};
use runtime_input::{
    CompiledInputMappings, ControllerAxis, ControllerButton, DirectInputIntentDescriptor,
    InputAxis, InputContext, InputEdge, IntentValueKind, KeyboardControl, PointerButton,
    RuntimeInputMapping, RuntimeInputTrigger,
};
use runtime_lifecycle::RuntimeLifecycleConfig;

const MAX_PHYSICAL_MAPPINGS: usize = 256;
const MAX_MAPPING_CHORD_CONTROLS: usize = 8;

const PHYSICAL_MAPPING_USAGE: &str = "--physical-mapping <mapping-id>=<intent-id>:<trigger>\n\
  key:<keyboard-control>:<held|pressed|released>[:context=<identity>][:chord=<keyboard-control>+...]\n\
  pointer-button:<primary|secondary|middle>:<held|pressed|released>[:context=<identity>]\n\
  pointer-axis:<x|y>[:context=<identity>]\n\
  wheel:<x|y>[:context=<identity>]\n\
  controller-button:<button-0..button-15>:<held|pressed|released>[:context=<identity>]\n\
  controller-axis:<axis-0..axis-3>[:context=<identity>]\n\
keyboard controls: key-a..key-z, digit-0..digit-9, space, enter, escape, shift-left,\n\
  shift-right, control-left, control-right, alt-left, alt-right";

fn main() -> Result<(), String> {
    let args = Arguments::parse()?;
    let content =
        CsharpProductContent::admit(&args.content_dir).map_err(|error| error.to_string())?;
    let mut runtime = match args.loader {
        ProductLoader::NativeAot => {
            CsharpProductRuntime::load_admitted(&args.library, content, args.runtime_config())
        }
        ProductLoader::CoreClr => CsharpProductRuntime::load_coreclr_admitted(
            &args.library,
            args.runtime_config_path
                .as_ref()
                .expect("CoreCLR arguments require a runtimeconfig path"),
            content,
            args.runtime_config(),
        ),
    }
    .map_err(|error| error.to_string())?;
    let bundle = load_bundle(&args.bundle_dir, runtime.render_resources())?;
    if args.exercise {
        runtime
            .exercise_updates()
            .map_err(|error| error.to_string())?;
    }
    let host = ProductDevHost::start(
        runtime,
        ProductDevHostConfig::new(args.port, bundle)
            .with_bind_host(args.bind_host)
            .with_live_debug(args.live_debug),
    )
    .map_err(|error| error.to_string())?;
    if args.exercise {
        let mut stream = TcpStream::connect(host.address()).map_err(|error| error.to_string())?;
        let request = format!(
            "GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            host.address()
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|error| error.to_string())?;
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|error| error.to_string())?;
        if !response.starts_with("HTTP/1.1 200") {
            return Err("loopback product host did not serve index.html".to_owned());
        }
        println!(
            "{} lifecycle and loopback host exercise passed at {}",
            args.loader.label(),
            host.origin()
        );
        host.shutdown().map_err(|error| error.to_string())?;
    } else {
        println!(
            "C# {} product host listening at {}",
            args.loader.label(),
            host.origin()
        );
        println!("Press Ctrl+C to stop.");
        wait_for_process_termination();
    }
    Ok(())
}

/// The standard host is owned by its foreground process supervisor. In
/// particular, service launchers commonly provide a closed stdin, so EOF must
/// not be interpreted as a request to shut the host down.
fn wait_for_process_termination() -> ! {
    loop {
        std::thread::park();
    }
}

#[derive(Debug)]
struct Arguments {
    loader: ProductLoader,
    library: PathBuf,
    runtime_config_path: Option<PathBuf>,
    bundle_dir: PathBuf,
    content_dir: PathBuf,
    port: u16,
    bind_host: Ipv4Addr,
    mode: RuntimeMode,
    direct_intents: Vec<DirectInputIntentDescriptor>,
    physical_mappings: Vec<RuntimeInputMapping>,
    persistence_root: Option<PathBuf>,
    content_store_root: Option<PathBuf>,
    live_debug: bool,
    exercise: bool,
}

#[derive(Clone, Copy, Debug)]
enum ProductLoader {
    NativeAot,
    CoreClr,
}

impl ProductLoader {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "nativeaot" => Ok(Self::NativeAot),
            "coreclr" => Ok(Self::CoreClr),
            _ => Err("--loader must be nativeaot or coreclr".to_owned()),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::NativeAot => "NativeAOT",
            Self::CoreClr => "CoreCLR",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RuntimeMode {
    Realtime,
    Demand,
    External,
}

impl RuntimeMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "realtime" => Ok(Self::Realtime),
            "demand" => Ok(Self::Demand),
            "external" => Ok(Self::External),
            _ => Err("--mode must be realtime, demand, or external".to_owned()),
        }
    }

    fn lifecycle_config(self) -> RuntimeLifecycleConfig {
        match self {
            Self::Realtime => CsharpProductRuntime::standard_realtime_config(),
            Self::Demand => RuntimeLifecycleConfig::Demand,
            Self::External => RuntimeLifecycleConfig::External,
        }
    }
}

impl Arguments {
    fn runtime_config(&self) -> CsharpProductRuntimeConfig {
        let (direct_intents, physical_mappings) = self.input_configuration();
        let mut config =
            CsharpProductRuntimeConfig::new(self.mode.lifecycle_config(), direct_intents)
                .with_physical_mappings(physical_mappings);
        if let Some(root) = &self.persistence_root {
            config = config.with_persistence_root(root.clone());
        }
        if let Some(root) = &self.content_store_root {
            config = config.with_content_store_root(root.clone());
        }
        config
    }

    /// The exercise retains its existing fixed declaration while normal launch
    /// declarations remain exactly in command-line order.
    fn input_configuration(&self) -> (Vec<DirectInputIntentDescriptor>, Vec<RuntimeInputMapping>) {
        let mut direct_intents = self.direct_intents.clone();
        let mut physical_mappings = self.physical_mappings.clone();
        if self.exercise
            && !direct_intents
                .iter()
                .any(|descriptor| descriptor.id() == "runtime.exercise.move")
        {
            direct_intents.push(
                DirectInputIntentDescriptor::new("runtime.exercise.move", IntentValueKind::Digital)
                    .expect("fixed exercise mapping intent"),
            );
        }
        if self.exercise {
            physical_mappings.push(
                RuntimeInputMapping::new(
                    "runtime.exercise.move",
                    "runtime.exercise.move",
                    RuntimeInputTrigger::Key {
                        code: KeyboardControl::KeyW,
                        edge: InputEdge::Held,
                        chord: Vec::new(),
                        context: None,
                    },
                )
                .expect("fixed exercise physical mapping"),
            );
        }
        (direct_intents, physical_mappings)
    }

    fn validate_input_configuration(&self) -> Result<(), String> {
        let (direct_intents, physical_mappings) = self.input_configuration();
        CompiledInputMappings::standard(direct_intents, physical_mappings)
            .map(|_| ())
            .map_err(|error| format!("--physical-mapping configuration is invalid: {error}"))
    }

    fn parse() -> Result<Self, String> {
        Self::parse_from(env::args().skip(1))
    }

    fn parse_from(values: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut library = None;
        let mut loader = None;
        let mut runtime_config_path = None;
        let mut bundle_dir = None;
        let mut content_dir = None;
        let mut port = 0;
        let mut bind_host = Ipv4Addr::LOCALHOST;
        let mut mode = None;
        let mut direct_intents = Vec::new();
        let mut physical_mappings = Vec::new();
        let mut persistence_root = None;
        let mut content_store_root = None;
        let mut live_debug = false;
        let mut exercise = false;
        let mut values = values.into_iter();
        while let Some(arg) = values.next() {
            match arg.as_str() {
                "--loader" => loader = Some(ProductLoader::parse(&values.next().ok_or("--loader requires a value")?)?),
                "--library" => library = values.next().map(PathBuf::from),
                "--runtimeconfig" => runtime_config_path = values.next().map(PathBuf::from),
                "--bundle-dir" => bundle_dir = values.next().map(PathBuf::from),
                "--content-dir" => content_dir = values.next().map(PathBuf::from),
                "--port" => port = values.next().ok_or("--port requires a value")?.parse().map_err(|_| "--port must be a u16")?,
                "--bind-host" => bind_host = values.next().ok_or("--bind-host requires an IPv4 address")?.parse().map_err(|_| "--bind-host must be an IPv4 address")?,
                "--mode" => mode = Some(RuntimeMode::parse(&values.next().ok_or("--mode requires a value")?)?),
                "--persistence-root" => {
                    persistence_root = Some(PathBuf::from(
                        values
                            .next()
                            .ok_or("--persistence-root requires a value")?,
                    ))
                }
                "--content-store-root" => {
                    content_store_root = Some(PathBuf::from(values.next().ok_or("--content-store-root requires a value")?))
                }
                "--live-debug" => live_debug = true,
                "--direct-intent" => direct_intents.push(parse_direct_intent(
                    &values.next().ok_or("--direct-intent requires id=digital, id=axis, or id=payload:contract")?,
                )?),
                "--physical-mapping" => {
                    if physical_mappings.len() == MAX_PHYSICAL_MAPPINGS {
                        return Err(format!(
                            "--physical-mapping accepts at most {MAX_PHYSICAL_MAPPINGS} declarations"
                        ));
                    }
                    physical_mappings.push(parse_physical_mapping(
                        &values.next().ok_or("--physical-mapping requires a declaration")?,
                    )?);
                }
                "--exercise" => exercise = true,
                "--help" => return Err(format!(
                    "usage: csharp-product-runtime [--loader <nativeaot|coreclr>] --library <product.so|product.dll> [--runtimeconfig <product.runtimeconfig.json>] --bundle-dir <browser-bundle> --content-dir <content> --mode <realtime|demand|external> [--persistence-root <absolute-path>] [--content-store-root <absolute-path>] [--direct-intent <id=digital|axis|payload:contract>] [--physical-mapping <declaration>] [--bind-host <ipv4>] [--port <u16>] [--live-debug] [--exercise]\n\n`--live-debug` explicitly admits the trusted product live-debug HTTP routes; they are absent by default. The default loader is `nativeaot`, preserving the existing shared-library workflow and its generated `rusty_product_bind` symbol. Select `coreclr` explicitly for development-only managed loading; it requires --runtimeconfig and resolves the same generated NativeProductApi bind entry point through nethost/hostfxr.\n\n{PHYSICAL_MAPPING_USAGE}"
                )),
                _ => return Err(format!("unknown argument `{arg}`")),
            }
        }
        let arguments = Self {
            loader: loader.unwrap_or(ProductLoader::NativeAot),
            library: library.ok_or("--library is required")?,
            runtime_config_path,
            bundle_dir: bundle_dir.ok_or("--bundle-dir is required")?,
            content_dir: content_dir.ok_or("--content-dir is required")?,
            port,
            bind_host,
            mode: mode.ok_or("--mode is required")?,
            direct_intents,
            physical_mappings,
            persistence_root,
            content_store_root,
            live_debug,
            exercise,
        };
        match (arguments.loader, &arguments.runtime_config_path) {
            (ProductLoader::CoreClr, None) => {
                return Err(
                    "--loader coreclr requires --runtimeconfig <product.runtimeconfig.json>"
                        .to_owned(),
                );
            }
            (ProductLoader::NativeAot, Some(_)) => {
                return Err("--runtimeconfig is only valid with --loader coreclr".to_owned());
            }
            _ => {}
        }
        arguments.validate_input_configuration()?;
        Ok(arguments)
    }
}

fn parse_direct_intent(value: &str) -> Result<DirectInputIntentDescriptor, String> {
    let (id, value_kind) = value
        .split_once('=')
        .ok_or("--direct-intent requires id=digital, id=axis, or id=payload:contract")?;
    if let Some(contract) = value_kind.strip_prefix("payload:") {
        return DirectInputIntentDescriptor::product_payload(id, contract)
            .map_err(|error| error.to_string());
    }
    let value_kind = match value_kind {
        "digital" => IntentValueKind::Digital,
        "axis" => IntentValueKind::Axis,
        _ => return Err("--direct-intent value kind must be digital or axis".to_owned()),
    };
    DirectInputIntentDescriptor::new(id, value_kind).map_err(|error| error.to_string())
}

fn parse_physical_mapping(value: &str) -> Result<RuntimeInputMapping, String> {
    let (mapping_id, declaration) = value
        .split_once('=')
        .ok_or("--physical-mapping requires mapping-id=intent-id:<trigger>")?;
    let mut tokens = declaration.split(':');
    let intent = tokens
        .next()
        .ok_or("--physical-mapping requires an intent id")?;
    let trigger_kind = tokens
        .next()
        .ok_or("--physical-mapping requires a trigger kind")?;
    let trigger = match trigger_kind {
        "key" => {
            let code =
                parse_keyboard_control(next_mapping_token(&mut tokens, "keyboard control")?)?;
            let edge = parse_input_edge(next_mapping_token(&mut tokens, "key edge")?)?;
            let (context, chord) = parse_mapping_qualifiers(&mut tokens, true)?;
            RuntimeInputTrigger::Key {
                code,
                edge,
                chord,
                context,
            }
        }
        "pointer-button" => RuntimeInputTrigger::PointerButton {
            button: parse_pointer_button(next_mapping_token(&mut tokens, "pointer button")?)?,
            edge: parse_input_edge(next_mapping_token(&mut tokens, "pointer-button edge")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        "pointer-axis" => RuntimeInputTrigger::PointerAxis {
            axis: parse_input_axis(next_mapping_token(&mut tokens, "pointer axis")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        "wheel" => RuntimeInputTrigger::Wheel {
            axis: parse_input_axis(next_mapping_token(&mut tokens, "wheel axis")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        "controller-button" => RuntimeInputTrigger::ControllerButton {
            button: parse_controller_button(next_mapping_token(&mut tokens, "controller button")?)?,
            edge: parse_input_edge(next_mapping_token(&mut tokens, "controller-button edge")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        "controller-axis" => RuntimeInputTrigger::ControllerAxis {
            axis: parse_controller_axis(next_mapping_token(&mut tokens, "controller axis")?)?,
            context: parse_mapping_qualifiers(&mut tokens, false)?.0,
        },
        _ => {
            return Err(format!(
                "--physical-mapping trigger `{trigger_kind}` is unsupported; expected key, pointer-button, pointer-axis, wheel, controller-button, or controller-axis"
            ));
        }
    };
    RuntimeInputMapping::new(mapping_id, intent, trigger)
        .map_err(|error| format!("--physical-mapping declaration is invalid: {error}"))
}

fn next_mapping_token<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    expected: &str,
) -> Result<&'a str, String> {
    tokens
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("--physical-mapping requires a {expected}"))
}

fn parse_mapping_qualifiers<'a>(
    tokens: &mut impl Iterator<Item = &'a str>,
    allows_chord: bool,
) -> Result<(Option<InputContext>, Vec<KeyboardControl>), String> {
    let mut context = None;
    let mut chord = Vec::new();
    for qualifier in tokens {
        if let Some(value) = qualifier.strip_prefix("context=") {
            if context.is_some() {
                return Err("--physical-mapping may declare context only once".to_owned());
            }
            context = Some(
                InputContext::new(value)
                    .map_err(|error| format!("--physical-mapping context is invalid: {error}"))?,
            );
        } else if let Some(value) = qualifier.strip_prefix("chord=") {
            if !allows_chord {
                return Err(
                    "--physical-mapping chord is supported only for key triggers".to_owned(),
                );
            }
            if !chord.is_empty() {
                return Err("--physical-mapping may declare chord only once".to_owned());
            }
            chord = parse_chord(value)?;
        } else {
            return Err(format!(
                "--physical-mapping qualifier `{qualifier}` is unsupported"
            ));
        }
    }
    Ok((context, chord))
}

fn parse_chord(value: &str) -> Result<Vec<KeyboardControl>, String> {
    let controls = value
        .split('+')
        .map(parse_keyboard_control)
        .collect::<Result<Vec<_>, _>>()?;
    if controls.is_empty() || controls.len() > MAX_MAPPING_CHORD_CONTROLS {
        return Err(format!(
            "--physical-mapping chord must contain 1 to {MAX_MAPPING_CHORD_CONTROLS} keyboard controls"
        ));
    }
    if (1..controls.len()).any(|index| controls[..index].contains(&controls[index])) {
        return Err("--physical-mapping chord controls must be unique".to_owned());
    }
    Ok(controls)
}

fn parse_input_edge(value: &str) -> Result<InputEdge, String> {
    match value {
        "held" => Ok(InputEdge::Held),
        "pressed" => Ok(InputEdge::Pressed),
        "released" => Ok(InputEdge::Released),
        _ => Err(format!("--physical-mapping edge `{value}` is unsupported")),
    }
}

fn parse_keyboard_control(value: &str) -> Result<KeyboardControl, String> {
    let control = match value {
        "key-a" => KeyboardControl::KeyA,
        "key-b" => KeyboardControl::KeyB,
        "key-c" => KeyboardControl::KeyC,
        "key-d" => KeyboardControl::KeyD,
        "key-e" => KeyboardControl::KeyE,
        "key-f" => KeyboardControl::KeyF,
        "key-g" => KeyboardControl::KeyG,
        "key-h" => KeyboardControl::KeyH,
        "key-i" => KeyboardControl::KeyI,
        "key-j" => KeyboardControl::KeyJ,
        "key-k" => KeyboardControl::KeyK,
        "key-l" => KeyboardControl::KeyL,
        "key-m" => KeyboardControl::KeyM,
        "key-n" => KeyboardControl::KeyN,
        "key-o" => KeyboardControl::KeyO,
        "key-p" => KeyboardControl::KeyP,
        "key-q" => KeyboardControl::KeyQ,
        "key-r" => KeyboardControl::KeyR,
        "key-s" => KeyboardControl::KeyS,
        "key-t" => KeyboardControl::KeyT,
        "key-u" => KeyboardControl::KeyU,
        "key-v" => KeyboardControl::KeyV,
        "key-w" => KeyboardControl::KeyW,
        "key-x" => KeyboardControl::KeyX,
        "key-y" => KeyboardControl::KeyY,
        "key-z" => KeyboardControl::KeyZ,
        "digit-0" => KeyboardControl::Digit0,
        "digit-1" => KeyboardControl::Digit1,
        "digit-2" => KeyboardControl::Digit2,
        "digit-3" => KeyboardControl::Digit3,
        "digit-4" => KeyboardControl::Digit4,
        "digit-5" => KeyboardControl::Digit5,
        "digit-6" => KeyboardControl::Digit6,
        "digit-7" => KeyboardControl::Digit7,
        "digit-8" => KeyboardControl::Digit8,
        "digit-9" => KeyboardControl::Digit9,
        "space" => KeyboardControl::Space,
        "enter" => KeyboardControl::Enter,
        "escape" => KeyboardControl::Escape,
        "shift-left" => KeyboardControl::ShiftLeft,
        "shift-right" => KeyboardControl::ShiftRight,
        "control-left" => KeyboardControl::ControlLeft,
        "control-right" => KeyboardControl::ControlRight,
        "alt-left" => KeyboardControl::AltLeft,
        "alt-right" => KeyboardControl::AltRight,
        _ => {
            return Err(format!(
                "--physical-mapping keyboard control `{value}` is unsupported"
            ))
        }
    };
    Ok(control)
}

fn parse_pointer_button(value: &str) -> Result<PointerButton, String> {
    match value {
        "primary" => Ok(PointerButton::Primary),
        "secondary" => Ok(PointerButton::Secondary),
        "middle" => Ok(PointerButton::Middle),
        _ => Err(format!(
            "--physical-mapping pointer button `{value}` is unsupported"
        )),
    }
}

fn parse_input_axis(value: &str) -> Result<InputAxis, String> {
    match value {
        "x" => Ok(InputAxis::X),
        "y" => Ok(InputAxis::Y),
        _ => Err(format!("--physical-mapping axis `{value}` is unsupported")),
    }
}

fn parse_controller_button(value: &str) -> Result<ControllerButton, String> {
    match value {
        "button-0" => Ok(ControllerButton::Button0),
        "button-1" => Ok(ControllerButton::Button1),
        "button-2" => Ok(ControllerButton::Button2),
        "button-3" => Ok(ControllerButton::Button3),
        "button-4" => Ok(ControllerButton::Button4),
        "button-5" => Ok(ControllerButton::Button5),
        "button-6" => Ok(ControllerButton::Button6),
        "button-7" => Ok(ControllerButton::Button7),
        "button-8" => Ok(ControllerButton::Button8),
        "button-9" => Ok(ControllerButton::Button9),
        "button-10" => Ok(ControllerButton::Button10),
        "button-11" => Ok(ControllerButton::Button11),
        "button-12" => Ok(ControllerButton::Button12),
        "button-13" => Ok(ControllerButton::Button13),
        "button-14" => Ok(ControllerButton::Button14),
        "button-15" => Ok(ControllerButton::Button15),
        _ => {
            return Err(format!(
                "--physical-mapping controller button `{value}` is unsupported"
            ))
        }
    }
}

fn parse_controller_axis(value: &str) -> Result<ControllerAxis, String> {
    match value {
        "axis-0" => Ok(ControllerAxis::Axis0),
        "axis-1" => Ok(ControllerAxis::Axis1),
        "axis-2" => Ok(ControllerAxis::Axis2),
        "axis-3" => Ok(ControllerAxis::Axis3),
        _ => Err(format!(
            "--physical-mapping controller axis `{value}` is unsupported"
        )),
    }
}

fn load_bundle(
    root: &Path,
    render_resources: &[ProductDevRendererResource],
) -> Result<ProductDevBundle, String> {
    let mut entries = Vec::new();
    collect_bundle(root, root, &mut entries)?;
    entries.extend(
        product_dev_renderer_preload_entries(render_resources)
            .map_err(|error| error.to_string())?,
    );
    ProductDevBundle::new(entries).map_err(|error| error.to_string())
}

fn collect_bundle(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<ProductDevBundleEntry>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_bundle(root, &path, entries)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "bundle entry escaped root")?
                .to_string_lossy()
                .replace('\\', "/");
            let content_type = content_type(&relative)
                .ok_or_else(|| format!("bundle file `{relative}` has no admitted content type"))?;
            entries.push(
                ProductDevBundleEntry::new(
                    relative,
                    content_type,
                    fs::read(path).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?,
            );
        }
    }
    Ok(())
}

fn content_type(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()? {
        "html" => Some("text/html; charset=utf-8"),
        "js" => Some("text/javascript; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "json" => Some("application/json; charset=utf-8"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "woff2" => Some("font/woff2"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_test_args(arguments: &[&str]) -> Result<Arguments, String> {
        let mut values = vec![
            "--library".to_owned(),
            "product.so".to_owned(),
            "--bundle-dir".to_owned(),
            "bundle".to_owned(),
            "--content-dir".to_owned(),
            "content".to_owned(),
            "--mode".to_owned(),
            "demand".to_owned(),
        ];
        values.extend(arguments.iter().map(|value| (*value).to_owned()));
        Arguments::parse_from(values)
    }

    fn parse_test_error(arguments: &[&str]) -> String {
        match parse_test_args(arguments) {
            Ok(_) => panic!("argument list unexpectedly parsed"),
            Err(error) => error,
        }
    }

    #[test]
    fn normal_launch_preserves_wasd_mapping_declaration_order() {
        let args = parse_test_args(&[
            "--direct-intent",
            "move.forward=digital",
            "--direct-intent",
            "move.left=digital",
            "--direct-intent",
            "move.backward=digital",
            "--direct-intent",
            "move.right=digital",
            "--physical-mapping",
            "move-forward=move.forward:key:key-w:held",
            "--physical-mapping",
            "move-left=move.left:key:key-a:held",
            "--physical-mapping",
            "move-backward=move.backward:key:key-s:held",
            "--physical-mapping",
            "move-right=move.right:key:key-d:held",
        ])
        .expect("normal mapping declaration parses");

        assert!(!args.exercise);
        assert!(matches!(args.loader, ProductLoader::NativeAot));
        let (intents, mappings) = args.input_configuration();
        assert_eq!(
            mappings
                .iter()
                .map(RuntimeInputMapping::id)
                .collect::<Vec<_>>(),
            ["move-forward", "move-left", "move-backward", "move-right"]
        );
        assert!(CompiledInputMappings::standard(intents, mappings).is_ok());
    }

    #[test]
    fn parser_supports_every_typed_physical_trigger_family() {
        let args = parse_test_args(&[
            "--direct-intent",
            "key=digital",
            "--direct-intent",
            "pointer-button=digital",
            "--direct-intent",
            "pointer-axis=axis",
            "--direct-intent",
            "wheel=axis",
            "--direct-intent",
            "controller-button=digital",
            "--direct-intent",
            "controller-axis=axis",
            "--physical-mapping",
            "key=key:key:key-w:pressed",
            "--physical-mapping",
            "pointer-button=pointer-button:pointer-button:primary:released",
            "--physical-mapping",
            "pointer-axis=pointer-axis:pointer-axis:x",
            "--physical-mapping",
            "wheel=wheel:wheel:y",
            "--physical-mapping",
            "controller-button=controller-button:controller-button:button-0:held",
            "--physical-mapping",
            "controller-axis=controller-axis:controller-axis:axis-3",
        ])
        .expect("all supported trigger families parse");
        let (_, mappings) = args.input_configuration();

        assert!(matches!(
            mappings[0].trigger(),
            RuntimeInputTrigger::Key { .. }
        ));
        assert!(matches!(
            mappings[1].trigger(),
            RuntimeInputTrigger::PointerButton { .. }
        ));
        assert!(matches!(
            mappings[2].trigger(),
            RuntimeInputTrigger::PointerAxis { .. }
        ));
        assert!(matches!(
            mappings[3].trigger(),
            RuntimeInputTrigger::Wheel { .. }
        ));
        assert!(matches!(
            mappings[4].trigger(),
            RuntimeInputTrigger::ControllerButton { .. }
        ));
        assert!(matches!(
            mappings[5].trigger(),
            RuntimeInputTrigger::ControllerAxis { .. }
        ));
    }

    #[test]
    fn parser_admits_key_context_and_chord() {
        let args = parse_test_args(&[
            "--direct-intent",
            "editor.save=digital",
            "--physical-mapping",
            "save=editor.save:key:key-s:pressed:context=editor.text:chord=control-left+shift-left",
        ])
        .expect("contextual chord parses");
        let (_, mappings) = args.input_configuration();
        let RuntimeInputTrigger::Key {
            code,
            edge,
            chord,
            context,
        } = mappings[0].trigger()
        else {
            panic!("expected key trigger");
        };
        assert_eq!(*code, KeyboardControl::KeyS);
        assert_eq!(*edge, InputEdge::Pressed);
        assert_eq!(
            chord,
            &[KeyboardControl::ControlLeft, KeyboardControl::ShiftLeft]
        );
        assert_eq!(
            context.as_ref().map(InputContext::as_str),
            Some("editor.text")
        );
    }

    #[test]
    fn parser_rejects_malformed_unknown_duplicate_and_mismatched_mappings() {
        for declaration in [
            "move=move.forward:key:key-w",
            "move=move.forward:gesture:key-w:held",
            "move=move.forward:key:not-a-key:held",
            "move=move.forward:pointer-axis:x:chord=control-left",
        ] {
            let error = parse_test_error(&[
                "--direct-intent",
                "move.forward=digital",
                "--physical-mapping",
                declaration,
            ]);
            assert!(error.contains("--physical-mapping"));
        }

        let unknown = parse_test_error(&[
            "--direct-intent",
            "move.forward=digital",
            "--physical-mapping",
            "move=move.missing:key:key-w:held",
        ]);
        assert!(unknown.contains("UnknownIntent"));

        let duplicate = parse_test_error(&[
            "--direct-intent",
            "move.forward=digital",
            "--physical-mapping",
            "move=move.forward:key:key-w:held",
            "--physical-mapping",
            "move=move.forward:key:key-a:held",
        ]);
        assert!(duplicate.contains("DuplicateMapping"));

        let mismatch = parse_test_error(&[
            "--direct-intent",
            "move.forward=digital",
            "--physical-mapping",
            "move=move.forward:pointer-axis:x",
        ]);
        assert!(mismatch.contains("IntentValueKindMismatch"));
    }

    #[test]
    fn parser_enforces_mapping_and_chord_declaration_bounds() {
        let mut values = vec!["--direct-intent", "move.forward=digital"];
        for _ in 0..=MAX_PHYSICAL_MAPPINGS {
            values.extend(["--physical-mapping", "move=move.forward:key:key-w:held"]);
        }
        let error = parse_test_error(&values);
        assert!(error.contains("at most 256"));

        let chord = (0..=MAX_MAPPING_CHORD_CONTROLS)
            .map(|_| "key-w")
            .collect::<Vec<_>>()
            .join("+");
        let declaration = format!("move=move.forward:key:key-w:held:chord={chord}");
        let error = parse_test_error(&[
            "--direct-intent",
            "move.forward=digital",
            "--physical-mapping",
            &declaration,
        ]);
        assert!(error.contains("1 to 8"));
    }

    #[test]
    fn help_documents_the_physical_mapping_vocabulary() {
        let error = parse_test_error(&["--help"]);
        assert!(error.contains(PHYSICAL_MAPPING_USAGE));
    }

    #[test]
    fn parser_requires_an_explicit_coreclr_runtimeconfig() {
        let missing = Arguments::parse_from(
            [
                "--loader",
                "coreclr",
                "--library",
                "product.dll",
                "--bundle-dir",
                "bundle",
                "--content-dir",
                "content",
                "--mode",
                "demand",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        assert!(missing
            .expect_err("CoreCLR without runtimeconfig is rejected")
            .contains("requires --runtimeconfig"));

        let native_with_runtimeconfig =
            parse_test_error(&["--runtimeconfig", "product.runtimeconfig.json"]);
        assert!(native_with_runtimeconfig.contains("only valid with --loader coreclr"));
    }
}
