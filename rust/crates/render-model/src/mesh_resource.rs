use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::{MeshDescriptorError, MeshPayloadDescriptor, MeshPayloadSource, MeshResourceEncoding};

/// One resource remains comfortably inside the Studio host/browser allocation
/// ceiling. Larger payload sets are partitioned deterministically.
pub const MAX_MESH_RESOURCE_BYTES: u32 = 64 * 1024 * 1024;
pub const MESH_RESOURCE_HEADER_BYTES: u32 = 16;
pub const MESH_RESOURCE_MAGIC: [u8; 8] = *b"RMSHLE01";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedMeshResource {
    pub resource: String,
    pub content_hash: String,
    pub bytes: Vec<u8>,
}

impl PackedMeshResource {
    pub fn byte_length(&self) -> u32 {
        u32::try_from(self.bytes.len()).expect("packed mesh resources are bounded to u32")
    }

    pub fn validate(&self) -> Result<(), MeshResourceError> {
        validate_mesh_resource_identity(&self.resource, &self.content_hash)?;
        let byte_length =
            u32::try_from(self.bytes.len()).map_err(|_| MeshResourceError::ResourceTooLarge {
                bytes: self.bytes.len(),
            })?;
        if !(MESH_RESOURCE_HEADER_BYTES..=MAX_MESH_RESOURCE_BYTES).contains(&byte_length) {
            return Err(MeshResourceError::ResourceTooLarge {
                bytes: self.bytes.len(),
            });
        }
        validate_mesh_resource_header(&self.bytes)?;
        let actual = mesh_resource_content_hash(&self.bytes);
        if actual != self.content_hash {
            return Err(MeshResourceError::ContentHashMismatch {
                expected: self.content_hash.clone(),
                actual,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackedMeshResourceSet {
    pub payloads: Vec<MeshPayloadDescriptor>,
    pub resources: Vec<PackedMeshResource>,
}

/// Packs inline f32/u32 mesh streams into deterministic, content-addressed
/// resources. The returned descriptors retain layout, bounds, groups, and
/// provenance while replacing only the data source.
pub fn pack_mesh_resources(
    payloads: &[MeshPayloadDescriptor],
    maximum_resource_bytes: u32,
) -> Result<PackedMeshResourceSet, MeshResourceError> {
    if !(MESH_RESOURCE_HEADER_BYTES..=MAX_MESH_RESOURCE_BYTES).contains(&maximum_resource_bytes) {
        return Err(MeshResourceError::InvalidMaximum {
            bytes: maximum_resource_bytes,
        });
    }
    if payloads.is_empty() {
        return Ok(PackedMeshResourceSet {
            payloads: Vec::new(),
            resources: Vec::new(),
        });
    }

    let stream_lengths = payloads
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            payload
                .validate()
                .map_err(|source| MeshResourceError::InvalidPayload { index, source })?;
            if !matches!(payload.source, MeshPayloadSource::Inline { .. }) {
                return Err(MeshResourceError::PayloadNotInline { index });
            }
            mesh_stream_bytes(payload)
                .ok_or(MeshResourceError::ResourceTooLarge { bytes: usize::MAX })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut ranges = Vec::new();
    let mut start = 0;
    let mut current = MESH_RESOURCE_HEADER_BYTES as usize;
    for (index, stream_bytes) in stream_lengths.iter().copied().enumerate() {
        let single = MESH_RESOURCE_HEADER_BYTES as usize + stream_bytes;
        if single > maximum_resource_bytes as usize {
            return Err(MeshResourceError::MeshExceedsMaximum {
                index,
                bytes: single,
                maximum: maximum_resource_bytes,
            });
        }
        if index > start && current + stream_bytes > maximum_resource_bytes as usize {
            ranges.push(start..index);
            start = index;
            current = MESH_RESOURCE_HEADER_BYTES as usize;
        }
        current += stream_bytes;
    }
    ranges.push(start..payloads.len());

    let mut packed_payloads = payloads.to_vec();
    let mut resources_by_id = BTreeMap::new();
    for range in ranges {
        let mut bytes = vec![0; MESH_RESOURCE_HEADER_BYTES as usize];
        bytes[..8].copy_from_slice(&MESH_RESOURCE_MAGIC);
        bytes[12..16].copy_from_slice(&(range.len() as u32).to_le_bytes());
        let mut offsets = Vec::with_capacity(range.len());
        for payload in &payloads[range.clone()] {
            let MeshPayloadSource::Inline {
                positions,
                normals,
                indices,
            } = &payload.source
            else {
                unreachable!("inline sources were checked before packing")
            };
            let positions_byte_offset = u32::try_from(bytes.len())
                .map_err(|_| MeshResourceError::ResourceTooLarge { bytes: bytes.len() })?;
            push_f32s(&mut bytes, positions);
            let normals_byte_offset = u32::try_from(bytes.len())
                .map_err(|_| MeshResourceError::ResourceTooLarge { bytes: bytes.len() })?;
            push_f32s(&mut bytes, normals);
            let indices_byte_offset = u32::try_from(bytes.len())
                .map_err(|_| MeshResourceError::ResourceTooLarge { bytes: bytes.len() })?;
            push_u32s(&mut bytes, indices);
            offsets.push((
                positions_byte_offset,
                normals_byte_offset,
                indices_byte_offset,
            ));
        }
        let byte_length = u32::try_from(bytes.len())
            .map_err(|_| MeshResourceError::ResourceTooLarge { bytes: bytes.len() })?;
        bytes[8..12].copy_from_slice(&byte_length.to_le_bytes());
        let content_hash = mesh_resource_content_hash(&bytes);
        let resource = format!("mesh-resource/{}", &content_hash["sha256:".len()..]);

        for (local_index, payload_index) in range.clone().enumerate() {
            let (positions_byte_offset, normals_byte_offset, indices_byte_offset) =
                offsets[local_index];
            packed_payloads[payload_index].source = MeshPayloadSource::Resource {
                resource: resource.clone(),
                content_hash: content_hash.clone(),
                byte_length,
                encoding: MeshResourceEncoding::PackedStreamsLeV1,
                positions_byte_offset,
                normals_byte_offset,
                indices_byte_offset,
            };
            packed_payloads[payload_index]
                .validate()
                .map_err(|source| MeshResourceError::InvalidPackedPayload {
                    index: payload_index,
                    source,
                })?;
        }

        resources_by_id
            .entry(resource.clone())
            .or_insert(PackedMeshResource {
                resource,
                content_hash,
                bytes,
            });
    }

    let resources = resources_by_id.into_values().collect::<Vec<_>>();
    for resource in &resources {
        resource.validate()?;
    }
    Ok(PackedMeshResourceSet {
        payloads: packed_payloads,
        resources,
    })
}

pub fn validate_mesh_resource_identity(
    resource: &str,
    content_hash: &str,
) -> Result<(), MeshResourceError> {
    let Some(digest) = content_hash.strip_prefix("sha256:") else {
        return Err(MeshResourceError::InvalidContentHash);
    };
    if digest.len() != 64
        || !digest
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(MeshResourceError::InvalidContentHash);
    }
    if resource != format!("mesh-resource/{digest}") {
        return Err(MeshResourceError::InvalidResourceIdentity);
    }
    Ok(())
}

pub fn validate_mesh_resource_header(bytes: &[u8]) -> Result<(), MeshResourceError> {
    if bytes.len() < MESH_RESOURCE_HEADER_BYTES as usize || bytes[..8] != MESH_RESOURCE_MAGIC {
        return Err(MeshResourceError::InvalidHeader);
    }
    let declared = u32::from_le_bytes(bytes[8..12].try_into().expect("four-byte slice"));
    if declared as usize != bytes.len() {
        return Err(MeshResourceError::HeaderLengthMismatch {
            declared,
            actual: bytes.len(),
        });
    }
    let payload_count = u32::from_le_bytes(bytes[12..16].try_into().expect("four-byte slice"));
    if payload_count == 0 {
        return Err(MeshResourceError::EmptyResource);
    }
    Ok(())
}

pub fn mesh_resource_content_hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn mesh_stream_bytes(payload: &MeshPayloadDescriptor) -> Option<usize> {
    let vertices = usize::try_from(payload.layout.vertex_count).ok()?;
    let indices = usize::try_from(payload.layout.index_count).ok()?;
    vertices
        .checked_mul(3)?
        .checked_mul(4)?
        .checked_mul(2)?
        .checked_add(indices.checked_mul(4)?)
}

fn push_f32s(bytes: &mut Vec<u8>, values: &[f32]) {
    bytes.reserve(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn push_u32s(bytes: &mut Vec<u8>, values: &[u32]) {
    bytes.reserve(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshResourceError {
    InvalidMaximum {
        bytes: u32,
    },
    InvalidPayload {
        index: usize,
        source: MeshDescriptorError,
    },
    PayloadNotInline {
        index: usize,
    },
    MeshExceedsMaximum {
        index: usize,
        bytes: usize,
        maximum: u32,
    },
    ResourceTooLarge {
        bytes: usize,
    },
    InvalidPackedPayload {
        index: usize,
        source: MeshDescriptorError,
    },
    InvalidContentHash,
    InvalidResourceIdentity,
    InvalidHeader,
    HeaderLengthMismatch {
        declared: u32,
        actual: usize,
    },
    EmptyResource,
    ContentHashMismatch {
        expected: String,
        actual: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MeshAttribute, MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor,
        MeshBufferLayout, MeshGroupDescriptor, MeshIndexWidth, MeshProvenance,
    };

    fn triangle(offset: f32) -> MeshPayloadDescriptor {
        MeshPayloadDescriptor {
            layout: MeshBufferLayout {
                vertex_count: 3,
                index_count: 3,
                index_width: MeshIndexWidth::U32,
                attributes: vec![
                    MeshAttribute {
                        name: MeshAttributeName::Position,
                        components: 3,
                        kind: MeshAttributeKind::F32,
                    },
                    MeshAttribute {
                        name: MeshAttributeName::Normal,
                        components: 3,
                        kind: MeshAttributeKind::F32,
                    },
                ],
            },
            groups: vec![MeshGroupDescriptor {
                material_slot: 0,
                start: 0,
                count: 3,
            }],
            bounds: MeshBoundsDescriptor {
                min: [offset, 0.0, 0.0],
                max: [offset + 1.0, 1.0, 0.0],
            },
            source: MeshPayloadSource::Inline {
                positions: vec![offset, 0.0, 0.0, offset + 1.0, 0.0, 0.0, offset, 1.0, 0.0],
                normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                indices: vec![0, 1, 2],
            },
            provenance: MeshProvenance::VoxelObject,
        }
    }

    #[test]
    fn packed_resources_are_deterministic_and_round_trip_the_contract() {
        let first = pack_mesh_resources(&[triangle(0.0), triangle(2.0)], 1024).unwrap();
        let second = pack_mesh_resources(&[triangle(0.0), triangle(2.0)], 1024).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.resources.len(), 1);
        first.resources[0].validate().unwrap();
        assert!(first
            .payloads
            .iter()
            .all(|payload| matches!(payload.source, MeshPayloadSource::Resource { .. })));
    }

    #[test]
    fn packing_partitions_before_the_resource_ceiling() {
        let one_mesh_bytes = MESH_RESOURCE_HEADER_BYTES + (3 * 3 * 4 * 2) + (3 * 4);
        let packed = pack_mesh_resources(&[triangle(0.0), triangle(2.0)], one_mesh_bytes).unwrap();
        assert_eq!(packed.resources.len(), 2);
        assert_ne!(packed.resources[0].resource, packed.resources[1].resource);
    }

    #[test]
    fn content_identity_and_header_reject_tampering() {
        let mut packed = pack_mesh_resources(&[triangle(0.0)], 1024).unwrap();
        packed.resources[0].bytes[12] = 0;
        assert!(matches!(
            packed.resources[0].validate(),
            Err(MeshResourceError::EmptyResource)
        ));
    }
}
