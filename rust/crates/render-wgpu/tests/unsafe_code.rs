//! Unsafe code stays in the ray scene module (Patch's decision on #10308):
//! the crate denies it, only `src/ray_scene.rs` allows it, and every unsafe
//! block there states its invariant in a `SAFETY:` comment.

use std::path::{Path, PathBuf};

/// The one file allowed unsafe code.
const ALLOWED: &str = "src/ray_scene.rs";

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read the crate's sources") {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

/// The file's lines with `//` comments removed.
fn code(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| match line.find("//") {
            Some(start) => line[..start].to_owned(),
            None => line.to_owned(),
        })
        .collect()
}

fn unsafe_lines(lines: &[String]) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .any(|word| word == "unsafe" || word == "unsafe_code")
        })
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn unsafe_code_is_only_in_the_ray_scene_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    let mut outside = Vec::new();
    for file in &files {
        let relative = file
            .strip_prefix(root)
            .expect("under the crate")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == ALLOWED {
            continue;
        }
        let text = std::fs::read_to_string(file).expect("read a source");
        let lines = code(&text);
        for index in unsafe_lines(&lines) {
            // The crate root denies unsafe code; that is the one mention.
            if relative == "src/lib.rs" && lines[index].trim() == "#![deny(unsafe_code)]" {
                continue;
            }
            outside.push(format!("{relative}:{}: {}", index + 1, lines[index].trim()));
        }
    }
    assert!(
        outside.is_empty(),
        "unsafe code outside {ALLOWED}:\n{}",
        outside.join("\n")
    );
    let lib = std::fs::read_to_string(root.join("src/lib.rs")).expect("read lib.rs");
    assert!(lib
        .lines()
        .any(|line| line.trim() == "#![deny(unsafe_code)]"));
}

#[test]
fn every_unsafe_block_in_the_ray_scene_states_its_invariant() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(root.join(ALLOWED)).expect("read the ray scene");
    let raw: Vec<&str> = text.lines().collect();
    let lines = code(&text);
    let mut unexplained = Vec::new();
    for index in unsafe_lines(&lines) {
        let line = lines[index].trim();
        if line == "#![allow(unsafe_code)]" || !line.contains("unsafe {") {
            continue;
        }
        // The comment closest above the block, within its statement.
        let explained = raw[..index]
            .iter()
            .rev()
            .take_while(|above| {
                let above = above.trim();
                above.starts_with("//") || !above.ends_with(';')
            })
            .any(|above| above.trim().starts_with("// SAFETY:"));
        if !explained {
            unexplained.push(format!("{ALLOWED}:{}: {line}", index + 1));
        }
    }
    assert!(
        unexplained.is_empty(),
        "unsafe blocks without a SAFETY comment:\n{}",
        unexplained.join("\n")
    );
}
