use std::collections::BTreeMap;

use render_model::{
    NodeError, RenderDiff, RenderFrameDiff, RenderFrameError, RenderHandle, RenderNode,
};

const LOCAL_HANDLE_BITS: u32 = 40;
const LOCAL_HANDLE_MASK: u64 = (1_u64 << LOCAL_HANDLE_BITS) - 1;

/// Compact namespaces let independent projection owners share one retained
/// scene without an ambient/global allocator while every handle remains exact
/// in the JSON/JavaScript number border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderHandleNamespace(u8);

impl RenderHandleNamespace {
    pub const ENTITY: Self = Self(1);
    pub const VOXEL: Self = Self(2);
    pub const AUTHORED: Self = Self(3);
    pub const DEBUG: Self = Self(4);
    pub const PRESENTATION: Self = Self(5);
    pub const VOXEL_OBJECT: Self = Self(6);

    pub const fn new(value: u8) -> Option<Self> {
        if value == 0 {
            None
        } else {
            Some(Self(value))
        }
    }

    pub const fn raw(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct StableHandleRegistry<K> {
    namespace: RenderHandleNamespace,
    next_local: u64,
    handles: BTreeMap<K, RenderHandle>,
}

impl<K: Ord> StableHandleRegistry<K> {
    pub fn new(namespace: RenderHandleNamespace) -> Self {
        Self {
            namespace,
            next_local: 1,
            handles: BTreeMap::new(),
        }
    }

    pub fn handle_of(&self, key: &K) -> Option<RenderHandle> {
        self.handles.get(key).copied()
    }

    pub fn len(&self) -> usize {
        self.handles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    pub(crate) fn remove(&mut self, key: &K) -> Option<RenderHandle> {
        self.handles.remove(key)
    }
}

impl<K: Ord + Clone> StableHandleRegistry<K> {
    pub(crate) fn allocate(&mut self, key: K) -> Result<RenderHandle, HandleAllocationError> {
        if let Some(handle) = self.handles.get(&key) {
            return Ok(*handle);
        }
        if self.next_local > LOCAL_HANDLE_MASK {
            return Err(HandleAllocationError::NamespaceExhausted {
                namespace: self.namespace.raw(),
            });
        }
        let raw = (u64::from(self.namespace.raw()) << LOCAL_HANDLE_BITS) | self.next_local;
        self.next_local += 1;
        let handle = RenderHandle::new(raw);
        self.handles.insert(key, handle);
        Ok(handle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleAllocationError {
    NamespaceExhausted { namespace: u8 },
}

/// Cohesive retained projection for primitive nodes. It computes and validates
/// the whole frame against cloned state, then commits registry/snapshot state.
#[derive(Debug, Clone)]
pub struct RetainedNodeProjector<K> {
    registry: StableHandleRegistry<K>,
    last: BTreeMap<K, RenderNode>,
}

impl<K: Ord + Clone> RetainedNodeProjector<K> {
    pub fn new(namespace: RenderHandleNamespace) -> Self {
        Self {
            registry: StableHandleRegistry::new(namespace),
            last: BTreeMap::new(),
        }
    }

    pub fn project(
        &mut self,
        current: BTreeMap<K, RenderNode>,
    ) -> Result<RenderFrameDiff, RetainedProjectionError> {
        for node in current.values() {
            node.validate()
                .map_err(RetainedProjectionError::InvalidNode)?;
        }

        let mut registry = self.registry.clone();
        let mut operations = Vec::new();

        for (key, previous) in &self.last {
            let Some(node) = current.get(key) else {
                let handle = registry
                    .remove(key)
                    .expect("last retained node has a registered handle");
                operations.push(RenderDiff::Destroy { handle });
                continue;
            };
            if previous.geometry != node.geometry || previous.layer != node.layer {
                let old_handle = registry
                    .remove(key)
                    .expect("last retained node has a registered handle");
                operations.push(RenderDiff::Destroy { handle: old_handle });
                let handle = registry
                    .allocate(key.clone())
                    .map_err(RetainedProjectionError::Handle)?;
                operations.push(RenderDiff::Create {
                    handle,
                    parent: None,
                    node: node.clone(),
                });
                continue;
            }
            if previous != node {
                let handle = registry
                    .handle_of(key)
                    .expect("last retained node has a registered handle");
                operations.push(RenderDiff::Update {
                    handle,
                    transform: (previous.transform != node.transform).then_some(node.transform),
                    material: (previous.material != node.material).then_some(node.material),
                    visible: (previous.visible != node.visible).then_some(node.visible),
                    metadata: (previous.metadata != node.metadata).then(|| node.metadata.clone()),
                });
            }
        }

        for (key, node) in &current {
            if self.last.contains_key(key) {
                continue;
            }
            let handle = registry
                .allocate(key.clone())
                .map_err(RetainedProjectionError::Handle)?;
            operations.push(RenderDiff::Create {
                handle,
                parent: None,
                node: node.clone(),
            });
        }

        let frame =
            RenderFrameDiff::try_from_ops(operations).map_err(RetainedProjectionError::Frame)?;
        self.registry = registry;
        self.last = current;
        Ok(frame)
    }

    pub fn handle_of(&self, key: &K) -> Option<RenderHandle> {
        self.registry.handle_of(key)
    }

    pub fn retained_len(&self) -> usize {
        self.last.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RetainedProjectionError {
    InvalidNode(NodeError),
    Handle(HandleAllocationError),
    Frame(RenderFrameError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_model::{Geometry, Transform};

    #[test]
    fn stable_handles_minimal_updates_and_removal_are_deterministic() {
        let mut projector = RetainedNodeProjector::new(RenderHandleNamespace::ENTITY);
        let mut first = BTreeMap::new();
        first.insert(9_u64, RenderNode::new(Geometry::Cube));
        let created = projector.project(first.clone()).unwrap();
        let handle = projector.handle_of(&9).unwrap();
        assert_eq!(handle.raw(), (1_u64 << LOCAL_HANDLE_BITS) | 1);
        assert!(matches!(created.ops[0], RenderDiff::Create { .. }));

        first.get_mut(&9).unwrap().transform = Transform {
            translation: [2.0, 0.0, 0.0],
            ..Transform::IDENTITY
        };
        let updated = projector.project(first).unwrap();
        assert!(matches!(
            &updated.ops[0],
            RenderDiff::Update {
                handle: actual,
                transform: Some(_),
                material: None,
                visible: None,
                metadata: None,
            } if *actual == handle
        ));

        let removed = projector.project(BTreeMap::new()).unwrap();
        assert_eq!(removed.ops, vec![RenderDiff::Destroy { handle }]);
    }

    #[test]
    fn invalid_input_does_not_mutate_projector_state() {
        let mut projector = RetainedNodeProjector::new(RenderHandleNamespace::ENTITY);
        let mut invalid = BTreeMap::new();
        let mut node = RenderNode::new(Geometry::Cube);
        node.transform.translation[0] = f32::NAN;
        invalid.insert(1_u64, node);
        assert!(projector.project(invalid).is_err());
        assert_eq!(projector.retained_len(), 0);
        assert_eq!(projector.handle_of(&1), None);
    }
}
