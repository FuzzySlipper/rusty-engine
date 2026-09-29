use std::collections::BTreeMap;

use render_model::RenderHandle;

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

    /// Whether `count` more handles fit in the namespace.
    pub(crate) fn can_allocate(&self, count: usize) -> bool {
        self.next_local
            .checked_add(count as u64)
            .is_some_and(|end| end <= LOCAL_HANDLE_MASK + 1)
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
