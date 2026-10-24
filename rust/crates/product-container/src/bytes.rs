use std::{
    fmt,
    ops::{Deref, Range},
    sync::{Arc, Weak},
};

use crate::Container;

/// A file's bytes, or a range of them, immutable and cheap to clone: shared
/// from the heap, or a raw container entry served from its map, which keeps
/// the container open. Mapped bytes take no heap; whoever must own a heap copy
/// asks for [`Self::shared`].
#[derive(Clone)]
pub struct ContentBytes {
    owner: Owner,
    range: Range<usize>,
}

/// A heap body sits behind its own `Arc`: a weak handle pins the allocation
/// it points at, which for the outer `Arc` is one pointer, not the file.
#[derive(Clone)]
enum Owner {
    Heap(Arc<Arc<[u8]>>),
    Mapped(Arc<Container>),
}

impl ContentBytes {
    pub(crate) fn mapped(container: Arc<Container>, range: Range<usize>) -> Self {
        Self {
            owner: Owner::Mapped(container),
            range,
        }
    }

    /// `range` of these bytes, sharing their storage.
    ///
    /// # Panics
    /// When `range` is not within the bytes.
    pub fn slice(&self, range: Range<usize>) -> Self {
        assert!(range.start <= range.end && range.end <= self.len());
        Self {
            owner: self.owner.clone(),
            range: self.range.start + range.start..self.range.start + range.end,
        }
    }

    /// The bytes on the heap: the same allocation for a whole heap body, a
    /// copy otherwise.
    pub fn shared(&self) -> Arc<[u8]> {
        match &self.owner {
            Owner::Heap(bytes) if self.range == (0..bytes.len()) => Arc::clone(&**bytes),
            _ => Arc::from(&**self),
        }
    }

    /// Whether both are the same bytes in memory, not merely equal bytes.
    pub fn same(&self, other: &Self) -> bool {
        self.range == other.range
            && match (&self.owner, &other.owner) {
                (Owner::Heap(a), Owner::Heap(b)) => Arc::ptr_eq(&**a, &**b),
                (Owner::Mapped(a), Owner::Mapped(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
    }

    /// A handle that does not keep the bytes alive.
    pub fn downgrade(&self) -> WeakContentBytes {
        WeakContentBytes {
            owner: match &self.owner {
                Owner::Heap(bytes) => WeakOwner::Heap(Arc::downgrade(bytes)),
                Owner::Mapped(container) => WeakOwner::Mapped(Arc::downgrade(container)),
            },
            range: self.range.clone(),
        }
    }
}

impl Deref for ContentBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        let whole: &[u8] = match &self.owner {
            Owner::Heap(bytes) => &bytes[..],
            Owner::Mapped(container) => container.map(),
        };
        &whole[self.range.clone()]
    }
}

impl AsRef<[u8]> for ContentBytes {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl Default for ContentBytes {
    fn default() -> Self {
        Self::from(Arc::<[u8]>::from([]))
    }
}

impl From<Arc<[u8]>> for ContentBytes {
    fn from(bytes: Arc<[u8]>) -> Self {
        Self {
            range: 0..bytes.len(),
            owner: Owner::Heap(Arc::new(bytes)),
        }
    }
}

impl From<Vec<u8>> for ContentBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from(Arc::<[u8]>::from(bytes))
    }
}

impl From<&[u8]> for ContentBytes {
    fn from(bytes: &[u8]) -> Self {
        Self::from(Arc::<[u8]>::from(bytes))
    }
}

impl PartialEq for ContentBytes {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for ContentBytes {}

impl fmt::Debug for ContentBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let storage = match self.owner {
            Owner::Heap(_) => "heap",
            Owner::Mapped(_) => "mapped",
        };
        write!(f, "ContentBytes({} bytes, {storage})", self.len())
    }
}

/// [`ContentBytes`] that are gone once every strong copy of their storage is.
#[derive(Clone)]
pub struct WeakContentBytes {
    owner: WeakOwner,
    range: Range<usize>,
}

#[derive(Clone)]
enum WeakOwner {
    Heap(Weak<Arc<[u8]>>),
    Mapped(Weak<Container>),
}

impl WeakContentBytes {
    pub fn upgrade(&self) -> Option<ContentBytes> {
        Some(ContentBytes {
            owner: match &self.owner {
                WeakOwner::Heap(bytes) => Owner::Heap(bytes.upgrade()?),
                WeakOwner::Mapped(container) => Owner::Mapped(container.upgrade()?),
            },
            range: self.range.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_entries_are_served_from_the_map_and_compressed_ones_from_the_heap() {
        let directory = tempfile::tempdir().unwrap();
        let module = directory.path().join("module");
        std::fs::create_dir_all(&module).unwrap();
        // Bytes zstd cannot shrink stay raw.
        let noise: Vec<u8> = (0..64_u32)
            .map(|index| (index.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect();
        std::fs::write(module.join("raw.bin"), &noise).unwrap();
        std::fs::write(module.join("packed.json"), "[1,2,3],".repeat(4096)).unwrap();
        let out = directory.path().join("module.rpak");
        crate::pack_content(&module, &out, true).unwrap();
        let container = Arc::new(Container::open(&out).unwrap());

        let raw = container.content("raw.bin").unwrap();
        assert_eq!(&*raw, &noise[..]);
        assert!(format!("{raw:?}").contains("mapped"));
        assert!(raw.same(&container.content("raw.bin").unwrap()));
        let tail = raw.slice(60..64);
        assert_eq!(&*tail, &noise[60..]);
        assert!(!tail.same(&raw));

        let packed = container.content("packed.json").unwrap();
        assert!(format!("{packed:?}").contains("heap"));
        assert_eq!(&*packed, "[1,2,3],".repeat(4096).as_bytes());

        // A weak handle keeps neither the container nor a heap body, whose
        // allocation no weak handle reaches.
        let weak_raw = raw.downgrade();
        let weak_packed = packed.downgrade();
        assert_eq!(Arc::weak_count(&packed.shared()), 0);
        drop((raw, tail, packed));
        assert!(weak_packed.upgrade().is_none());
        assert!(weak_raw.upgrade().is_some());
        drop(container);
        assert!(weak_raw.upgrade().is_none());
    }
}
