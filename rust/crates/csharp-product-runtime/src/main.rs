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
use product_model::IntentValueKind;
use runtime_input::{DirectInputIntentDescriptor, RuntimeInputMapping, RuntimeInputTrigger};
use runtime_lifecycle::RuntimeLifecycleConfig;

fn main() -> Result<(), String> {
    let args = Arguments::parse()?;
    let content =
        CsharpProductContent::admit(&args.content_dir).map_err(|error| error.to_string())?;
    let mut runtime =
        CsharpProductRuntime::load_admitted(&args.library, content, args.runtime_config())
            .map_err(|error| error.to_string())?;
    let bundle = load_bundle(&args.bundle_dir, runtime.render_resources())?;
    if args.exercise {
        runtime
            .exercise_turns()
            .map_err(|error| error.to_string())?;
    }
    let host = ProductDevHost::start(
        runtime,
        ProductDevHostConfig::new(args.port, bundle).with_bind_host(args.bind_host),
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
            "NativeAOT lifecycle and loopback host exercise passed at {}",
            host.origin()
        );
        host.shutdown().map_err(|error| error.to_string())?;
    } else {
        println!("C# NativeAOT product host listening at {}", host.origin());
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

struct Arguments {
    library: PathBuf,
    bundle_dir: PathBuf,
    content_dir: PathBuf,
    port: u16,
    bind_host: Ipv4Addr,
    mode: RuntimeMode,
    direct_intents: Vec<DirectInputIntentDescriptor>,
    persistence_root: Option<PathBuf>,
    content_store_root: Option<PathBuf>,
    exercise: bool,
}

#[derive(Clone, Copy)]
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
        let mut direct_intents = self.direct_intents.clone();
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
        let physical_mappings = if self.exercise {
            vec![RuntimeInputMapping::new(
                "runtime.exercise.move",
                "runtime.exercise.move",
                RuntimeInputTrigger::Key {
                    code: product_model::KeyboardControl::KeyW,
                    edge: product_model::InputEdge::Held,
                    chord: Vec::new(),
                    context: None,
                },
            )
            .expect("fixed exercise physical mapping")]
        } else {
            Vec::new()
        };
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

    fn parse() -> Result<Self, String> {
        let mut library = None;
        let mut bundle_dir = None;
        let mut content_dir = None;
        let mut port = 0;
        let mut bind_host = Ipv4Addr::LOCALHOST;
        let mut mode = None;
        let mut direct_intents = Vec::new();
        let mut persistence_root = None;
        let mut content_store_root = None;
        let mut exercise = false;
        let mut values = env::args().skip(1);
        while let Some(arg) = values.next() {
            match arg.as_str() {
                "--library" => library = values.next().map(PathBuf::from),
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
                "--direct-intent" => direct_intents.push(parse_direct_intent(
                    &values.next().ok_or("--direct-intent requires id=digital, id=axis, or id=payload:contract")?,
                )?),
                "--exercise" => exercise = true,
                "--help" => return Err("usage: csharp-product-runtime --library <product.so> --bundle-dir <browser-bundle> --content-dir <content> --mode <realtime|demand|external> [--persistence-root <absolute-path>] [--content-store-root <absolute-path>] [--direct-intent <id=digital|axis|payload:contract>] [--bind-host <ipv4>] [--port <u16>] [--exercise]".to_owned()),
                _ => return Err(format!("unknown argument `{arg}`")),
            }
        }
        Ok(Self {
            library: library.ok_or("--library is required")?,
            bundle_dir: bundle_dir.ok_or("--bundle-dir is required")?,
            content_dir: content_dir.ok_or("--content-dir is required")?,
            port,
            bind_host,
            mode: mode.ok_or("--mode is required")?,
            direct_intents,
            persistence_root,
            content_store_root,
            exercise,
        })
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
