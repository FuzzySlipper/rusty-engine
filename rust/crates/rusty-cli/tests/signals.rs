#![cfg(unix)]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
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

#[test]
fn termination_signals_close_supervision_and_wait_for_disposal() {
    for signal in ["INT", "TERM"] {
        let root =
            std::env::temp_dir().join(format!("rusty-dev-signal-{}-{signal}", std::process::id()));
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("product")).unwrap();
        fs::write(root.join("product/product.json"), "{}").unwrap();
        fs::write(root.join("product.csproj"), "<Project/>").unwrap();
        fs::write(root.join("runtime-manifest.json"),
            r#"{"artifact":"rusty.product.runtime-pack","schemaVersion":1,"target":"linux-x64","runtime":{}}"#).unwrap();
        for (name, script) in [
            (
                "dotnet",
                r#"#!/bin/sh
case "$*" in
 *RustyEngineStagedProductDirectory*) echo "$SIGNAL_TEST_ROOT/product";;
 *RustyEngineWatchPaths*) echo "$SIGNAL_TEST_ROOT/product.csproj";;
esac
"#,
            ),
            (
                "rusty-product-host",
                r#"#!/bin/sh
if [ "$1" = "--identity" ]; then echo '{}'; exit 0; fi
echo ready > "$SIGNAL_TEST_ROOT/ready"
cat >/dev/null
sleep 0.1
echo disposed > "$SIGNAL_TEST_ROOT/disposed"
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
                .env("SIGNAL_TEST_ROOT", &root)
                .env(
                    "PATH",
                    format!(
                        "{}:{}",
                        root.join("bin").display(),
                        std::env::var("PATH").unwrap()
                    ),
                )
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while !root.join("ready").exists() {
            assert!(Instant::now() < deadline, "supervised host did not start");
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "supervisor exited early"
            );
            thread::sleep(Duration::from_millis(20));
        }
        assert!(Command::new("kill")
            .args([format!("-{signal}"), child.0.id().to_string()])
            .status()
            .unwrap()
            .success());
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success(), "{signal}: {status}");
                break;
            }
            assert!(Instant::now() < deadline, "supervisor ignored {signal}");
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            fs::read_to_string(root.join("disposed")).unwrap(),
            "disposed\n"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
