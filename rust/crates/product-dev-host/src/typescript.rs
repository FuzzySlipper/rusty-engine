//! The Rust-to-TypeScript wire contracts. Each browser package that reads or
//! sends these shapes has one generated file: the declarations it uses and
//! their dependencies, emitted by ts-rs from the Rust types. This test fails
//! while a checked-in file differs from what the types emit;
//! `scripts/generate-typescript-contracts.sh` rewrites them.

use std::{
    any::TypeId,
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

use ts_rs::{TypeVisitor, TS};

use crate::engine_debug::{
    ProductDevRendererInspection, ProductDevRendererStatus, ProductDevTimeAnswer,
};
use crate::frames::{header, FLAG_HELD, FLAG_VIDEO, FRAME_MAGIC};
use crate::host::{
    ProductDevConnectionBaseline, ProductDevControlClaimRequest, ProductDevControlRequest,
    ProductDevDiagnosticsReadRequest, ProductDevDiagnosticsReadResponse, ProductDevEmptyRequest,
    ProductDevErrorResponse, ProductDevExternalRequest, ProductDevInputRequest,
    ProductDevLifecycleRequest, ProductDevRealtimeRequest,
};
use crate::model::{ProductDevRuntimeOutputWire, ProductDevTimelineCompletionWire};
use crate::{
    ProductDevBrowserBootstrap, ProductDevBrowserDiagnosticsReport,
    ProductDevBrowserDiagnosticsResult, ProductDevCursorMode, ProductDevDebugCatalog,
    ProductDevFrameFormat, ProductDevInputResult, ProductDevOperationResult,
    ProductDevRenderOutput, ProductDevTimelineCompletionResult, PRODUCT_DEV_BOOTSTRAP_PATH,
    PRODUCT_DEV_FRAMES_PATH, PRODUCT_DEV_RUNTIME_BASE_PATH,
};

const HEADER: &str = "\
// Generated from the Rust wire types by product-dev-host's
// `typescript_contracts_are_current` test. Do not edit: change the Rust types
// and run scripts/generate-typescript-contracts.sh.
";

/// Every declaration reachable from the types a package names.
#[derive(Default)]
struct Contracts {
    seen: HashSet<TypeId>,
    declarations: BTreeMap<String, String>,
    constants: Vec<String>,
}

impl Contracts {
    fn with<T: TS + 'static + ?Sized>(mut self) -> Self {
        self.visit::<T>();
        self
    }

    fn constant(mut self, declaration: String) -> Self {
        self.constants.push(declaration);
        self
    }

    fn render(self) -> String {
        let mut file = HEADER.to_owned();
        for declaration in self.constants.iter().chain(self.declarations.values()) {
            file.push('\n');
            file.push_str(declaration);
            file.push('\n');
        }
        file
    }
}

impl TypeVisitor for Contracts {
    fn visit<T: TS + 'static + ?Sized>(&mut self) {
        if T::output_path().is_none() || !self.seen.insert(TypeId::of::<T>()) {
            return;
        }
        let mut declaration = T::docs().unwrap_or_default();
        declaration.push_str("export ");
        declaration.push_str(&T::decl());
        self.declarations.insert(T::ident(), declaration);
        T::visit_dependencies(self);
    }
}

fn frame_stream_constants() -> String {
    format!(
        "/** Where the page pulls the frames the runtime renders. */
export const FRAME_STREAM_PATH = {path:?};

/** The little-endian `RSF1` frame header: its magic, field offsets, formats and flags. */
export const FRAME_STREAM_HEADER = {{
  magic: {magic:#010x},
  offsets: {{ headerBytes: {header_bytes}, sequence: {sequence}, step: {step}, width: {width}, height: {height}, format: {format}, flags: {flags}, payloadBytes: {payload_bytes} }},
  bytes: {len},
  formats: {{ jpeg: {jpeg}, rgba8: {rgba8} }},
  flags: {{ held: {held}, video: {video} }},
}} as const;",
        path = PRODUCT_DEV_FRAMES_PATH,
        magic = u32::from_le_bytes(*FRAME_MAGIC),
        header_bytes = header::HEADER_BYTES,
        sequence = header::SEQUENCE,
        step = header::STEP,
        width = header::WIDTH,
        height = header::HEIGHT,
        format = header::FORMAT,
        flags = header::FLAGS,
        payload_bytes = header::PAYLOAD_BYTES,
        len = header::LEN,
        jpeg = ProductDevFrameFormat::Jpeg as u8,
        rgba8 = ProductDevFrameFormat::Rgba8 as u8,
        held = FLAG_HELD,
        video = FLAG_VIDEO,
    )
}

fn packages() -> Vec<(&'static str, String)> {
    vec![
        (
            "application-host",
            Contracts::default()
                .constant(frame_stream_constants())
                .with::<runtime_input::RuntimeInputWireEvent>()
                .with::<runtime_ui::RuntimeUiProjectionWire>()
                .with::<ProductDevRenderOutput>()
                .with::<ProductDevCursorMode>()
                .render(),
        ),
        (
            "product-browser-host",
            Contracts::default()
                .constant(format!(
                    "/** The runtime's route prefix. */\nexport const RUNTIME_BASE_PATH = {PRODUCT_DEV_RUNTIME_BASE_PATH:?};\n\n\
                     /** Where the page reads its `ProductDevBrowserBootstrap`. */\nexport const BOOTSTRAP_PATH = {PRODUCT_DEV_BOOTSTRAP_PATH:?};"
                ))
                .with::<ProductDevBrowserBootstrap>()
                .with::<ProductDevRuntimeOutputWire>()
                .with::<ProductDevConnectionBaseline>()
                .with::<ProductDevOperationResult>()
                .with::<ProductDevInputResult>()
                .with::<ProductDevTimelineCompletionWire>()
                .with::<ProductDevTimelineCompletionResult>()
                .with::<ProductDevBrowserDiagnosticsReport>()
                .with::<ProductDevBrowserDiagnosticsResult>()
                .with::<ProductDevEmptyRequest>()
                .with::<ProductDevLifecycleRequest>()
                .with::<ProductDevControlRequest>()
                .with::<ProductDevControlClaimRequest>()
                .with::<ProductDevInputRequest>()
                .with::<ProductDevRealtimeRequest>()
                .with::<ProductDevExternalRequest>()
                .with::<ProductDevDebugCatalog>()
                .with::<ProductDevRendererInspection>()
                .with::<ProductDevTimeAnswer>()
                .render(),
        ),
        (
            "live-debug-client",
            Contracts::default()
                .with::<ProductDevDebugCatalog>()
                .with::<ProductDevDiagnosticsReadRequest>()
                .with::<ProductDevDiagnosticsReadResponse>()
                .with::<ProductDevErrorResponse>()
                .with::<ProductDevRendererStatus>()
                .render(),
        ),
    ]
}

#[test]
fn typescript_contracts_are_current() {
    let render = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../render/packages");
    let write = std::env::var_os("RUSTY_WRITE_TYPESCRIPT_CONTRACTS").is_some();
    let mut stale = Vec::new();
    for (package, contents) in packages() {
        let path = render.join(package).join("src/generated/contracts.ts");
        if write {
            std::fs::create_dir_all(path.parent().expect("generated directory"))
                .expect("create the generated directory");
            std::fs::write(&path, &contents).expect("write the TypeScript contracts");
        } else if std::fs::read_to_string(&path).ok().as_deref() != Some(contents.as_str()) {
            stale.push(path);
        }
    }
    assert!(
        stale.is_empty(),
        "these TypeScript contracts differ from the Rust types: {stale:?}\n\
         run scripts/generate-typescript-contracts.sh and commit the result"
    );
}
