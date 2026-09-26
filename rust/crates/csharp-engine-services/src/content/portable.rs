use super::*;
use asset_catalog::portable::{PortableAssets, PortableDefinition};

#[derive(Default)]
pub(super) struct PortableState {
    assets: BTreeMap<u64, Asset>,
    leases: BTreeMap<u64, Readout>,
    errors: BTreeMap<u64, Diagnostic>,
    next: u64,
}
struct Asset {
    document: PortableAssets,
    selected: String,
    members: BTreeMap<String, AdmittedContent>,
}
#[derive(Default)]
struct Text(Vec<String>);
impl Text {
    fn copy(&mut self, value: &str) -> NativeUtf8Slice {
        self.0.push(value.to_owned());
        slice(self.0.last().unwrap())
    }
}
fn slice(s: &str) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: s.as_ptr(),
        len: s.len(),
    }
}
struct Diagnostic {
    _message: String,
    _row: Box<NativeEngineDiagnostic>,
}
#[derive(Default)]
struct Readout {
    text: Text,
    members: Vec<NativePortableAssetMember>,
    frames: Vec<NativePortableSpriteFrame>,
    anchors: Vec<NativePortableSpriteAnchor>,
    animation_frames: Vec<NativePortableSpriteAnimationFrame>,
    directions: Vec<NativePortableSpriteDirection>,
    actions: Vec<NativePortableSpriteAction>,
    relationships: Vec<NativePortableAssetRelationship>,
    attachments: Vec<NativePortableMeshAttachment>,
}
impl PortableState {
    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
    fn fail(
        &mut self,
        operation: &'static str,
        message: String,
        out: *mut NativeOperationErrorReceipt,
    ) -> i32 {
        if !out.is_null() {
            let value = self.id();
            let row = Box::new(NativeEngineDiagnostic {
                code: slice("portable_asset"),
                message: slice(&message),
                source: slice(""),
            });
            let diagnostics = NativeEngineDiagnosticLease {
                handle: NativeEngineDiagnosticLeaseHandle { value },
                diagnostics: &*row,
                diagnostics_len: 1,
            };
            self.errors.insert(
                value,
                Diagnostic {
                    _message: message,
                    _row: row,
                },
            );
            // Operation names are static literals at every call site.
            unsafe {
                *out = NativeOperationErrorReceipt {
                    service: slice("Content"),
                    operation: slice(operation),
                    status: 0,
                    diagnostics,
                };
            }
        }
        0
    }
}
fn build(source: AdmittedContent, selected: String) -> Result<Asset, String> {
    let document =
        PortableAssets::decode(&source.bytes).map_err(|e| format!("{}: {e}", source.path))?;
    let closure = document
        .resolve(&selected)
        .map_err(|e| format!("{}: {e}", source.path))?;
    let directory = source
        .path
        .rsplit_once('/')
        .map(|(p, _)| format!("{p}/"))
        .unwrap_or_default();
    let mut members = BTreeMap::new();
    for asset in closure {
        if let PortableDefinition::Texture { path } | PortableDefinition::Model { path, .. } =
            &asset.definition
        {
            let full = format!("{directory}{path}");
            let bytes = source.files.get(&full).ok_or_else(|| {
                format!(
                    "{}: asset '{}' references missing file '{path}' (resolved '{full}')",
                    source.path, asset.id
                )
            })?;
            members.insert(
                asset.id.clone(),
                AdmittedContent {
                    path: full,
                    sha256: sha256(bytes),
                    bytes: bytes.clone(),
                    transient: source.transient,
                    files: source.files.clone(),
                },
            );
        }
    }
    Ok(Asset {
        document,
        selected,
        members,
    })
}
pub(super) unsafe extern "C" fn load(
    context: *mut c_void,
    request: *const NativePortableAssetLoadRequest,
    result: *mut NativePortableAssetHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let request = unsafe { &*request };
    let selection =
        unsafe { borrowed_utf8(request.asset_id.bytes, request.asset_id.len, "asset ID") };
    let candidate = selection.map_err(|e| e.to_string()).and_then(|id| {
        let source = bridge
            .references
            .get(&request.descriptor.value)
            .cloned()
            .ok_or_else(|| "descriptor content reference is no longer available".to_owned())?;
        build(source, id.to_owned())
    });
    match candidate {
        Ok(asset) => {
            let value = bridge.portable.id();
            bridge.portable.assets.insert(value, asset);
            unsafe {
                *result = NativePortableAssetHandle { value };
            }
            ABI_OK
        }
        Err(message) => bridge.portable.fail("LoadPortableAsset", message, error),
    }
}
pub(super) unsafe extern "C" fn destroy(
    context: *mut c_void,
    handle: NativePortableAssetHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    i32::from(bridge.portable.assets.remove(&handle.value).is_some())
}
pub(super) unsafe extern "C" fn open_member(
    context: *mut c_void,
    request: *const NativePortableAssetMemberRequest,
    result: *mut NativeContentReferenceHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let request = unsafe { &*request };
    let id = unsafe { borrowed_utf8(request.member_id.bytes, request.member_id.len, "member ID") };
    let member = id.map_err(|e| e.to_string()).and_then(|id| {
        bridge
            .portable
            .assets
            .get(&request.asset.value)
            .and_then(|a| a.members.get(id))
            .cloned()
            .ok_or_else(|| format!("missing file member '{id}' in portable asset"))
    });
    match member {
        Ok(member) => match bridge.retain(member) {
            Some(handle) => {
                unsafe {
                    *result = handle;
                }
                ABI_OK
            }
            None => 0,
        },
        Err(message) => bridge
            .portable
            .fail("OpenPortableAssetMember", message, error),
    }
}
fn vec2(v: [u32; 2]) -> NativeVec2 {
    NativeVec2 {
        x: v[0] as f32,
        y: v[1] as f32,
    }
}
fn point(v: [f32; 2]) -> NativeVec2 {
    NativeVec2 { x: v[0], y: v[1] }
}
impl Readout {
    fn relationship(
        &mut self,
        member: &str,
        kind: NativePortableAssetRelationshipKind,
        name: &str,
        target: &str,
    ) {
        self.relationships.push(NativePortableAssetRelationship {
            member_id: self.text.copy(member),
            kind,
            name: self.text.copy(name),
            target: self.text.copy(target),
        });
    }
    fn build(asset: &Asset) -> Self {
        let mut out = Self::default();
        // Resolution succeeded when the immutable asset was loaded.
        for member in asset
            .document
            .resolve(&asset.selected)
            .expect("retained resolved descriptor")
        {
            let (kind, path) = match &member.definition {
                PortableDefinition::Draft { .. } => unreachable!("resolved members are complete"),
                PortableDefinition::Attachment {
                    target,
                    child,
                    joint,
                    convention,
                    translation,
                    rotation,
                    scale,
                } => {
                    let vec3 = |v: &[f32; 3]| NativeVec3 {
                        x: v[0],
                        y: v[1],
                        z: v[2],
                    };
                    out.attachments.push(NativePortableMeshAttachment {
                        target_id: out.text.copy(target),
                        child_id: out.text.copy(child),
                        joint: out.text.copy(joint),
                        convention: out.text.copy(convention),
                        transform: NativeTransform {
                            translation: vec3(translation),
                            rotation: NativeQuat {
                                x: rotation[0],
                                y: rotation[1],
                                z: rotation[2],
                                w: rotation[3],
                            },
                            scale: vec3(scale),
                        },
                    });
                    (NativePortableAssetKind::Attachment, "")
                }
                PortableDefinition::Texture { path } => {
                    (NativePortableAssetKind::Texture, path.as_str())
                }
                PortableDefinition::Model {
                    path,
                    materials,
                    clips,
                } => {
                    for (name, target) in materials {
                        out.relationship(
                            &member.id,
                            NativePortableAssetRelationshipKind::MaterialSlot,
                            name,
                            target,
                        );
                    }
                    for (name, target) in clips {
                        out.relationship(
                            &member.id,
                            NativePortableAssetRelationshipKind::AnimationClip,
                            name,
                            target,
                        );
                    }
                    (NativePortableAssetKind::Model, path.as_str())
                }
                PortableDefinition::Material { textures } => {
                    for (name, target) in textures {
                        out.relationship(
                            &member.id,
                            NativePortableAssetRelationshipKind::TextureRole,
                            name,
                            target,
                        );
                    }
                    (NativePortableAssetKind::Material, "")
                }
                PortableDefinition::Sprite {
                    frames,
                    animations,
                    directions,
                } => {
                    for frame in frames {
                        let (origin, extent) = frame.rectangle().expect("resolved frame");
                        out.frames.push(NativePortableSpriteFrame {
                            id: out.text.copy(&frame.id),
                            texture_id: out.text.copy(&frame.texture),
                            origin: vec2(origin),
                            extent: vec2(extent),
                            canvas: vec2(frame.canvas),
                            trim: vec2(frame.trim),
                            pivot: point(frame.pivot),
                        });
                        for (name, position) in &frame.anchors {
                            out.anchors.push(NativePortableSpriteAnchor {
                                frame_id: out.text.copy(&frame.id),
                                name: out.text.copy(name),
                                position: point(*position),
                            });
                        }
                    }
                    for (name, animation) in animations {
                        for (frame, duration) in animation
                            .frames
                            .iter()
                            .zip(animation.durations().expect("resolved timing"))
                        {
                            out.animation_frames
                                .push(NativePortableSpriteAnimationFrame {
                                    animation: out.text.copy(name),
                                    frame_id: out.text.copy(frame),
                                    duration_seconds: duration,
                                    looping: animation.looping,
                                });
                        }
                    }
                    if let Some(directions) = directions {
                        for sector in &directions.sectors {
                            out.directions.push(NativePortableSpriteDirection {
                                id: out.text.copy(&sector.id),
                                yaw_degrees: sector.yaw_degrees,
                            });
                        }
                        for (action, mapping) in &directions.actions {
                            for (direction, animation) in mapping {
                                out.actions.push(NativePortableSpriteAction {
                                    action: out.text.copy(action),
                                    direction: out.text.copy(direction),
                                    animation: out.text.copy(animation),
                                });
                            }
                        }
                    }
                    (NativePortableAssetKind::Sprite, "")
                }
            };
            out.members.push(NativePortableAssetMember {
                id: out.text.copy(&member.id),
                kind,
                path: out.text.copy(path),
            });
        }
        out
    }
}
pub(super) unsafe extern "C" fn read(
    context: *mut c_void,
    handle: NativePortableAssetHandle,
    result: *mut NativePortableAssetReadoutLease,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    let value = bridge.portable.id();
    let Some(asset) = bridge.portable.assets.get(&handle.value) else {
        return 0;
    };
    let mut lease = Readout::build(asset);
    let convention = match &asset
        .document
        .asset(&asset.selected)
        .expect("resolved asset")
        .definition
    {
        PortableDefinition::Sprite {
            directions: Some(d),
            ..
        } => d.convention.as_str(),
        _ => "",
    };
    let output = NativePortableAssetReadoutLease {
        handle: NativePortableAssetReadoutLeaseHandle { value },
        asset_id: lease.text.copy(&asset.selected),
        direction_convention: lease.text.copy(convention),
        members: lease.members.as_ptr(),
        members_len: lease.members.len(),
        frames: lease.frames.as_ptr(),
        frames_len: lease.frames.len(),
        anchors: lease.anchors.as_ptr(),
        anchors_len: lease.anchors.len(),
        animation_frames: lease.animation_frames.as_ptr(),
        animation_frames_len: lease.animation_frames.len(),
        directions: lease.directions.as_ptr(),
        directions_len: lease.directions.len(),
        actions: lease.actions.as_ptr(),
        actions_len: lease.actions.len(),
        relationships: lease.relationships.as_ptr(),
        relationships_len: lease.relationships.len(),
        attachments: lease.attachments.as_ptr(),
        attachments_len: lease.attachments.len(),
    };
    bridge.portable.leases.insert(value, lease);
    unsafe {
        *result = output;
    }
    ABI_OK
}
pub(super) unsafe extern "C" fn destroy_readout(
    context: *mut c_void,
    handle: NativePortableAssetReadoutLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    i32::from(bridge.portable.leases.remove(&handle.value).is_some())
}
pub(super) unsafe extern "C" fn destroy_diagnostic(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeContentBridge>() };
    i32::from(bridge.portable.errors.remove(&handle.value).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(prefix: &str) -> AdmittedContent {
        let body: Arc<[u8]> = Arc::from(br#"{"schemaVersion":1,"assets":[{"id":"body","kind":"model","path":"body.glb","clips":{"idle":"Idle"}},{"id":"draft-no-kind"},{"id":"draft-required-field","kind":"attachment"}]}"#.as_slice());
        let path = format!("{prefix}asset.json");
        let files = Arc::new(BTreeMap::from([
            (path.clone(), body.clone()),
            (
                format!("{prefix}body.glb"),
                Arc::from(b"model bytes".as_slice()),
            ),
        ]));
        AdmittedContent {
            path,
            sha256: sha256(&body),
            bytes: body,
            transient: false,
            files,
        }
    }
    #[test]
    fn loose_and_bundle_contexts_resolve_equivalent_semantics_and_retain_members() {
        let loose = build(source("loose/"), "body".into()).unwrap();
        let bundled = build(source("bundles/models/"), "body".into()).unwrap();
        assert_eq!(loose.document, bundled.document);
        assert_eq!(loose.members["body"].bytes, bundled.members["body"].bytes);
        let mut bridge = RuntimeContentBridge::new(BTreeMap::new());
        let reference = bridge.retain(source("bundle/")).unwrap();
        let api = super::super::api(&mut bridge);
        let mut asset = NativePortableAssetHandle::default();
        let mut error = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                (api.load_portable_asset)(
                    api.context,
                    &NativePortableAssetLoadRequest {
                        descriptor: reference,
                        asset_id: slice("body"),
                    },
                    &mut asset,
                    &mut error,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_reference)(api.context, reference) },
            ABI_OK
        );
        let mut member = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe {
                (api.open_portable_asset_member)(
                    api.context,
                    &NativePortableAssetMemberRequest {
                        asset,
                        member_id: slice("body"),
                    },
                    &mut member,
                    &mut error,
                )
            },
            ABI_OK
        );
        assert_eq!(
            unsafe { (api.destroy_portable_asset)(api.context, asset) },
            ABI_OK
        );
        assert_eq!(&*bridge.retained_bytes(member).unwrap(), b"model bytes");
        assert_eq!(
            unsafe { (api.destroy_reference)(api.context, member) },
            ABI_OK
        );
        assert!(bridge.portable.assets.is_empty());
    }
    #[test]
    fn reference_context_does_not_fall_back_to_unrelated_catalog_files() {
        let mut isolated = source("bundle/");
        isolated.files = Arc::new(BTreeMap::new());
        let error = match build(isolated, "body".into()) {
            Ok(_) => panic!("missing member accepted"),
            Err(e) => e,
        };
        assert!(error.contains("body.glb"));
        assert!(error.contains("bundle/body.glb"));
    }
}
