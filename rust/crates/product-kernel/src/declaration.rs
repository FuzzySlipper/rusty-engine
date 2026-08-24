use product_model::{CapabilityKind, CapabilityMetadata, ProductKernelCapabilityDescriptor};

/// Product Model's exact UTF-8 byte bound for source-linked contract text.
pub const MAX_PRODUCT_KERNEL_CONTRACT_TEXT_BYTES: usize =
    product_model::MAX_CAPABILITY_PROVENANCE_BYTES;

/// Product Model's exact identity byte bound for source-linked identities.
pub const MAX_PRODUCT_KERNEL_IDENTITY_BYTES: usize = product_model::MAX_IDENTITY_BYTES;

/// A concrete downstream type contract associated with one Product Kernel
/// owner. The associated data types remain downstream-owned; this marker is
/// only a static source link and never stores or calls a value of any type.
pub trait ProductKernelCapabilityContract {
    type Snapshot;
    type Request;
    type Result;
    type Error;

    /// Current source-level identity of the concrete contract.
    const TYPE_ID: &'static str;
    /// Full Product Model target, including the `kernel.` namespace.
    const TARGET: &'static str;
    /// The closed Product Model kind for this owner.
    const KIND: CapabilityKind;
}

/// A closed durable product schema identity. Schemas are source-linked
/// declarations used by offline assembly/migration tooling; they are not live
/// Product Model capability targets.
pub trait ProductKernelSchemaContract {
    const TYPE_ID: &'static str;
}

/// A closed offline migration contract. Migration functions remain downstream
/// owned and are never admitted into a live schedule or runtime capability
/// catalog.
pub trait ProductKernelMigrationContract {
    const TYPE_ID: &'static str;
    const FROM_SCHEMA: &'static str;
    const TO_SCHEMA: &'static str;
}

/// One source-linked schema identity included in the deterministic Product
/// Kernel contract export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductKernelSchemaDescriptor {
    identity: &'static str,
    contract_type: &'static str,
    contract_identity: &'static str,
}

impl ProductKernelSchemaDescriptor {
    pub const fn new(
        identity: &'static str,
        contract_type: &'static str,
        contract_identity: &'static str,
    ) -> Self {
        Self {
            identity,
            contract_type,
            contract_identity,
        }
    }

    pub const fn identity(self) -> &'static str {
        self.identity
    }

    pub const fn contract_type(self) -> &'static str {
        self.contract_type
    }

    pub const fn contract_identity(self) -> &'static str {
        self.contract_identity
    }
}

/// One source-linked offline migration identity included in the deterministic
/// Product Kernel contract export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductKernelMigrationDescriptor {
    identity: &'static str,
    from_schema: &'static str,
    to_schema: &'static str,
    contract_type: &'static str,
    contract_identity: &'static str,
    contract_from_schema: &'static str,
    contract_to_schema: &'static str,
}

impl ProductKernelMigrationDescriptor {
    pub const fn new(
        identity: &'static str,
        from_schema: &'static str,
        to_schema: &'static str,
        contract_type: &'static str,
        contract_identity: &'static str,
        contract_from_schema: &'static str,
        contract_to_schema: &'static str,
    ) -> Self {
        Self {
            identity,
            from_schema,
            to_schema,
            contract_type,
            contract_identity,
            contract_from_schema,
            contract_to_schema,
        }
    }

    pub const fn identity(self) -> &'static str {
        self.identity
    }

    pub const fn from_schema(self) -> &'static str {
        self.from_schema
    }

    pub const fn to_schema(self) -> &'static str {
        self.to_schema
    }

    pub const fn contract_type(self) -> &'static str {
        self.contract_type
    }

    pub const fn contract_identity(self) -> &'static str {
        self.contract_identity
    }

    pub const fn contract_from_schema(self) -> &'static str {
        self.contract_from_schema
    }

    pub const fn contract_to_schema(self) -> &'static str {
        self.contract_to_schema
    }
}

/// One generated source-linked entry. It contains metadata only; it has no
/// executable owner, service handle, callback, or erased value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductKernelCapabilityEntry<O> {
    owner: O,
    identity: &'static str,
    target: &'static str,
    contract_target: &'static str,
    contract_type: &'static str,
    metadata: CapabilityMetadata,
    contract_kind: CapabilityKind,
}

impl<O: Copy> ProductKernelCapabilityEntry<O> {
    pub const fn new(
        owner: O,
        identity: &'static str,
        target: &'static str,
        contract_target: &'static str,
        contract_type: &'static str,
        metadata: CapabilityMetadata,
        contract_kind: CapabilityKind,
    ) -> Self {
        Self {
            owner,
            identity,
            target,
            contract_target,
            contract_type,
            metadata,
            contract_kind,
        }
    }

    pub const fn owner(self) -> O {
        self.owner
    }

    pub const fn identity(self) -> &'static str {
        self.identity
    }

    pub const fn target(self) -> &'static str {
        self.target
    }

    pub const fn contract_type(self) -> &'static str {
        self.contract_type
    }

    pub const fn contract_target(self) -> &'static str {
        self.contract_target
    }

    pub const fn metadata(self) -> CapabilityMetadata {
        self.metadata
    }

    pub const fn contract_kind(self) -> CapabilityKind {
        self.contract_kind
    }
}

/// A generated owner identity that can be matched by downstream code without
/// introducing a dynamic dispatch key or a string invoke path.
pub trait ProductKernelOwner: Copy + Eq + 'static {
    fn identity(self) -> &'static str;
    fn target(self) -> &'static str;
    fn contract_type(self) -> &'static str;
    fn kind(self) -> CapabilityKind;
    fn metadata(self) -> CapabilityMetadata;
    fn entry(self) -> ProductKernelCapabilityEntry<Self>;

    fn selection(self, binding_id: impl Into<String>) -> ProductKernelSelection<Self> {
        ProductKernelSelection::new(binding_id, self, self.contract_type())
    }
}

/// The one declaration interface consumed by [`crate::ProductAssembly`]. A
/// macro expansion is the normal implementation; the trait is public so a
/// generated downstream declaration can be tested without runtime discovery.
pub trait ProductKernelDeclaration {
    type Owner: ProductKernelOwner;

    fn entries() -> &'static [ProductKernelCapabilityEntry<Self::Owner>];
    fn descriptors() -> &'static [ProductKernelCapabilityDescriptor];
    fn schemas() -> &'static [ProductKernelSchemaDescriptor];
    fn migrations() -> &'static [ProductKernelMigrationDescriptor];
    fn contract_json() -> Result<String, crate::ProductAssemblyError>;

    fn contract_typescript() -> Result<String, crate::ProductAssemblyError>
    where
        Self: Sized,
    {
        crate::render_contract_typescript::<Self>()
    }
}

/// A downstream-selected binding and its expected closed owner type identity.
///
/// The string is only a pre-start contract assertion. It is never interpreted
/// as a method name or used to locate an executable owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductKernelSelection<O> {
    binding_id: String,
    owner: O,
    contract_type: String,
}

impl<O: ProductKernelOwner> ProductKernelSelection<O> {
    pub fn new(binding_id: impl Into<String>, owner: O, contract_type: impl Into<String>) -> Self {
        Self {
            binding_id: binding_id.into(),
            owner,
            contract_type: contract_type.into(),
        }
    }

    pub fn binding_id(&self) -> &str {
        &self.binding_id
    }

    pub const fn owner(&self) -> O {
        self.owner
    }

    pub fn contract_type(&self) -> &str {
        &self.contract_type
    }
}

/// Declare one closed downstream Product Kernel catalog.
///
/// Each entry names its concrete Rust contract type. The macro derives the
/// full `kernel.` target, immutable Product Model descriptor, typed owner enum,
/// and deterministic JSON renderer from this single source declaration.
#[macro_export]
macro_rules! product_kernel_declaration {
    (
        declaration: $declaration:ident,
        owner: $owner:ident,
        capabilities: [
            $(
                $variant:ident => $contract:ty {
                    identity: $identity:literal,
                    kind: $kind:expr,
                    uses: $uses:expr,
                    availability: $availability:expr,
                    reads: $reads:expr,
                    writes: $writes:expr,
                    maximum_compact_json_payload_bytes: $budget:expr,
                    owner: $provenance_owner:literal,
                    source: $source:literal,
                    logical_path: $logical_path:literal
                }
            ),+ $(,)?
        ],
        schemas: [
            $(
                $schema_variant:ident => $schema_contract:ty {
                    identity: $schema_identity:literal
                }
            ),* $(,)?
        ],
        migrations: [
            $(
                $migration_variant:ident => $migration_contract:ty {
                    identity: $migration_identity:literal,
                    from: $migration_from:literal,
                    to: $migration_to:literal
                }
            ),* $(,)?
        ]
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $owner {
            $($variant,)+
        }

        pub struct $declaration;

        impl $declaration {
        const ENTRIES: &[$crate::ProductKernelCapabilityEntry<$owner>] = &[
            $(
                $crate::ProductKernelCapabilityEntry::new(
                    $owner::$variant,
                    $identity,
                    concat!("kernel.", $identity),
                    <$contract as $crate::ProductKernelCapabilityContract>::TARGET,
                    <$contract as $crate::ProductKernelCapabilityContract>::TYPE_ID,
                    $crate::product_model::CapabilityMetadata::new(
                        $kind,
                        $uses,
                        $availability,
                        $crate::product_model::CapabilityAccess::new($reads, $writes),
                        $crate::product_model::CapabilityBudget::new($budget),
                        $crate::product_model::CapabilityProvenance::new(
                            $provenance_owner,
                            $source,
                            $logical_path,
                        ),
                    ),
                    <$contract as $crate::ProductKernelCapabilityContract>::KIND,
                ),
            )+
        ];

        const DESCRIPTORS: &[$crate::product_model::ProductKernelCapabilityDescriptor] = &[
            $(
                $crate::product_model::ProductKernelCapabilityDescriptor::new(
                    $identity,
                    $crate::product_model::CapabilityMetadata::new(
                        $kind,
                        $uses,
                        $availability,
                        $crate::product_model::CapabilityAccess::new($reads, $writes),
                        $crate::product_model::CapabilityBudget::new($budget),
                        $crate::product_model::CapabilityProvenance::new(
                            $provenance_owner,
                            $source,
                            $logical_path,
                        ),
                    ),
                ),
            )+
        ];

        const SCHEMAS: &[$crate::ProductKernelSchemaDescriptor] = &[
            $(
                $crate::ProductKernelSchemaDescriptor::new(
                    $schema_identity,
                    <$schema_contract as $crate::ProductKernelSchemaContract>::TYPE_ID,
                    <$schema_contract as $crate::ProductKernelSchemaContract>::TYPE_ID,
                ),
            )*
        ];

        const MIGRATIONS: &[$crate::ProductKernelMigrationDescriptor] = &[
            $(
                $crate::ProductKernelMigrationDescriptor::new(
                    $migration_identity,
                    $migration_from,
                    $migration_to,
                    <$migration_contract as $crate::ProductKernelMigrationContract>::TYPE_ID,
                    <$migration_contract as $crate::ProductKernelMigrationContract>::TYPE_ID,
                    <$migration_contract as $crate::ProductKernelMigrationContract>::FROM_SCHEMA,
                    <$migration_contract as $crate::ProductKernelMigrationContract>::TO_SCHEMA,
                ),
            )*
        ];
        }

        impl $crate::ProductKernelOwner for $owner {
            fn identity(self) -> &'static str {
                match self {
                    $(Self::$variant => $identity,)+
                }
            }

            fn target(self) -> &'static str {
                match self {
                    $(Self::$variant => concat!("kernel.", $identity),)+
                }
            }

            fn contract_type(self) -> &'static str {
                match self {
                    $(Self::$variant => <$contract as $crate::ProductKernelCapabilityContract>::TYPE_ID,)+
                }
            }

            fn kind(self) -> $crate::product_model::CapabilityKind {
                match self {
                    $(Self::$variant => $kind,)+
                }
            }

            fn metadata(self) -> $crate::product_model::CapabilityMetadata {
                match self {
                    $(
                        Self::$variant =>
                            <$owner as $crate::ProductKernelOwner>::entry(self).metadata(),
                    )+
                }
            }

            fn entry(self) -> $crate::ProductKernelCapabilityEntry<Self> {
                match self {
                    $(
                        Self::$variant => $crate::ProductKernelCapabilityEntry::new(
                            Self::$variant,
                            $identity,
                            concat!("kernel.", $identity),
                            <$contract as $crate::ProductKernelCapabilityContract>::TARGET,
                            <$contract as $crate::ProductKernelCapabilityContract>::TYPE_ID,
                            $crate::product_model::CapabilityMetadata::new(
                                $kind,
                                $uses,
                                $availability,
                                $crate::product_model::CapabilityAccess::new($reads, $writes),
                                $crate::product_model::CapabilityBudget::new($budget),
                                $crate::product_model::CapabilityProvenance::new(
                                    $provenance_owner,
                                    $source,
                                    $logical_path,
                                ),
                            ),
                            <$contract as $crate::ProductKernelCapabilityContract>::KIND,
                        ),
                    )+
                }
            }
        }

        #[allow(dead_code)]
        impl $owner {
            pub const fn all() -> &'static [Self] {
                &[$(Self::$variant,)+]
            }

            pub fn entry(self) -> $crate::ProductKernelCapabilityEntry<Self> {
                <$owner as $crate::ProductKernelOwner>::entry(self)
            }

            pub fn selection(self, binding_id: impl Into<String>) -> $crate::ProductKernelSelection<Self> {
                $crate::ProductKernelSelection::new(
                    binding_id,
                    self,
                    <$owner as $crate::ProductKernelOwner>::contract_type(self),
                )
            }
        }

        impl $crate::ProductKernelDeclaration for $declaration {
            type Owner = $owner;

            fn entries() -> &'static [$crate::ProductKernelCapabilityEntry<Self::Owner>] {
                $declaration::ENTRIES
            }

            fn descriptors() -> &'static [$crate::product_model::ProductKernelCapabilityDescriptor] {
                $declaration::DESCRIPTORS
            }

            fn schemas() -> &'static [$crate::ProductKernelSchemaDescriptor] {
                $declaration::SCHEMAS
            }

            fn migrations() -> &'static [$crate::ProductKernelMigrationDescriptor] {
                $declaration::MIGRATIONS
            }

            fn contract_json() -> Result<String, $crate::ProductAssemblyError> {
                $crate::render_contract_json::<$declaration>()
            }
        }
    };
}
