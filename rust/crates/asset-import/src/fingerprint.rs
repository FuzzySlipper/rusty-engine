use core_assets::AssetHash;
use sha2::{Digest, Sha256};

pub fn fingerprint_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

pub fn fingerprint_hash(bytes: &[u8]) -> AssetHash {
    AssetHash::parse(&fingerprint_hex(bytes)).expect("SHA-256 is lowercase hexadecimal")
}
