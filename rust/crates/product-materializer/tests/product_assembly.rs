use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{Shutdown, TcpStream},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use product_assembly::{
    plan_product_assembly, plan_product_assembly_with_kernel_capabilities, verify_product_assembly,
    verify_product_assembly_with_kernel_capabilities, PublicationFile,
};
use product_materializer::{
    materialize_product, EngineAsset, EngineAssets, MaterializationLimits, MaterializationToolchain,
};
use product_model::{
    decode_product_manifest, CapabilityAccess, CapabilityAvailability, CapabilityBudget,
    CapabilityKind, CapabilityMetadata, CapabilityProvenance, CapabilityUses,
    ProductKernelCapabilityDescriptor,
};

const MANIFEST: &str = r#"[product]
id = "rusty.product.assembly.e2e"

[runtime_composition]
entrypoints = ["rules/main.ts"]

[lifecycle]
mode = "demand"

[ui]
entry = "ui/main.ts"

[content]
root = "content"

[outputs]
compiled_composition = "generated/compiled-composition.json"
admitted_runtime_content = "generated/runtime-content"
product_assembly = "generated/product-assembly"
product_bundle = "generated/product-bundle"
"#;

// The ordinary assembly test exercises the demand route directly so it stays
// cheap and host-neutral. The browser gate uses realtime so the generated
// Engine browser composition root drives one real cadence request without
// needing a product-specific RPC or test-only browser hook.
const BROWSER_MANIFEST: &str = r#"[product]
id = "rusty.product.assembly.e2e"

[runtime_composition]
entrypoints = ["rules/main.ts", "rules/extra.ts"]

[lifecycle]
mode = "realtime"

[lifecycle.realtime]
fixed_step_hz = 60
max_catch_up_steps = 4

[kernel]
entry = "kernel/entry.rs"

[ui]
entry = "ui/main.ts"
projection_stream = "counter"
projection_contract = "counter.v1"

[content]
root = "content"

[outputs]
compiled_composition = "generated/compiled-composition.json"
admitted_runtime_content = "generated/runtime-content"
product_assembly = "generated/product-assembly"
product_bundle = "generated/product-bundle"
"#;

#[test]
#[ignore = "requires prepared Rules and renderer artifacts; run scripts/verify-product-materializer.sh"]
fn materialized_product_assembles_relocates_and_serves_closed_browser_bundle() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let original_root = PathBuf::from(format!("/home/dev/rusty-product-assembly-e2e-{nonce}"));
    let relocated_root = PathBuf::from(format!(
        "/home/dev/rusty-product-assembly-e2e-relocated-{nonce}"
    ));
    create_fixture(&original_root);
    let manifest = decode_product_manifest(MANIFEST).expect("manifest admission");
    let assets = test_assets();
    let toolchain = toolchain();

    let first_materialized = materialize_product(
        &original_root,
        &manifest,
        &assets,
        &toolchain,
        MaterializationLimits::default(),
    )
    .expect("materialize authored Product Model");
    let generated_package = original_root.join("generated/product-assembly");
    let engine_package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rusty-engine");
    let first_inputs = first_materialized
        .assembly_inputs()
        .expect("typed assembly inputs")
        .with_engine_dependency_path(relative_path(&generated_package, &engine_package))
        .expect("relocatable Engine dependency");
    let first_plan = plan_product_assembly(&original_root, &manifest, &first_inputs)
        .expect("plan Product Assembly");
    first_plan
        .publish(&original_root)
        .expect("publish Product Assembly");
    verify_product_assembly(&original_root, &manifest, &first_inputs).expect("verify assembly");
    let first_generated = tree_bytes(&original_root.join("generated"));

    // Product Assembly is a fresh publication. Delete the ignored generated
    // closure, regenerate from the authored source and compare every byte,
    // rather than only comparing its receipt.
    fs::remove_dir_all(original_root.join("generated")).expect("delete generated closure");
    fs::create_dir_all(original_root.join("generated")).expect("recreate generated root");
    let second_materialized = materialize_product(
        &original_root,
        &manifest,
        &assets,
        &toolchain,
        MaterializationLimits::default(),
    )
    .expect("regenerate authored Product Model");
    assert_eq!(
        first_materialized.compiled_composition(),
        second_materialized.compiled_composition(),
        "delete/regenerate must preserve the admitted composition bytes"
    );
    assert_eq!(
        publication_bytes(first_materialized.browser_bundle_files()),
        publication_bytes(second_materialized.browser_bundle_files()),
        "delete/regenerate must preserve the browser closure bytes"
    );
    let second_inputs = second_materialized
        .assembly_inputs()
        .expect("typed regenerated assembly inputs")
        .with_engine_dependency_path(relative_path(&generated_package, &engine_package))
        .expect("regenerated Engine dependency");
    let second_plan = plan_product_assembly(&original_root, &manifest, &second_inputs)
        .expect("plan regenerated Product Assembly");
    second_plan
        .publish(&original_root)
        .expect("publish regenerated Product Assembly");
    assert_eq!(
        first_generated,
        tree_bytes(&original_root.join("generated"))
    );

    // Relocation intentionally removes the authored source lanes before the
    // generated package is built. The generated host may only use admitted
    // include_bytes! bundle/composition resources and the direct Engine path.
    fs::create_dir_all(&relocated_root).expect("relocated root");
    copy_tree(
        &original_root.join("generated"),
        &relocated_root.join("generated"),
    );
    for lane in ["rules", "ui", "content", "rusty.toml"] {
        let path = original_root.join(lane);
        if path.is_dir() {
            fs::remove_dir_all(path).expect("remove authored lane");
        } else if path.exists() {
            fs::remove_file(path).expect("remove authored manifest");
        }
    }
    let relocated_package = relocated_root.join("generated/product-assembly");
    let target = relocated_root.join("target");
    let build = Command::new("cargo")
        .args([
            "build",
            "--quiet",
            "--manifest-path",
            relocated_package
                .join("Cargo.toml")
                .to_str()
                .expect("manifest path"),
            "--offline",
        ])
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("build relocated Product Assembly");
    assert!(
        build.success(),
        "relocated generated host build failed: {build}"
    );
    fs::remove_dir_all(relocated_root.join("generated"))
        .expect("remove generated source and bundle before runtime start");

    let binary = target.join("debug/rusty-product-rusty-product-assembly-e2e");
    let mut child = Command::new(&binary)
        .arg("--port")
        .arg("0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start relocated Product Dev Host");
    let stdout = child.stdout.take().expect("host stdout");
    let mut stdout = BufReader::new(stdout);
    let mut origin = String::new();
    stdout.read_line(&mut origin).expect("host origin");
    let origin = origin.trim().to_owned();
    assert!(
        origin.starts_with("http://127.0.0.1:"),
        "unexpected host origin: {origin}"
    );

    let index = http_request(
        &origin,
        "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert!(
        index.starts_with("HTTP/1.1 200 OK\r\n"),
        "index response: {index}"
    );
    assert!(
        index.contains("./main.js"),
        "Engine-owned bundle index: {index}"
    );
    let ui = http_request(
        &origin,
        "GET /ui/main.js HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
    );
    assert!(ui.starts_with("HTTP/1.1 200 OK\r\n"), "UI response: {ui}");
    assert!(
        ui.contains("e2e-ui"),
        "authored UI must be served from the bundle: {ui}"
    );
    assert!(
        !ui.contains("createElement(\"canvas\")") && !ui.contains("createElement('canvas')"),
        "authored UI must not create a second renderer canvas: {ui}"
    );
    let start = http_request(
        &origin,
        "POST /__rusty/product/runtime/lifecycle/start HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
    );
    assert!(
        start.starts_with("HTTP/1.1 200 OK\r\n"),
        "start response: {start}"
    );
    assert!(
        start.contains("\"accepted\":true"),
        "start response: {start}"
    );
    let demand = http_request(
        &origin,
        "POST /__rusty/product/runtime/admit-demand-step HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
    );
    assert!(
        demand.starts_with("HTTP/1.1 200 OK\r\n"),
        "demand response: {demand}"
    );
    assert!(
        demand.contains("\"accepted\":true"),
        "demand response: {demand}"
    );

    child
        .stdin
        .take()
        .expect("host stdin")
        .write_all(b"\n")
        .expect("request host shutdown");
    let status = child.wait().expect("wait relocated Product Dev Host");
    assert!(
        status.success(),
        "generated host exited unsuccessfully: {status}"
    );
    let _ = fs::remove_dir_all(&original_root);
    let _ = fs::remove_dir_all(&relocated_root);
}

/// Explicit #7262 gate: this is intentionally ignored by the ordinary
/// materializer/provider test command because it builds a generated Product
/// executable and launches real Chromium against its loopback origin.
///
/// The browser is not given a test server or intercepted responses. It loads
/// the exact generated `ProductDevHost` bundle, receives the lifecycle output
/// through EventSource, then sends real canvas keyboard input back through the
/// generated local HTTP bridge. The host is shut down through its stdin after
/// Playwright closes the browser context.
#[test]
#[ignore = "explicit #7262 generated Product Host + Chromium gate"]
fn generated_product_host_passes_real_chromium_browser_gate() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let product_root = PathBuf::from(format!("/home/dev/rusty-product-browser-e2e-{nonce}"));
    create_fixture(&product_root);
    fs::write(product_root.join("rusty.toml"), BROWSER_MANIFEST).expect("browser manifest");
    fs::create_dir_all(product_root.join("kernel")).expect("browser kernel lane");
    fs::write(
        product_root.join("kernel/entry.rs"),
        include_str!("../../../../fixtures/product-assembly/counter-kernel.rs"),
    )
    .expect("browser counter kernel");
    fs::write(
        product_root.join("rules/main.ts"),
        "import { inputAction, kernelCapability, productIntent, schedule } from '@rusty-engine/runtime-composition-authoring';\nexport default { product: 'rusty.product.assembly.e2e', capabilities: [kernelCapability('counter.increment', 'counter-increment')], intentDescriptors: [productIntent({ id: 'increment', valueKind: 'digital', capability: 'counter.increment', payload: { amount: 1 } })], inputMap: [inputAction({ id: 'increment-w', intent: 'increment', trigger: { kind: 'key', code: 'key-w', edge: 'pressed', context: 'gameplay.default' } })], schedule: schedule({}), gameplayDefinitions: [], timelines: [] };\n",
    )
    .expect("browser counter rules");
    fs::write(
        product_root.join("ui/main.ts"),
        "import type { RustyApplicationUiContext, RustyApplicationUiOwner } from '@rusty-engine/application-host';\nexport function mountProductUi(root: HTMLElement, context: RustyApplicationUiContext): RustyApplicationUiOwner { const label = document.createElement('output'); label.id = 'e2e-ui'; label.textContent = '0'; root.append(label); const unsubscribe = context.projection?.subscribe((envelope) => { label.textContent = envelope === null ? '0' : String(envelope.value); }) ?? (() => {}); return { dispose: unsubscribe }; }\n",
    )
    .expect("browser UI");

    let manifest = decode_product_manifest(BROWSER_MANIFEST).expect("browser manifest admission");
    let assets = test_assets();
    let toolchain = toolchain();
    let materialized = materialize_product(
        &product_root,
        &manifest,
        &assets,
        &toolchain,
        MaterializationLimits::default(),
    )
    .expect("materialize browser Product Model");
    let generated_package = product_root.join("generated/product-assembly");
    let engine_package = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rusty-engine");
    let inputs = materialized
        .assembly_inputs()
        .expect("typed browser assembly inputs")
        .with_engine_dependency_path(relative_path(&generated_package, &engine_package))
        .expect("relocatable Engine dependency")
        .with_kernel_dependency_path(relative_path(
            &generated_package,
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../product-kernel"),
        ))
        .expect("relocatable Product Kernel dependency");
    let capabilities = counter_capabilities();
    let plan = plan_product_assembly_with_kernel_capabilities(
        &product_root,
        &manifest,
        &inputs,
        &capabilities,
    )
    .expect("plan browser Product Assembly");
    plan.publish(&product_root)
        .expect("publish browser Product Assembly");
    verify_product_assembly_with_kernel_capabilities(
        &product_root,
        &manifest,
        &inputs,
        &capabilities,
    )
    .expect("verify browser Product Assembly");

    let target = product_root.join("target");
    let build = Command::new("cargo")
        .args([
            "build",
            "--quiet",
            "--manifest-path",
            generated_package
                .join("Cargo.toml")
                .to_str()
                .expect("generated manifest path"),
            "--offline",
        ])
        .env("CARGO_TARGET_DIR", &target)
        .status()
        .expect("build generated browser Product Assembly");
    assert!(
        build.success(),
        "generated browser host build failed: {build}"
    );

    let binary = target.join("debug/rusty-product-rusty-product-assembly-e2e");
    let mut host = Command::new(&binary)
        .arg("--port")
        .arg("0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("start generated browser ProductDevHost");
    let stdout = host.stdout.take().expect("generated host stdout");
    let mut stdout = BufReader::new(stdout);
    let mut origin = String::new();
    stdout
        .read_line(&mut origin)
        .expect("generated host origin");
    let origin = origin.trim().to_owned();
    assert!(
        origin.starts_with("http://127.0.0.1:"),
        "generated host must publish a loopback origin: {origin}"
    );

    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let playwright_config = workspace.join("render/playwright.product-assembly.config.ts");
    let playwright = Command::new("pnpm")
        .args([
            "--dir",
            workspace.join("render").to_str().expect("render path"),
            "exec",
            "playwright",
            "test",
            "--config",
            playwright_config.to_str().expect("Playwright config path"),
        ])
        .env("PLAYWRIGHT_PRODUCT_HOST_ORIGIN", &origin)
        .status()
        .expect("run explicit generated browser Product Host gate");

    // Closing the browser before asking the host to stop lets the host's SSE
    // connection observe ordinary client disposal before the process exits.
    host.stdin
        .take()
        .expect("generated host stdin")
        .write_all(b"\n")
        .expect("request generated host shutdown");
    let host_status = host.wait().expect("wait generated browser ProductDevHost");

    let _ = fs::remove_dir_all(&product_root);
    assert!(
        playwright.success(),
        "generated browser Product Host Playwright gate failed: {playwright}"
    );
    assert!(
        host_status.success(),
        "generated browser ProductDevHost exited unsuccessfully: {host_status}"
    );
}

fn create_fixture(root: &Path) {
    fs::create_dir_all(root.join("content")).expect("content");
    fs::create_dir_all(root.join("rules")).expect("rules");
    fs::create_dir_all(root.join("ui")).expect("ui");
    fs::write(root.join("rusty.toml"), MANIFEST).expect("manifest");
    fs::write(
        root.join("rules/main.ts"),
        "import type { RuntimeCompositionDraft } from '@rusty-engine/runtime-composition-authoring';\nimport { base } from './base.js';\nexport default base satisfies RuntimeCompositionDraft;\n",
    )
    .expect("rules entry");
    fs::write(
        root.join("rules/base.ts"),
        "export const base = { product: 'rusty.product.assembly.e2e', capabilities: [] };\n",
    )
    .expect("rules base");
    fs::write(
        root.join("rules/extra.ts"),
        "import { fragment, gameplayDefinition } from '@rusty-engine/runtime-composition-authoring';\nexport default fragment({ gameplayDefinitions: [gameplayDefinition('e2e', { order: 1 })] });\n",
    )
    .expect("rules fragment");
    fs::write(
        root.join("ui/main.ts"),
        "export function mountProductUi(root: Element, _context: unknown): void { const label = document.createElement('output'); label.id = 'e2e-ui'; label.textContent = 'ready'; root.append(label); }\n",
    )
    .expect("UI entry");
    fs::write(root.join("content/.keep"), []).expect("content keep");
}

fn test_assets() -> EngineAssets {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut assets = Vec::new();
    for (package, directory) in [
        (
            "@rusty-engine/runtime-composition-authoring",
            workspace.join("rules/packages/runtime-composition-authoring"),
        ),
        (
            "@rusty-engine/product-browser-host",
            workspace.join("render/artifacts/product-browser-host"),
        ),
        (
            "@rusty-engine/application-host",
            workspace.join("render/artifacts/application-host"),
        ),
    ] {
        let base = if package.ends_with("authoring") {
            directory.join("dist")
        } else {
            directory.clone()
        };
        if package.ends_with("authoring") {
            assets.push(
                EngineAsset::new(
                    package,
                    "package.json",
                    fs::read(directory.join("package.json")).expect("authoring package"),
                )
                .expect("authoring package asset"),
            );
        }
        for entry in fs::read_dir(base).expect("Engine artifact directory") {
            let entry = entry.expect("Engine artifact entry");
            let path = entry.path();
            if path.is_file()
                && (path.extension().and_then(|value| value.to_str()) == Some("js")
                    || entry.file_name().to_string_lossy().ends_with(".d.ts"))
            {
                assets.push(
                    EngineAsset::new(
                        package,
                        if package.ends_with("authoring") {
                            format!("dist/{}", entry.file_name().to_string_lossy())
                        } else {
                            entry.file_name().to_string_lossy().to_string()
                        },
                        fs::read(path).expect("Engine artifact"),
                    )
                    .expect("Engine artifact asset"),
                );
            }
        }
        if !package.ends_with("authoring") {
            assets.push(
                EngineAsset::new(
                    package,
                    "package.json",
                    fs::read(directory.join("package.json")).expect("host package"),
                )
                .expect("host package asset"),
            );
        }
    }
    EngineAssets::new(assets).expect("Engine assets")
}

fn toolchain() -> MaterializationToolchain {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    MaterializationToolchain::new(
        "/usr/bin/node",
        workspace.join("rules/node_modules/typescript/lib/typescript.js"),
        workspace.join("render/node_modules/vite/bin/vite.js"),
    )
    .with_temporary_parent(workspace.join("render"))
}

fn counter_capabilities() -> [ProductKernelCapabilityDescriptor; 1] {
    [ProductKernelCapabilityDescriptor::new(
        "counter-increment",
        CapabilityMetadata::new(
            CapabilityKind::Operation,
            CapabilityUses::INPUT_MAP,
            CapabilityAvailability::Linkable,
            CapabilityAccess::new(&[], &["counter.value"]),
            CapabilityBudget::new(4096),
            CapabilityProvenance::new("rusty.test.kernel", "kernel/entry.rs", "counter_increment"),
        ),
    )]
}

fn publication_bytes(files: &[PublicationFile]) -> BTreeMap<String, Vec<u8>> {
    files
        .iter()
        .map(|file| {
            (
                file.relative_path().as_str().to_owned(),
                file.bytes().to_vec(),
            )
        })
        .collect()
}

fn tree_bytes(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, current: &Path, output: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(current).expect("tree") {
            let entry = entry.expect("tree entry");
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, output);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("tree relative")
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/");
                output.insert(relative, fs::read(path).expect("tree bytes"));
            }
        }
    }
    let mut output = BTreeMap::new();
    visit(root, root, &mut output);
    output
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("copy destination");
    for entry in fs::read_dir(source).expect("copy source") {
        let entry = entry.expect("copy entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            fs::copy(source_path, destination_path).expect("copy file");
        }
    }
}

fn relative_path(from: &Path, to: &Path) -> String {
    let from = from.canonicalize().unwrap_or_else(|_| from.to_owned());
    let to = to.canonicalize().unwrap_or_else(|_| to.to_owned());
    let from = from.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut output = Vec::new();
    for component in &from[common..] {
        if matches!(component, Component::Normal(_)) {
            output.push("..");
        }
    }
    for component in &to[common..] {
        if let Component::Normal(value) = component {
            output.push(value.to_str().expect("UTF-8 path"));
        }
    }
    output.join("/")
}

fn http_request(origin: &str, raw: &str) -> String {
    let address = origin.trim_start_matches("http://");
    let mut stream = TcpStream::connect(address).expect("connect host");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout");
    let raw = raw.replace("Host: 127.0.0.1", &format!("Host: {address}"));
    stream.write_all(raw.as_bytes()).expect("request");
    stream.shutdown(Shutdown::Write).expect("shutdown write");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("response");
    response
}
