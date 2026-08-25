use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::{MeshDescriptorError, MeshPayloadDescriptor, MeshPayloadSource, MeshResourceEncoding};

/// One resource remains comfortably inside the Studio host/browser allocation
/// ceiling. Larger payload sets are partitioned deterministically.
pub const MAX_MESH_RESOURCE_BYTES: u32 = 64 * 1024 * 1024;
/// Matches the owning renderer-host admission ceiling for one retained set.
pub const MAX_MESH_RESOURCE_AGGREGATE_BYTES: usize = 256 * 1024 * 1024;
pub const MESH_RESOURCE_HEADER_BYTES: u32 = 16;
pub const MESH_RESOURCE_MAGIC: [u8; 8] = *b"RMSHLE01";
pub const MESH_RESOURCE_MAGIC_V2: [u8; 8] = *b"RMSHLE02";
pub const MESH_RESOURCE_MAGIC_V3: [u8; 8] = *b"RMSHLE03";

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

    let stream_kinds = payloads
        .iter()
        .map(|payload| {
            let MeshPayloadSource::Inline { uvs, colors, .. } = &payload.source else {
                unreachable!("inline sources were checked before packing")
            };
            (uvs.is_some(), colors.is_some())
        })
        .collect::<Vec<_>>();
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
        if index > start
            && (current + stream_bytes > maximum_resource_bytes as usize
                || stream_kinds[index] != stream_kinds[start])
        {
            ranges.push(start..index);
            start = index;
            current = MESH_RESOURCE_HEADER_BYTES as usize;
        }
        current += stream_bytes;
    }
    ranges.push(start..payloads.len());
    validate_aggregate_resource_bytes(ranges.iter().map(|range| {
        MESH_RESOURCE_HEADER_BYTES as usize + stream_lengths[range.clone()].iter().sum::<usize>()
    }))?;

    let mut packed_payloads = payloads.to_vec();
    let mut resources_by_id = BTreeMap::new();
    for range in ranges {
        let mut bytes = vec![0; MESH_RESOURCE_HEADER_BYTES as usize];
        let encoding = match stream_kinds[range.start] {
            (false, false) => MeshResourceEncoding::PackedStreamsLeV1,
            (true, false) => MeshResourceEncoding::PackedStreamsLeV2,
            (_, true) => MeshResourceEncoding::PackedStreamsLeV3,
        };
        let magic = match encoding {
            MeshResourceEncoding::PackedStreamsLeV1 => MESH_RESOURCE_MAGIC,
            MeshResourceEncoding::PackedStreamsLeV2 => MESH_RESOURCE_MAGIC_V2,
            MeshResourceEncoding::PackedStreamsLeV3 => MESH_RESOURCE_MAGIC_V3,
        };
        bytes[..8].copy_from_slice(&magic);
        bytes[12..16].copy_from_slice(&(range.len() as u32).to_le_bytes());
        let mut offsets = Vec::with_capacity(range.len());
        for payload in &payloads[range.clone()] {
            let MeshPayloadSource::Inline {
                positions,
                normals,
                uvs,
                colors,
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
            let uvs_byte_offset = if let Some(uvs) = uvs {
                let offset = u32::try_from(bytes.len())
                    .map_err(|_| MeshResourceError::ResourceTooLarge { bytes: bytes.len() })?;
                push_f32s(&mut bytes, uvs);
                Some(offset)
            } else {
                None
            };
            let colors_byte_offset = if let Some(colors) = colors {
                let offset = u32::try_from(bytes.len())
                    .map_err(|_| MeshResourceError::ResourceTooLarge { bytes: bytes.len() })?;
                push_f32s(&mut bytes, colors);
                Some(offset)
            } else {
                None
            };
            let indices_byte_offset = u32::try_from(bytes.len())
                .map_err(|_| MeshResourceError::ResourceTooLarge { bytes: bytes.len() })?;
            push_u32s(&mut bytes, indices);
            offsets.push((
                positions_byte_offset,
                normals_byte_offset,
                uvs_byte_offset,
                colors_byte_offset,
                indices_byte_offset,
            ));
        }
        let byte_length = u32::try_from(bytes.len())
            .map_err(|_| MeshResourceError::ResourceTooLarge { bytes: bytes.len() })?;
        bytes[8..12].copy_from_slice(&byte_length.to_le_bytes());
        let content_hash = mesh_resource_content_hash(&bytes);
        let resource = format!("mesh-resource/{}", &content_hash["sha256:".len()..]);

        for (local_index, payload_index) in range.clone().enumerate() {
            let (
                positions_byte_offset,
                normals_byte_offset,
                uvs_byte_offset,
                colors_byte_offset,
                indices_byte_offset,
            ) = offsets[local_index];
            packed_payloads[payload_index].source = MeshPayloadSource::Resource {
                resource: resource.clone(),
                content_hash: content_hash.clone(),
                byte_length,
                encoding,
                positions_byte_offset,
                normals_byte_offset,
                uvs_byte_offset,
                colors_byte_offset,
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
    if bytes.len() < MESH_RESOURCE_HEADER_BYTES as usize
        || (bytes[..8] != MESH_RESOURCE_MAGIC
            && bytes[..8] != MESH_RESOURCE_MAGIC_V2
            && bytes[..8] != MESH_RESOURCE_MAGIC_V3)
    {
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

/// Identifies one of the streams in a packed mesh resource for decode
/// diagnostics. The stream order itself is fixed by the resource encoding;
/// offsets in the admitted descriptor select the exact byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshResourceStreamKind {
    Positions,
    Normals,
    Uvs,
    Colors,
    Indices,
}

/// Decodes one admitted resource-backed payload into an owned inline payload.
///
/// The resource descriptor is validated again before any stream is read. The
/// supplied bytes must be the complete content-addressed resource named by
/// the descriptor, including its 16-byte header. No host, filesystem, or
/// resolver is involved here; callers decide where admitted bytes come from.
/// All decoded vectors remain local until the complete inline descriptor also
/// validates, so malformed input cannot produce a partially usable payload.
pub fn decode_mesh_resource_payload(
    payload: &MeshPayloadDescriptor,
    bytes: &[u8],
) -> Result<MeshPayloadDescriptor, MeshResourceError> {
    payload
        .validate()
        .map_err(|source| MeshResourceError::InvalidResourcePayload { source })?;

    let MeshPayloadSource::Resource {
        resource,
        content_hash,
        byte_length,
        encoding,
        positions_byte_offset,
        normals_byte_offset,
        uvs_byte_offset,
        colors_byte_offset,
        indices_byte_offset,
    } = &payload.source
    else {
        return Err(MeshResourceError::PayloadNotResource);
    };

    let actual_byte_length = u32::try_from(bytes.len())
        .map_err(|_| MeshResourceError::ResourceTooLarge { bytes: bytes.len() })?;
    if actual_byte_length != *byte_length {
        return Err(MeshResourceError::ResourceByteLengthMismatch {
            expected: *byte_length,
            actual: bytes.len(),
        });
    }

    // Repeat the identity check here rather than relying only on descriptor
    // validation so this function remains safe if its admission ordering is
    // changed later.
    validate_mesh_resource_identity(resource, content_hash)?;
    validate_mesh_resource_header(bytes)?;
    let actual_hash = mesh_resource_content_hash(bytes);
    if actual_hash != *content_hash {
        return Err(MeshResourceError::ContentHashMismatch {
            expected: content_hash.clone(),
            actual: actual_hash,
        });
    }

    let header_encoding =
        encoding_for_magic(&bytes[..8]).ok_or(MeshResourceError::InvalidHeader)?;
    if header_encoding != *encoding {
        return Err(MeshResourceError::ResourceEncodingMismatch {
            descriptor: *encoding,
            header: header_encoding,
        });
    }

    let vertex_count = payload.layout.vertex_count;
    let index_count = payload.layout.index_count;
    let positions = decode_f32_stream(
        bytes,
        *positions_byte_offset,
        vertex_count,
        3,
        MeshResourceStreamKind::Positions,
        *byte_length,
    )?;
    let normals = decode_f32_stream(
        bytes,
        *normals_byte_offset,
        vertex_count,
        3,
        MeshResourceStreamKind::Normals,
        *byte_length,
    )?;
    let uvs = uvs_byte_offset
        .map(|offset| {
            decode_f32_stream(
                bytes,
                offset,
                vertex_count,
                2,
                MeshResourceStreamKind::Uvs,
                *byte_length,
            )
        })
        .transpose()?;
    let colors = colors_byte_offset
        .map(|offset| {
            decode_f32_stream(
                bytes,
                offset,
                vertex_count,
                4,
                MeshResourceStreamKind::Colors,
                *byte_length,
            )
        })
        .transpose()?;
    let indices = decode_u32_stream(
        bytes,
        *indices_byte_offset,
        index_count,
        MeshResourceStreamKind::Indices,
        *byte_length,
    )?;

    let decoded = MeshPayloadDescriptor {
        layout: payload.layout.clone(),
        groups: payload.groups.clone(),
        bounds: payload.bounds,
        source: MeshPayloadSource::Inline {
            positions,
            normals,
            uvs,
            colors,
            indices,
        },
        provenance: payload.provenance,
    };
    decoded
        .validate()
        .map_err(|source| MeshResourceError::DecodedPayloadInvalid { source })?;
    Ok(decoded)
}

fn encoding_for_magic(magic: &[u8]) -> Option<MeshResourceEncoding> {
    match magic {
        value if value == MESH_RESOURCE_MAGIC => Some(MeshResourceEncoding::PackedStreamsLeV1),
        value if value == MESH_RESOURCE_MAGIC_V2 => Some(MeshResourceEncoding::PackedStreamsLeV2),
        value if value == MESH_RESOURCE_MAGIC_V3 => Some(MeshResourceEncoding::PackedStreamsLeV3),
        _ => None,
    }
}

fn decode_f32_stream(
    bytes: &[u8],
    offset: u32,
    vertex_count: u32,
    components: usize,
    stream: MeshResourceStreamKind,
    byte_length: u32,
) -> Result<Vec<f32>, MeshResourceError> {
    let value_count = usize::try_from(vertex_count)
        .ok()
        .and_then(|count| count.checked_mul(components))
        .ok_or(MeshResourceError::DecodeStreamOutOfRange {
            stream,
            offset,
            byte_length,
        })?;
    let range = decode_stream_range(bytes, offset, value_count, stream, byte_length)?;
    let mut values = Vec::with_capacity(value_count);
    for (index, chunk) in bytes[range].chunks_exact(4).enumerate() {
        let value = f32::from_le_bytes(chunk.try_into().expect("chunks_exact guarantees width"));
        if !value.is_finite() {
            return Err(MeshResourceError::NonFiniteStreamValue { stream, index });
        }
        values.push(value);
    }
    Ok(values)
}

fn decode_u32_stream(
    bytes: &[u8],
    offset: u32,
    index_count: u32,
    stream: MeshResourceStreamKind,
    byte_length: u32,
) -> Result<Vec<u32>, MeshResourceError> {
    let value_count =
        usize::try_from(index_count).map_err(|_| MeshResourceError::DecodeStreamOutOfRange {
            stream,
            offset,
            byte_length,
        })?;
    let range = decode_stream_range(bytes, offset, value_count, stream, byte_length)?;
    let mut values = Vec::with_capacity(value_count);
    for chunk in bytes[range].chunks_exact(4) {
        values.push(u32::from_le_bytes(
            chunk.try_into().expect("chunks_exact guarantees width"),
        ));
    }
    Ok(values)
}

fn decode_stream_range(
    bytes: &[u8],
    offset: u32,
    value_count: usize,
    stream: MeshResourceStreamKind,
    byte_length: u32,
) -> Result<std::ops::Range<usize>, MeshResourceError> {
    let start = usize::try_from(offset).map_err(|_| MeshResourceError::DecodeStreamOutOfRange {
        stream,
        offset,
        byte_length,
    })?;
    let byte_count =
        value_count
            .checked_mul(4)
            .ok_or(MeshResourceError::DecodeStreamOutOfRange {
                stream,
                offset,
                byte_length,
            })?;
    let end = start
        .checked_add(byte_count)
        .ok_or(MeshResourceError::DecodeStreamOutOfRange {
            stream,
            offset,
            byte_length,
        })?;
    if end > bytes.len() {
        return Err(MeshResourceError::DecodeStreamOutOfRange {
            stream,
            offset,
            byte_length,
        });
    }
    Ok(start..end)
}

fn mesh_stream_bytes(payload: &MeshPayloadDescriptor) -> Option<usize> {
    let vertices = usize::try_from(payload.layout.vertex_count).ok()?;
    let indices = usize::try_from(payload.layout.index_count).ok()?;
    let base = vertices
        .checked_mul(3)?
        .checked_mul(4)?
        .checked_mul(2)?
        .checked_add(indices.checked_mul(4)?)?;
    match &payload.source {
        MeshPayloadSource::Inline { uvs, colors, .. } => base
            .checked_add(if uvs.is_some() {
                vertices.checked_mul(2)?.checked_mul(4)?
            } else {
                0
            })?
            .checked_add(if colors.is_some() {
                vertices.checked_mul(4)?.checked_mul(4)?
            } else {
                0
            }),
        _ => Some(base),
    }
}

fn push_f32s(bytes: &mut Vec<u8>, values: &[f32]) {
    bytes.reserve(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn validate_aggregate_resource_bytes(
    resource_bytes: impl IntoIterator<Item = usize>,
) -> Result<(), MeshResourceError> {
    let mut total = 0_usize;
    for bytes in resource_bytes {
        total =
            total
                .checked_add(bytes)
                .ok_or(MeshResourceError::AggregateResourceBytesExceeded {
                    bytes: usize::MAX,
                    maximum: MAX_MESH_RESOURCE_AGGREGATE_BYTES,
                })?;
        if total > MAX_MESH_RESOURCE_AGGREGATE_BYTES {
            return Err(MeshResourceError::AggregateResourceBytesExceeded {
                bytes: total,
                maximum: MAX_MESH_RESOURCE_AGGREGATE_BYTES,
            });
        }
    }
    Ok(())
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
    AggregateResourceBytesExceeded {
        bytes: usize,
        maximum: usize,
    },
    InvalidPackedPayload {
        index: usize,
        source: MeshDescriptorError,
    },
    InvalidResourcePayload {
        source: MeshDescriptorError,
    },
    PayloadNotResource,
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
    ResourceByteLengthMismatch {
        expected: u32,
        actual: usize,
    },
    ResourceEncodingMismatch {
        descriptor: MeshResourceEncoding,
        header: MeshResourceEncoding,
    },
    DecodeStreamOutOfRange {
        stream: MeshResourceStreamKind,
        offset: u32,
        byte_length: u32,
    },
    NonFiniteStreamValue {
        stream: MeshResourceStreamKind,
        index: usize,
    },
    DecodedPayloadInvalid {
        source: MeshDescriptorError,
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
                uvs: None,
                colors: None,
                indices: vec![0, 1, 2],
            },
            provenance: MeshProvenance::VoxelObject,
        }
    }

    fn textured_triangle(offset: f32) -> MeshPayloadDescriptor {
        let mut payload = triangle(offset);
        payload.layout.attributes.push(MeshAttribute {
            name: MeshAttributeName::Uv,
            components: 2,
            kind: MeshAttributeKind::F32,
        });
        let MeshPayloadSource::Inline { uvs, .. } = &mut payload.source else {
            unreachable!()
        };
        *uvs = Some(vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        payload
    }

    fn colored_triangle(offset: f32) -> MeshPayloadDescriptor {
        let mut payload = triangle(offset);
        payload.layout.attributes.push(MeshAttribute {
            name: MeshAttributeName::Color,
            components: 4,
            kind: MeshAttributeKind::F32,
        });
        let MeshPayloadSource::Inline { colors, .. } = &mut payload.source else {
            unreachable!()
        };
        *colors = Some(vec![
            1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.5, 0.0, 0.0, 1.0, 1.0,
        ]);
        payload
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

    #[test]
    fn legacy_v1_bytes_stay_exact_and_uv_payloads_use_partitioned_v2_resources() {
        let legacy = pack_mesh_resources(&[triangle(0.0)], 1024).unwrap();
        assert_eq!(legacy.resources[0].bytes[..8], MESH_RESOURCE_MAGIC);
        assert_eq!(
            legacy.resources[0].content_hash,
            "sha256:daeca78b7e966826d5311bf7aeb02e11baf414a6fe2fd395d00e9782f21d3659"
        );
        assert!(matches!(
            legacy.payloads[0].source,
            MeshPayloadSource::Resource {
                encoding: MeshResourceEncoding::PackedStreamsLeV1,
                uvs_byte_offset: None,
                ..
            }
        ));

        let packed = pack_mesh_resources(&[triangle(0.0), textured_triangle(2.0)], 1024).unwrap();
        assert_eq!(packed.resources.len(), 2);
        assert!(packed
            .resources
            .iter()
            .any(|resource| resource.bytes[..8] == MESH_RESOURCE_MAGIC));
        assert!(packed
            .resources
            .iter()
            .any(|resource| resource.bytes[..8] == MESH_RESOURCE_MAGIC_V2));
        assert!(matches!(
            packed.payloads[1].source,
            MeshPayloadSource::Resource {
                encoding: MeshResourceEncoding::PackedStreamsLeV2,
                uvs_byte_offset: Some(_),
                ..
            }
        ));
        packed
            .resources
            .iter()
            .for_each(|resource| resource.validate().unwrap());
        packed
            .payloads
            .iter()
            .for_each(|payload| payload.validate().unwrap());
    }

    #[test]
    fn v2_aggregate_admission_accepts_the_host_limit_and_rejects_one_over() {
        assert_eq!(
            validate_aggregate_resource_bytes([64 * 1024 * 1024; 4]),
            Ok(())
        );
        assert_eq!(
            validate_aggregate_resource_bytes([64 * 1024 * 1024; 4].into_iter().chain([1])),
            Err(MeshResourceError::AggregateResourceBytesExceeded {
                bytes: MAX_MESH_RESOURCE_AGGREGATE_BYTES + 1,
                maximum: MAX_MESH_RESOURCE_AGGREGATE_BYTES,
            })
        );
    }

    #[test]
    fn normalized_vertex_colors_use_v3_resources_without_changing_legacy_encodings() {
        let packed = pack_mesh_resources(&[colored_triangle(0.0)], 1024).unwrap();
        assert_eq!(packed.resources[0].bytes[..8], MESH_RESOURCE_MAGIC_V3);
        assert!(matches!(
            packed.payloads[0].source,
            MeshPayloadSource::Resource {
                encoding: MeshResourceEncoding::PackedStreamsLeV3,
                uvs_byte_offset: None,
                colors_byte_offset: Some(_),
                ..
            }
        ));
        packed.payloads[0].validate().unwrap();
        packed.resources[0].validate().unwrap();
    }

    fn assert_decode_round_trip(original: MeshPayloadDescriptor) {
        let packed = pack_mesh_resources(std::slice::from_ref(&original), 1024).unwrap();
        let decoded =
            decode_mesh_resource_payload(&packed.payloads[0], &packed.resources[0].bytes).unwrap();
        assert_eq!(decoded, original);

        let repacked = pack_mesh_resources(std::slice::from_ref(&decoded), 1024).unwrap();
        assert_eq!(repacked.resources, packed.resources);
    }

    fn refresh_resource_identity(payload: &mut MeshPayloadDescriptor, bytes: &[u8]) {
        let MeshPayloadSource::Resource {
            resource,
            content_hash,
            byte_length,
            ..
        } = &mut payload.source
        else {
            unreachable!("test payload is resource-backed")
        };
        *content_hash = mesh_resource_content_hash(bytes);
        *resource = format!("mesh-resource/{}", &content_hash["sha256:".len()..]);
        *byte_length = u32::try_from(bytes.len()).unwrap();
    }

    #[test]
    fn resource_decode_round_trips_v1_v2_and_v3_streams() {
        assert_decode_round_trip(triangle(0.0));
        assert_decode_round_trip(textured_triangle(2.0));
        assert_decode_round_trip(colored_triangle(4.0));
    }

    #[test]
    fn resource_decode_rejects_descriptor_hash_header_and_length_drift() {
        let packed = pack_mesh_resources(&[triangle(0.0)], 1024).unwrap();

        let mut hash_drift = packed.payloads[0].clone();
        let mut tampered = packed.resources[0].bytes.clone();
        tampered[16] ^= 1;
        assert!(matches!(
            decode_mesh_resource_payload(&hash_drift, &tampered),
            Err(MeshResourceError::ContentHashMismatch { .. })
        ));

        let mut header_drift = packed.payloads[0].clone();
        let mut wrong_header = packed.resources[0].bytes.clone();
        wrong_header[..8].copy_from_slice(&MESH_RESOURCE_MAGIC_V2);
        refresh_resource_identity(&mut header_drift, &wrong_header);
        assert!(matches!(
            decode_mesh_resource_payload(&header_drift, &wrong_header),
            Err(MeshResourceError::ResourceEncodingMismatch {
                descriptor: MeshResourceEncoding::PackedStreamsLeV1,
                header: MeshResourceEncoding::PackedStreamsLeV2,
            })
        ));

        let mut truncated = packed.resources[0].bytes.clone();
        truncated.pop();
        assert!(matches!(
            decode_mesh_resource_payload(&packed.payloads[0], &truncated),
            Err(MeshResourceError::ResourceByteLengthMismatch { .. })
        ));

        let mut invalid_header = packed.resources[0].bytes.clone();
        invalid_header[..8].copy_from_slice(b"NOTMESH!");
        refresh_resource_identity(&mut hash_drift, &invalid_header);
        assert!(matches!(
            decode_mesh_resource_payload(&hash_drift, &invalid_header),
            Err(MeshResourceError::InvalidHeader)
        ));
    }

    #[test]
    fn resource_decode_rejects_offset_misalignment_and_stream_bounds() {
        let packed = pack_mesh_resources(&[triangle(0.0)], 1024).unwrap();

        let mut misaligned = packed.payloads[0].clone();
        let MeshPayloadSource::Resource {
            positions_byte_offset,
            ..
        } = &mut misaligned.source
        else {
            unreachable!()
        };
        *positions_byte_offset += 1;
        assert!(matches!(
            decode_mesh_resource_payload(&misaligned, &packed.resources[0].bytes),
            Err(MeshResourceError::InvalidResourcePayload {
                source: MeshDescriptorError::InvalidResourceOffset { .. },
            })
        ));

        let mut out_of_bounds = packed.payloads[0].clone();
        let byte_length = match &out_of_bounds.source {
            MeshPayloadSource::Resource { byte_length, .. } => *byte_length,
            _ => unreachable!(),
        };
        let MeshPayloadSource::Resource {
            positions_byte_offset,
            ..
        } = &mut out_of_bounds.source
        else {
            unreachable!()
        };
        *positions_byte_offset = byte_length;
        assert!(matches!(
            decode_mesh_resource_payload(&out_of_bounds, &packed.resources[0].bytes),
            Err(MeshResourceError::InvalidResourcePayload {
                source: MeshDescriptorError::ResourceStreamOutOfRange { .. },
            })
        ));
    }

    #[test]
    fn resource_decode_rejects_decoded_indices_outside_the_declared_vertex_range() {
        let packed = pack_mesh_resources(&[triangle(0.0)], 1024).unwrap();
        let mut bytes = packed.resources[0].bytes.clone();
        let MeshPayloadSource::Resource {
            indices_byte_offset,
            ..
        } = &packed.payloads[0].source
        else {
            unreachable!()
        };
        let offset = usize::try_from(*indices_byte_offset).unwrap();
        bytes[offset..offset + 4].copy_from_slice(&99_u32.to_le_bytes());

        let mut descriptor = packed.payloads[0].clone();
        refresh_resource_identity(&mut descriptor, &bytes);
        assert!(matches!(
            decode_mesh_resource_payload(&descriptor, &bytes),
            Err(MeshResourceError::DecodedPayloadInvalid {
                source: MeshDescriptorError::IndexOutOfRange {
                    index: 99,
                    vertex_count: 3,
                },
            })
        ));
    }
}
