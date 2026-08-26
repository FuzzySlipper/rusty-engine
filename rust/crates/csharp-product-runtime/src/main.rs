use std::{
    env, fs,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
};

use csharp_product_runtime::{CsharpProductContent, CsharpProductRuntime};
use product_dev_host::{
    product_dev_renderer_preload_entries, ProductDevBundle, ProductDevBundleEntry, ProductDevHost,
    ProductDevHostConfig, ProductDevRendererResource,
};

fn main() -> Result<(), String> {
    let args = Arguments::parse()?;
    let content =
        CsharpProductContent::admit(&args.content_dir).map_err(|error| error.to_string())?;
    let mut runtime = CsharpProductRuntime::load_admitted(&args.library, content)
        .map_err(|error| error.to_string())?;
    let bundle = load_bundle(&args.bundle_dir, runtime.render_resources())?;
    if args.exercise {
        runtime
            .exercise_turns()
            .map_err(|error| error.to_string())?;
    }
    let host = ProductDevHost::start(runtime, ProductDevHostConfig::new(args.port, bundle))
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
        println!("Press Enter to stop.");
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        host.shutdown().map_err(|error| error.to_string())?;
    }
    Ok(())
}

struct Arguments {
    library: PathBuf,
    bundle_dir: PathBuf,
    content_dir: PathBuf,
    port: u16,
    exercise: bool,
}
impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut library = None;
        let mut bundle_dir = None;
        let mut content_dir = None;
        let mut port = 0;
        let mut exercise = false;
        let mut values = env::args().skip(1);
        while let Some(arg) = values.next() {
            match arg.as_str() {
                "--library" => library = values.next().map(PathBuf::from),
                "--bundle-dir" => bundle_dir = values.next().map(PathBuf::from),
                "--content-dir" => content_dir = values.next().map(PathBuf::from),
                "--port" => port = values.next().ok_or("--port requires a value")?.parse().map_err(|_| "--port must be a u16")?,
                "--exercise" => exercise = true,
                "--help" => return Err("usage: csharp-product-runtime --library <product.so> --bundle-dir <browser-bundle> --content-dir <content> [--port <u16>] [--exercise]".to_owned()),
                _ => return Err(format!("unknown argument `{arg}`")),
            }
        }
        Ok(Self {
            library: library.ok_or("--library is required")?,
            bundle_dir: bundle_dir.ok_or("--bundle-dir is required")?,
            content_dir: content_dir.ok_or("--content-dir is required")?,
            port,
            exercise,
        })
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
