#![cfg(unix)]
//! A host that ends because the player closed its window or the product
//! asked to (exit 79) stops `rusty dev`, even when it ends while a restage is
//! staging the next Product.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for(path: &Path, child: &mut OwnedChild, deadline: Instant, what: &str) {
    while !path.exists() {
        assert!(Instant::now() < deadline, "{what}");
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "rusty dev exited early"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_product_stop_during_a_restage_stops_rusty_dev_without_a_new_host() {
    let root = std::env::temp_dir().join(format!("rusty-dev-product-stop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::create_dir_all(root.join("product")).unwrap();
    fs::write(root.join("product/product.json"), "{}").unwrap();
    fs::write(root.join("product.csproj"), "<Project/>").unwrap();
    fs::write(
        root.join("runtime-manifest.json"),
        r#"{"artifact":"rusty.product.runtime-pack","schemaVersion":1,"target":"linux-x64","runtime":{}}"#,
    )
    .unwrap();
    for (name, script) in [
        // Staging. During the restage the host ends (the player quits while
        // the C# build runs), and staging then succeeds.
        (
            "dotnet",
            r#"#!/bin/sh
for argument in "$@"; do
  case "$argument" in
    -getResultOutputFile:*)
      printf '{"Properties":{"RustyEngineStagedProductDirectory":"%s/product","RustyEngineWatchPaths":"%s/product.csproj"}}' \
        "$STOP_TEST_ROOT" "$STOP_TEST_ROOT" > "${argument#-getResultOutputFile:}";;
  esac
done
if [ -e "$STOP_TEST_ROOT/restaging" ]; then
  touch "$STOP_TEST_ROOT/quit"
  while [ ! -e "$STOP_TEST_ROOT/exited" ]; do sleep 0.02; done
  sleep 0.2
fi
"#,
        ),
        // A host that counts its starts and ends with the product-stopped
        // code when told to.
        (
            "rusty-product-host",
            r#"#!/bin/sh
if [ "$1" = "--identity" ]; then echo '{}'; exit 0; fi
echo started >> "$STOP_TEST_ROOT/starts"
while [ ! -e "$STOP_TEST_ROOT/quit" ]; do sleep 0.02; done
touch "$STOP_TEST_ROOT/exited"
exit 79
"#,
        ),
    ] {
        let path = root.join("bin").join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut child = OwnedChild(
        Command::new(env!("CARGO_BIN_EXE_rusty"))
            .args(["dev", "--project"])
            .arg(root.join("product.csproj"))
            .arg("--runtime")
            .arg(&root)
            .env("STOP_TEST_ROOT", &root)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    root.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    wait_for(
        &root.join("starts"),
        &mut child,
        deadline,
        "the host did not start",
    );
    // Edit the project: a C# restage, during which the host ends.
    fs::write(root.join("restaging"), "").unwrap();
    fs::write(
        root.join("product.csproj"),
        "<Project><!-- edited --></Project>",
    )
    .unwrap();
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "rusty dev kept running after the product stopped"
        );
        thread::sleep(Duration::from_millis(20));
    };
    let mut output = String::new();
    std::io::Read::read_to_string(child.0.stdout.as_mut().unwrap(), &mut output).unwrap();
    assert!(status.success(), "{status}\n{output}");
    assert!(output.contains(r#""reason":"product-stopped""#), "{output}");
    assert!(!output.contains("child-exited-during-restage"), "{output}");
    assert_eq!(
        fs::read_to_string(root.join("starts")).unwrap(),
        "started\n",
        "a new host started after the product stopped"
    );
    fs::remove_dir_all(root).unwrap();
}
