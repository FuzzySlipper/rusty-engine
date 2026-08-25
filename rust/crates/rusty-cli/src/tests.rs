use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    args::{Command, Invocation, OutputFormat},
    execute, EXIT_CONFORMANCE, EXIT_ROOT,
};

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("rusty-cli-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn init(root: &Path) -> crate::Execution {
    execute(
        Invocation {
            command: Command::Init {
                target: root.to_path_buf(),
                product_id: Some("test.product".into()),
            },
            format: OutputFormat::Human,
        },
        root.parent().unwrap(),
    )
}

#[test]
fn init_is_atomic_idempotent_and_refuses_conflicts() {
    let parent = TempDir::new("init");
    let product = parent.path().join("product");
    assert_eq!(init(&product).exit_code, 0);
    assert_eq!(init(&product).exit_code, 0);
    fs::write(product.join("ui/main.ts"), "changed").unwrap();
    let conflicted = init(&product);
    assert_eq!(conflicted.exit_code, EXIT_CONFORMANCE);
    assert_eq!(
        fs::read_to_string(product.join("ui/main.ts")).unwrap(),
        "changed"
    );

    let occupied = parent.path().join("occupied");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("user.txt"), "preserve").unwrap();
    let before = fs::read_to_string(occupied.join("user.txt")).unwrap();
    assert_eq!(init(&occupied).exit_code, EXIT_CONFORMANCE);
    assert_eq!(
        fs::read_to_string(occupied.join("user.txt")).unwrap(),
        before
    );

    let empty = parent.path().join("empty");
    fs::create_dir(&empty).unwrap();
    assert_eq!(init(&empty).exit_code, 0);
    assert!(empty.join("rusty.toml").is_file());
}

#[test]
#[cfg(unix)]
fn repeated_init_rejects_symlinked_expected_files_and_directories() {
    let parent = TempDir::new("init-symlink");
    let product = parent.path().join("product");
    init(&product);
    let expected_rules = fs::read(product.join("rules/main.ts")).unwrap();
    let external = parent.path().join("external.ts");
    fs::write(&external, &expected_rules).unwrap();
    fs::remove_file(product.join("rules/main.ts")).unwrap();
    std::os::unix::fs::symlink(&external, product.join("rules/main.ts")).unwrap();
    assert_eq!(init(&product).exit_code, EXIT_CONFORMANCE);

    fs::remove_file(product.join("rules/main.ts")).unwrap();
    fs::write(product.join("rules/main.ts"), expected_rules).unwrap();
    let external_content = parent.path().join("external-content");
    fs::create_dir(&external_content).unwrap();
    fs::remove_dir_all(product.join("content")).unwrap();
    std::os::unix::fs::symlink(&external_content, product.join("content")).unwrap();
    assert_eq!(init(&product).exit_code, EXIT_CONFORMANCE);
}

#[test]
#[cfg(unix)]
fn init_rejects_a_symlinked_intermediate_parent_before_publication() {
    let parent = TempDir::new("init-parent-alias");
    let real = parent.path().join("real");
    fs::create_dir(&real).unwrap();
    let alias = parent.path().join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let result = init(&alias.join("product"));
    assert_eq!(result.exit_code, EXIT_CONFORMANCE);
    assert_eq!(result.report.diagnostics[0].code, "RUSTY_INIT_PARENT_ALIAS");
    assert!(!real.join("product").exists());
}

#[test]
#[ignore = "requires prepared Rules and renderer artifacts; scripts/verify-product-conformance.sh owns this integration proof"]
fn check_discovers_nested_product_and_reports_missing_root() {
    let parent = TempDir::new("discovery");
    let product = parent.path().join("product");
    init(&product);
    let nested = product.join("rules/nested/deeper");
    fs::create_dir_all(&nested).unwrap();
    let checked = execute(
        Invocation {
            command: Command::Check { start: nested },
            format: OutputFormat::Json,
        },
        parent.path(),
    );
    assert_eq!(checked.exit_code, 0);
    assert_eq!(checked.report.status, "ok");
    let missing = execute(
        Invocation {
            command: Command::Check {
                start: parent.path().to_path_buf(),
            },
            format: OutputFormat::Human,
        },
        parent.path(),
    );
    assert_eq!(missing.exit_code, EXIT_ROOT);
    assert_eq!(missing.report.diagnostics[0].code, "RUSTY_ROOT_NOT_FOUND");
}

#[test]
fn check_reports_malformed_layout_host_additions_and_generated_escapes() {
    let parent = TempDir::new("layout");
    let product = parent.path().join("product");
    init(&product);
    fs::remove_file(product.join("ui/main.ts")).unwrap();
    fs::write(product.join("vite.config.mts"), "export {};").unwrap();
    fs::create_dir(product.join("rules/dev")).unwrap();
    fs::write(product.join("rules/dev/index.html"), "<!doctype html>").unwrap();
    let outside = parent.path().join("outside");
    fs::create_dir(&outside).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, product.join("generated/product-assembly")).unwrap();
    let checked = execute(
        Invocation {
            command: Command::Check {
                start: product.clone(),
            },
            format: OutputFormat::Human,
        },
        parent.path(),
    );
    assert_eq!(checked.exit_code, EXIT_CONFORMANCE);
    let codes = checked
        .report
        .diagnostics
        .iter()
        .map(|item| item.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&"RUSTY_LAYOUT_ENTRYPOINT_MISSING"));
    assert!(codes.contains(&"RUSTY_PROHIBITED_HOST_PATH"));
    #[cfg(unix)]
    assert!(codes.contains(&"RUSTY_OUTPUT_OUTSIDE_GENERATED"));
}

#[test]
#[cfg(unix)]
fn check_rejects_a_declared_entrypoint_symlink_that_escapes_the_product() {
    let parent = TempDir::new("source-symlink");
    let product = parent.path().join("product");
    init(&product);
    let outside = parent.path().join("outside.ts");
    fs::write(&outside, "export {};").unwrap();
    fs::remove_file(product.join("ui/main.ts")).unwrap();
    std::os::unix::fs::symlink(&outside, product.join("ui/main.ts")).unwrap();
    let checked = execute(
        Invocation {
            command: Command::Check {
                start: product.clone(),
            },
            format: OutputFormat::Human,
        },
        parent.path(),
    );
    assert_eq!(checked.exit_code, EXIT_CONFORMANCE);
    assert!(checked
        .report
        .diagnostics
        .iter()
        .any(|item| item.code == "RUSTY_PATH_SYMLINK_ESCAPE"));
}

#[test]
#[cfg(unix)]
fn check_rejects_intermediate_and_dangling_symlink_escapes() {
    let parent = TempDir::new("intermediate-symlink");
    let product = parent.path().join("product");
    init(&product);
    let outside = parent.path().join("outside-rules");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("main.ts"), "export {};\n").unwrap();
    fs::remove_dir_all(product.join("rules")).unwrap();
    std::os::unix::fs::symlink(&outside, product.join("rules")).unwrap();
    std::os::unix::fs::symlink(
        parent.path().join("missing"),
        product.join("generated/product-assembly"),
    )
    .unwrap();
    let checked = execute(
        Invocation {
            command: Command::Check { start: product },
            format: OutputFormat::Human,
        },
        parent.path(),
    );
    assert_eq!(checked.exit_code, EXIT_CONFORMANCE);
    let codes = checked
        .report
        .diagnostics
        .iter()
        .map(|item| item.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&"RUSTY_PATH_SYMLINK_ESCAPE"));
    assert!(codes.contains(&"RUSTY_OUTPUT_SYMLINK_READ"));
}

#[test]
#[cfg(unix)]
fn check_requires_a_real_generated_lane_even_without_declared_outputs() {
    let parent = TempDir::new("generated-lane");
    let product = parent.path().join("product");
    init(&product);
    fs::remove_dir_all(product.join("generated")).unwrap();
    std::os::unix::fs::symlink(product.join("content"), product.join("generated")).unwrap();
    let checked = execute(
        Invocation {
            command: Command::Check { start: product },
            format: OutputFormat::Human,
        },
        parent.path(),
    );
    assert_eq!(checked.exit_code, EXIT_CONFORMANCE);
    assert!(checked
        .report
        .diagnostics
        .iter()
        .any(|item| item.code == "RUSTY_OUTPUT_GENERATED_LANE"));
}

#[test]
fn check_enforces_entrypoint_extensions_and_generated_output_kinds() {
    let parent = TempDir::new("output-kinds");
    let product = parent.path().join("product");
    init(&product);
    let manifest = fs::read_to_string(product.join("rusty.toml"))
        .unwrap()
        .replace("rules/main.ts", "rules/main.js");
    fs::write(product.join("rusty.toml"), manifest).unwrap();
    fs::write(product.join("rules/main.js"), "export {};\n").unwrap();
    fs::create_dir(product.join("generated/compiled-composition.json")).unwrap();
    fs::write(product.join("generated/product-bundle"), "not a directory").unwrap();
    let checked = execute(
        Invocation {
            command: Command::Check { start: product },
            format: OutputFormat::Human,
        },
        parent.path(),
    );
    assert_eq!(checked.exit_code, EXIT_CONFORMANCE);
    let codes = checked
        .report
        .diagnostics
        .iter()
        .map(|item| item.code)
        .collect::<Vec<_>>();
    assert!(codes.contains(&"RUSTY_LAYOUT_RULES_EXTENSION"));
    assert_eq!(
        codes
            .iter()
            .filter(|code| **code == "RUSTY_OUTPUT_KIND")
            .count(),
        2
    );
}

#[test]
fn check_rejects_a_malformed_manifest_without_reimplementing_schema_validation() {
    let parent = TempDir::new("manifest");
    let product = parent.path().join("product");
    init(&product);
    fs::write(product.join("rusty.toml"), "not = [valid").unwrap();
    let checked = execute(
        Invocation {
            command: Command::Check {
                start: product.clone(),
            },
            format: OutputFormat::Human,
        },
        parent.path(),
    );
    assert_eq!(checked.exit_code, EXIT_CONFORMANCE);
    assert_eq!(checked.report.diagnostics[0].code, "RUSTY_MANIFEST_INVALID");
}

#[test]
#[cfg(unix)]
fn discovery_rejects_a_symlinked_manifest_before_accepting_the_root() {
    let parent = TempDir::new("manifest-symlink");
    let product = parent.path().join("product");
    init(&product);
    let external_manifest = parent.path().join("external-rusty.toml");
    fs::rename(product.join("rusty.toml"), &external_manifest).unwrap();
    std::os::unix::fs::symlink(&external_manifest, product.join("rusty.toml")).unwrap();
    let checked = execute(
        Invocation {
            command: Command::Check { start: product },
            format: OutputFormat::Human,
        },
        parent.path(),
    );
    assert_eq!(checked.exit_code, EXIT_ROOT);
    assert_eq!(checked.report.diagnostics[0].code, "RUSTY_MANIFEST_SYMLINK");
}

#[test]
#[ignore = "requires prepared Rules and renderer artifacts; scripts/verify-product-conformance.sh owns this integration proof"]
fn doctor_reports_the_complete_product_workflow_without_a_wrapper_claim() {
    let parent = TempDir::new("doctor");
    let product = parent.path().join("product");
    init(&product);
    let doctor = execute(
        Invocation {
            command: Command::Doctor {
                start: product.clone(),
            },
            format: OutputFormat::Json,
        },
        parent.path(),
    );
    assert_eq!(doctor.exit_code, 0);
    assert_eq!(doctor.report.status, "ok");
    assert!(doctor
        .report
        .facts
        .iter()
        .any(|fact| fact.path == "doctor.desktopWrapper"));
    let encoded = serde_json::to_string(&doctor.report).unwrap();
    assert_eq!(encoded, serde_json::to_string(&doctor.report).unwrap());
    assert!(encoded.len() < 32 * 1024);
}

#[test]
fn init_failure_does_not_publish_any_partial_product() {
    let parent = TempDir::new("failure");
    let product = parent.path().join("product");
    let failed = execute(
        Invocation {
            command: Command::Init {
                target: product.clone(),
                product_id: Some("Invalid ID".into()),
            },
            format: OutputFormat::Human,
        },
        parent.path(),
    );
    assert_eq!(failed.exit_code, crate::EXIT_USAGE);
    assert!(!product.exists());
}

#[test]
fn serialized_reports_remain_bounded_under_many_maximum_diagnostics() {
    let escaped = "\u{0001}".repeat(2_000);
    let report = crate::report::Report::checked(
        (0..64)
            .map(|index| {
                crate::report::Diagnostic::error(
                    "RUSTY_TEST",
                    escaped.clone(),
                    format!("{index}: {escaped}"),
                )
            })
            .collect(),
    );
    assert!(
        serde_json::to_vec(&report).unwrap().len() < crate::report::MAX_SERIALIZED_REPORT_BYTES
    );
    assert!(report.diagnostics.len() <= crate::report::MAX_DIAGNOSTICS);
}
