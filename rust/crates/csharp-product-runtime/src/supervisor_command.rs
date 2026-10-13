//! The commands `rusty dev` writes to the host it starts. On Unix the
//! supervisor reads them and forwards reloads to its runtime; on Windows the
//! host runs its product in process and reads them itself.

use std::{io::Read, path::PathBuf, sync::mpsc, thread};

/// `rusty dev` writes commands over stdin after each restage: a new Product
/// directory replaces the runtime, restaged UI or bundle content is reloaded
/// into it, and restaged live loose content files (content-root-relative) are
/// re-admitted in it. EOF is a clean stop. Browser input cannot reach it.
#[derive(Debug, serde::Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum SupervisorCommand {
    ReplaceRuntime { product_directory: PathBuf },
    ReloadAssets,
    ReloadContent { paths: Vec<String> },
}

pub(crate) const SUPERVISOR_EOF: &str = "PRODUCT_HOST_SUPERVISOR_EOF: supervisor stdin closed";

/// Reads commands from stdin on a helper thread until the first error,
/// EOF included, which it also delivers.
pub(crate) fn read_supervisor_commands() -> mpsc::Receiver<Result<SupervisorCommand, String>> {
    let (commands, received) = mpsc::sync_channel(4);
    thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            let command = read_supervisor_frame(&mut input);
            let stop = command.is_err();
            if commands.send(command).is_err() || stop {
                return;
            }
        }
    });
    received
}

/// One command: a little-endian `u32` length followed by that many JSON bytes.
fn read_supervisor_frame(input: &mut impl Read) -> Result<SupervisorCommand, String> {
    let mut prefix = [0_u8; 4];
    if let Err(error) = input.read_exact(&mut prefix) {
        return Err(if error.kind() == std::io::ErrorKind::UnexpectedEof {
            SUPERVISOR_EOF.to_owned()
        } else {
            format!("PRODUCT_HOST_SUPERVISOR_READ: {error}")
        });
    }
    let mut bytes = vec![0_u8; u32::from_le_bytes(prefix) as usize];
    input
        .read_exact(&mut bytes)
        .map_err(|_| SUPERVISOR_EOF.to_owned())?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "PRODUCT_HOST_SUPERVISOR_DECODE: command is not a closed envelope".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_frames_decode_replacement_and_report_eof() {
        let mut frame = Vec::new();
        for body in [
            br#"{"kind":"replace-runtime","productDirectory":"/tmp/product"}"#.as_slice(),
            br#"{"kind":"reload-assets"}"#.as_slice(),
            br#"{"kind":"reload-content","paths":["tuning/a.json"]}"#.as_slice(),
        ] {
            frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
            frame.extend_from_slice(body);
        }
        let mut input = frame.as_slice();
        let Ok(SupervisorCommand::ReplaceRuntime { product_directory }) =
            read_supervisor_frame(&mut input)
        else {
            panic!("expected a replacement");
        };
        assert_eq!(product_directory, PathBuf::from("/tmp/product"));
        assert!(matches!(
            read_supervisor_frame(&mut input),
            Ok(SupervisorCommand::ReloadAssets)
        ));
        let Ok(SupervisorCommand::ReloadContent { paths }) = read_supervisor_frame(&mut input)
        else {
            panic!("expected a content reload");
        };
        assert_eq!(paths, ["tuning/a.json"]);
        assert_eq!(
            read_supervisor_frame(&mut input).unwrap_err(),
            SUPERVISOR_EOF
        );
    }
}
