use std::{
    borrow::Cow,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{is_relative_path, Error};

const MAGIC: [u8; 8] = *b"RUSTYPAK";
const HEADER_LEN: u64 = 32;
/// Every entry starts on this boundary, so a mapped slice can be viewed as
/// any `f32`/`u32`/SIMD array without copying.
const ALIGNMENT: u64 = 64;

/// A compressed entry must save at least this fraction of its length, or it
/// is stored raw.
const MIN_COMPRESSION_SAVING: f64 = 0.1;

/// One file in the inventory. `byteLength` and `sha256` describe the file
/// itself, compressed or not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub path: String,
    pub byte_length: u64,
    pub sha256: String,
    pub offset: u64,
    /// The bundle this file belongs to, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
    /// Present when the file is stored as zstd: the stored length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zstd_length: Option<u64>,
}

impl Entry {
    /// The bytes the entry occupies in the container.
    pub fn stored_length(&self) -> u64 {
        self.zstd_length.unwrap_or(self.byte_length)
    }
}

/// A content bundle: its ID and its root, a path prefix within the container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub id: String,
    pub root: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    entries: Vec<Entry>,
    #[serde(default)]
    bundles: Vec<Bundle>,
}

/// An open container: the read-only file mapping and its checked inventory.
pub struct Container {
    path: PathBuf,
    map: Mmap,
    entries: Vec<Entry>,
    bundles: Vec<Bundle>,
}

impl std::fmt::Debug for Container {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Container")
            .field("path", &self.path)
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl Container {
    /// Checks the header, the inventory and every entry's range, then maps the
    /// file. Nothing is hashed: the recorded SHA-256 is each file's identity.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let mut file = File::open(path).map_err(Error::io(path))?;
        let actual = file.metadata().map_err(Error::io(path))?.len();
        let mut header = [0_u8; HEADER_LEN as usize];
        let read = read_prefix(&mut file, &mut header).map_err(Error::io(path))?;
        if read < MAGIC.len() || header[..MAGIC.len()] != MAGIC {
            return Err(Error::NotAContainer(path.to_owned()));
        }
        if read < header.len() {
            return Err(Error::Truncated {
                path: path.to_owned(),
                expected: HEADER_LEN,
                actual,
            });
        }
        let field = |index: usize| {
            u64::from_le_bytes(header[8 + index * 8..16 + index * 8].try_into().unwrap())
        };
        let (inventory_offset, inventory_len, file_len) = (field(0), field(1), field(2));
        let corrupt = |detail: String| Error::Corrupt {
            path: path.to_owned(),
            detail,
        };
        if actual < file_len {
            return Err(Error::Truncated {
                path: path.to_owned(),
                expected: file_len,
                actual,
            });
        }
        if actual > file_len {
            return Err(corrupt(format!(
                "{actual} bytes, but the header records {file_len}"
            )));
        }
        let inventory_end = inventory_offset
            .checked_add(inventory_len)
            .filter(|&end| inventory_offset >= HEADER_LEN && end <= file_len)
            .ok_or_else(|| corrupt("the inventory lies outside the file".to_owned()))?;
        // SAFETY: the map is read-only. A container is a release output that
        // is replaced by rename and never written in place, so the bytes
        // behind the map do not change while it is alive.
        #[allow(unsafe_code)]
        let map = unsafe { Mmap::map(&file) }.map_err(Error::io(path))?;
        let inventory: Inventory =
            serde_json::from_slice(&map[inventory_offset as usize..inventory_end as usize])
                .map_err(|error| corrupt(format!("invalid inventory: {error}")))?;
        check(&inventory, inventory_offset).map_err(corrupt)?;
        Ok(Self {
            path: path.to_owned(),
            map,
            entries: inventory.entries,
            bundles: inventory.bundles,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every entry, sorted by path.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn bundles(&self) -> &[Bundle] {
        &self.bundles
    }

    pub fn entry(&self, path: &str) -> Option<&Entry> {
        self.entries
            .binary_search_by(|entry| entry.path.as_str().cmp(path))
            .ok()
            .map(|index| &self.entries[index])
    }

    /// The entry's file bytes: borrowed from the map when stored raw,
    /// decompressed when stored as zstd.
    pub fn read(&self, entry: &Entry) -> Result<Cow<'_, [u8]>, Error> {
        let stored =
            &self.map[entry.offset as usize..(entry.offset + entry.stored_length()) as usize];
        if entry.zstd_length.is_none() {
            return Ok(Cow::Borrowed(stored));
        }
        let corrupt = |detail: String| Error::Corrupt {
            path: self.path.clone(),
            detail: format!("entry `{}`: {detail}", entry.path),
        };
        let bytes = zstd::bulk::decompress(stored, entry.byte_length as usize)
            .map_err(|error| corrupt(error.to_string()))?;
        // The output buffer is sized from the inventory; a short stream would
        // otherwise pass for the file.
        if bytes.len() as u64 != entry.byte_length {
            return Err(corrupt(format!("decompressed to {} bytes", bytes.len())));
        }
        Ok(Cow::Owned(bytes))
    }

    pub fn get(&self, path: &str) -> Result<Cow<'_, [u8]>, Error> {
        let entry = self
            .entry(path)
            .ok_or_else(|| Error::Missing(path.to_owned()))?;
        self.read(entry)
    }
}

fn read_prefix(file: &mut File, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut read = 0;
    while read < buffer.len() {
        match file.read(&mut buffer[read..])? {
            0 => break,
            count => read += count,
        }
    }
    Ok(read)
}

/// Paths are relative, unique and sorted; each entry is aligned, in range and
/// after the previous one; each bundle member names a declared bundle and lies
/// under its root.
fn check(inventory: &Inventory, data_end: u64) -> Result<(), String> {
    for (index, bundle) in inventory.bundles.iter().enumerate() {
        if !is_relative_path(&bundle.id)
            || bundle.id.contains('/')
            || !is_relative_path(&bundle.root)
            || inventory.bundles[..index].iter().any(|b| b.id == bundle.id)
        {
            return Err(format!("invalid bundle `{}`", bundle.id));
        }
    }
    let mut previous: Option<&Entry> = None;
    for entry in &inventory.entries {
        let path = &entry.path;
        if !is_relative_path(path) {
            return Err(format!("invalid entry path `{path}`"));
        }
        if previous.is_some_and(|p| p.path >= *path) {
            return Err(format!("entry `{path}` is duplicated or out of order"));
        }
        if entry.sha256.len() != 64
            || !entry
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(format!("entry `{path}` has an invalid SHA-256"));
        }
        if entry.offset % ALIGNMENT != 0 {
            return Err(format!("entry `{path}` is not {ALIGNMENT}-byte aligned"));
        }
        let start = previous.map_or(HEADER_LEN, |p| p.offset + p.stored_length());
        let end = entry.offset.checked_add(entry.stored_length());
        if entry.offset < start || end.is_none_or(|end| end > data_end) {
            return Err(format!(
                "entry `{path}` overlaps another or lies outside the file"
            ));
        }
        if let Some(id) = &entry.bundle {
            let bundle = inventory.bundles.iter().find(|b| &b.id == id);
            if !bundle.is_some_and(|b| path.starts_with(&format!("{}/", b.root))) {
                return Err(format!("entry `{path}` is not under bundle `{id}`"));
            }
        }
        previous = Some(entry);
    }
    Ok(())
}

/// Where a new entry's bytes come from.
pub enum Body<'a> {
    File(PathBuf),
    Bytes(&'a [u8]),
}

pub struct NewEntry<'a> {
    pub path: String,
    pub bundle: Option<String>,
    pub body: Body<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct WriteReport {
    pub bytes: u64,
    pub entries: usize,
}

/// Writes a container, hashing each body while copying it. With `compress`,
/// a body that zstd shrinks by at least `MIN_COMPRESSION_SAVING` is stored
/// compressed; the rest stay raw. The file appears at `out` by rename once
/// complete.
pub fn write(
    out: &Path,
    mut entries: Vec<NewEntry<'_>>,
    bundles: Vec<Bundle>,
    compress: bool,
) -> Result<WriteReport, Error> {
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    for (index, entry) in entries.iter().enumerate() {
        if !is_relative_path(&entry.path) || index > 0 && entries[index - 1].path == entry.path {
            return Err(Error::InvalidPath(entry.path.clone()));
        }
    }
    let name = out
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::InvalidPath(out.display().to_string()))?;
    let pending = out.with_file_name(format!(".{name}.rusty-pending"));
    let _ = fs::remove_file(&pending);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .map_err(Error::io(&pending))?;
        let io = Error::io(&pending);
        let written = write_body(&mut file, entries, bundles, compress);
        let (inventory_offset, inventory_len, file_len, count) = written.map_err(io)?;
        let io = Error::io(&pending);
        (|| {
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&MAGIC)?;
            for value in [inventory_offset, inventory_len, file_len] {
                file.write_all(&value.to_le_bytes())?;
            }
            file.sync_all()
        })()
        .map_err(io)?;
        fs::rename(&pending, out).map_err(Error::io(out))?;
        Ok(WriteReport {
            bytes: file_len,
            entries: count,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&pending);
    }
    result
}

fn write_body(
    file: &mut File,
    entries: Vec<NewEntry<'_>>,
    bundles: Vec<Bundle>,
    compress: bool,
) -> std::io::Result<(u64, u64, u64, usize)> {
    let mut out = std::io::BufWriter::with_capacity(1 << 20, file);
    out.write_all(&[0; HEADER_LEN as usize])?;
    let mut position = HEADER_LEN;
    let mut inventory = Inventory {
        entries: Vec::with_capacity(entries.len()),
        bundles,
    };
    let mut buffer = vec![0_u8; 1 << 20];
    for entry in entries {
        let padding = position.next_multiple_of(ALIGNMENT) - position;
        out.write_all(&[0; ALIGNMENT as usize][..padding as usize])?;
        let offset = position + padding;
        let (length, sha256, zstd_length) = if compress {
            let raw = match entry.body {
                Body::Bytes(bytes) => Cow::Borrowed(bytes),
                Body::File(path) => Cow::Owned(fs::read(path)?),
            };
            let packed = zstd::bulk::compress(&raw, zstd::DEFAULT_COMPRESSION_LEVEL)?;
            let worth = (packed.len() as f64) <= raw.len() as f64 * (1.0 - MIN_COMPRESSION_SAVING);
            out.write_all(if worth { &packed } else { &raw })?;
            (
                raw.len() as u64,
                format!("{:x}", Sha256::digest(&raw)),
                worth.then_some(packed.len() as u64),
            )
        } else {
            let mut hash = Sha256::new();
            let mut length = 0_u64;
            let mut copy = |bytes: &[u8]| -> std::io::Result<()> {
                hash.update(bytes);
                length += bytes.len() as u64;
                out.write_all(bytes)
            };
            match entry.body {
                Body::Bytes(bytes) => copy(bytes)?,
                Body::File(path) => {
                    let mut source = File::open(&path)?;
                    loop {
                        match source.read(&mut buffer)? {
                            0 => break,
                            count => copy(&buffer[..count])?,
                        }
                    }
                }
            }
            (length, format!("{:x}", hash.finalize()), None)
        };
        position = offset + zstd_length.unwrap_or(length);
        inventory.entries.push(Entry {
            path: entry.path,
            byte_length: length,
            sha256,
            offset,
            bundle: entry.bundle,
            zstd_length,
        });
    }
    let json = serde_json::to_vec(&inventory).map_err(std::io::Error::other)?;
    out.write_all(&json)?;
    out.flush()?;
    let inventory_len = json.len() as u64;
    Ok((
        position,
        inventory_len,
        position + inventory_len,
        inventory.entries.len(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(directory: &Path) -> PathBuf {
        fixture_with(directory, false)
    }

    fn fixture_with(directory: &Path, compress: bool) -> PathBuf {
        let body = directory.join("body.bin");
        fs::write(&body, vec![7_u8; 100_000]).unwrap();
        let out = directory.join("product.rpak");
        write(
            &out,
            vec![
                NewEntry {
                    path: "content/rules/a.json".into(),
                    bundle: Some("rules".into()),
                    body: Body::Bytes(b"{\"id\":1}"),
                },
                NewEntry {
                    path: "product.json".into(),
                    bundle: None,
                    body: Body::Bytes(b"{}"),
                },
                NewEntry {
                    path: "content/body.bin".into(),
                    bundle: None,
                    body: Body::File(body),
                },
            ],
            vec![Bundle {
                id: "rules".into(),
                root: "content/rules".into(),
            }],
            compress,
        )
        .unwrap();
        out
    }

    #[test]
    fn round_trip_borrows_raw_entries_and_decompresses_zstd_ones() {
        for compress in [false, true] {
            round_trip(compress);
        }
    }

    fn round_trip(compress: bool) {
        let directory = tempfile::tempdir().unwrap();
        let container = Container::open(&fixture_with(directory.path(), compress)).unwrap();
        let paths: Vec<_> = container
            .entries()
            .iter()
            .map(|e| e.path.as_str())
            .collect();
        assert_eq!(
            paths,
            ["content/body.bin", "content/rules/a.json", "product.json"]
        );
        assert_eq!(
            container.get("content/rules/a.json").unwrap().as_ref(),
            b"{\"id\":1}"
        );
        assert_eq!(
            container.get("content/body.bin").unwrap().as_ref(),
            &[7; 100_000][..]
        );
        for entry in container.entries() {
            assert_eq!(entry.offset % ALIGNMENT, 0);
            let bytes = container.read(entry).unwrap();
            assert_eq!(entry.sha256, format!("{:x}", Sha256::digest(&bytes)));
            // Only a body zstd shrinks enough is stored compressed.
            let shrinks = compress && entry.path == "content/body.bin";
            assert_eq!(entry.zstd_length.is_some(), shrinks, "{}", entry.path);
            assert_eq!(matches!(bytes, Cow::Borrowed(_)), !shrinks);
        }
        let rules = container.entry("content/rules/a.json").unwrap();
        assert_eq!(rules.bundle.as_deref(), Some("rules"));
        assert!(matches!(
            container.get("content/missing"),
            Err(Error::Missing(_))
        ));
    }

    #[test]
    fn a_damaged_zstd_entry_fails_its_read() {
        let directory = tempfile::tempdir().unwrap();
        let path = fixture_with(directory.path(), true);
        let entry = Container::open(&path)
            .unwrap()
            .entry("content/body.bin")
            .unwrap()
            .clone();
        let mut bytes = fs::read(&path).unwrap();
        bytes[entry.offset as usize..][..8].copy_from_slice(b"notzstd!");
        fs::write(&path, bytes).unwrap();
        let container = Container::open(&path).unwrap();
        assert!(matches!(container.read(&entry), Err(Error::Corrupt { .. })));
    }

    fn damaged(edit: impl FnOnce(&mut Vec<u8>)) -> Error {
        let directory = tempfile::tempdir().unwrap();
        let path = fixture(directory.path());
        let mut bytes = fs::read(&path).unwrap();
        edit(&mut bytes);
        fs::write(&path, bytes).unwrap();
        Container::open(&path).unwrap_err()
    }

    fn set_header(bytes: &mut [u8], index: usize, value: u64) {
        bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
    }

    fn inventory_offset(bytes: &[u8]) -> usize {
        u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize
    }

    /// Rewrites the inventory JSON in place, fixing up the header lengths.
    fn with_inventory(edit: impl FnOnce(&mut serde_json::Value)) -> Error {
        damaged(|bytes| {
            let offset = inventory_offset(bytes);
            let mut inventory: serde_json::Value =
                serde_json::from_slice(&bytes[offset..]).unwrap();
            edit(&mut inventory);
            bytes.truncate(offset);
            bytes.extend(serde_json::to_vec(&inventory).unwrap());
            let (len, total) = ((bytes.len() - offset) as u64, bytes.len() as u64);
            set_header(bytes, 1, len);
            set_header(bytes, 2, total);
        })
    }

    #[test]
    fn damage_fails_at_open_with_a_named_error() {
        assert!(matches!(
            damaged(|bytes| bytes[0] = b'X'),
            Error::NotAContainer(_)
        ));
        assert!(matches!(
            damaged(|bytes| bytes.truncate(20)),
            Error::Truncated { expected: 32, .. }
        ));
        assert!(matches!(
            damaged(|bytes| bytes.truncate(bytes.len() - 1)),
            Error::Truncated { .. }
        ));
        assert!(matches!(
            damaged(|bytes| bytes.push(0)),
            Error::Corrupt { .. }
        ));
        let corrupt = |error: Error, detail: &str| match error {
            Error::Corrupt { detail: found, .. } => {
                assert!(found.contains(detail), "{found}")
            }
            other => panic!("{other}"),
        };
        corrupt(
            damaged(|bytes| {
                let offset = inventory_offset(bytes);
                bytes[offset] = b'!';
            }),
            "invalid inventory",
        );
        corrupt(
            damaged(|bytes| set_header(bytes, 0, u64::MAX - 1)),
            "outside the file",
        );
        corrupt(
            with_inventory(|inventory| inventory["entries"][0]["byteLength"] = 1_000_000.into()),
            "outside the file",
        );
        corrupt(
            with_inventory(|inventory| inventory["entries"][1]["offset"] = 64.into()),
            "overlaps",
        );
        corrupt(
            with_inventory(|inventory| inventory["entries"][0]["offset"] = 96.into()),
            "aligned",
        );
        corrupt(
            with_inventory(|inventory| inventory["entries"][2]["path"] = "content/body.bin".into()),
            "duplicated",
        );
        corrupt(
            with_inventory(|inventory| inventory["entries"][1]["bundle"] = "other".into()),
            "not under bundle",
        );
    }
}
