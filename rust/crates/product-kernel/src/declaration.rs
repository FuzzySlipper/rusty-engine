use product_model::{CapabilityKind, CapabilityMetadata, ProductKernelCapabilityDescriptor};
use runtime_standard_capabilities::BoundStandardCapabilities;

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

    /// Source-only executable links emitted by a declaration macro. The
    /// default is intentionally empty so metadata-only declarations remain
    /// useful to authoring and migration tooling; a generated assembly must
    /// call the execution validator before publishing a live schedule.
    fn execution_links() -> &'static [crate::ProductKernelExecutionLink<Self::Owner>] {
        &[]
    }

    fn contract_typescript() -> Result<String, crate::ProductAssemblyError>
    where
        Self: Sized,
    {
        crate::render_contract_typescript::<Self>()
    }
}

/// One immutable byte resource made available to a source-linked Product
/// Runtime definition.  The generated Product Assembly constructs these
/// values from `include_bytes!`; a definition may inspect them while it
/// creates its concrete adapter, but it never receives a product-root path or
/// a filesystem handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductRuntimeResource<'a> {
    path: &'static str,
    bytes: &'a [u8],
}

impl<'a> ProductRuntimeResource<'a> {
    pub const fn new(path: &'static str, bytes: &'a [u8]) -> Self {
        Self { path, bytes }
    }

    pub const fn path(self) -> &'static str {
        self.path
    }

    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Immutable resources supplied to the fixed source-linked runtime
/// definition.  The composition bytes and bundle resources are deliberately
/// separate from authored source and are the only generated product inputs a
/// runtime definition can inspect at construction time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductRuntimeResources<'a> {
    compiled_composition: &'a [u8],
    resources: &'a [ProductRuntimeResource<'a>],
}

/// Typed rejection emitted when a source-linked Product Kernel does not elect
/// to own a compiled Engine standard-capability plan. This keeps a linked
/// Engine capability from silently degrading into declaration-only runtime
/// data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductKernelStandardCapabilityBindError {
    UnhandledObservePairs { received: usize },
    UnexpectedObservePairs { expected: usize, received: usize },
}

impl std::fmt::Display for ProductKernelStandardCapabilityBindError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnhandledObservePairs { received } => write!(
                formatter,
                "Product Kernel did not bind {received} compiled engine.runtime.observe-pairs plan(s)"
            ),
            Self::UnexpectedObservePairs { expected, received } => write!(
                formatter,
                "Product Kernel expected {expected} compiled engine.runtime.observe-pairs plan(s), received {received}"
            ),
        }
    }
}

impl std::error::Error for ProductKernelStandardCapabilityBindError {}

impl<'a> ProductRuntimeResources<'a> {
    pub const fn new(
        compiled_composition: &'a [u8],
        resources: &'a [ProductRuntimeResource<'a>],
    ) -> Self {
        Self {
            compiled_composition,
            resources,
        }
    }

    pub const fn compiled_composition(self) -> &'a [u8] {
        self.compiled_composition
    }

    pub const fn resources(self) -> &'a [ProductRuntimeResource<'a>] {
        self.resources
    }

    pub fn resource(self, path: &str) -> Option<&'a [u8]> {
        self.resources
            .iter()
            .find(|resource| resource.path() == path)
            .map(|resource| resource.bytes())
    }
}

/// One static owner selection exposed by the fixed Product Runtime
/// definition.  It is source-linked metadata, not a runtime dispatch key.
/// The generated adapter still matches its concrete owner enum or target
/// strings directly in ordinary Rust code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductKernelRuntimeSelection {
    identity: &'static str,
    target: &'static str,
    contract_type: &'static str,
    kind: CapabilityKind,
}

impl ProductKernelRuntimeSelection {
    pub const fn new(
        identity: &'static str,
        target: &'static str,
        contract_type: &'static str,
        kind: CapabilityKind,
    ) -> Self {
        Self {
            identity,
            target,
            contract_type,
            kind,
        }
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

    pub const fn kind(self) -> CapabilityKind {
        self.kind
    }
}

/// One static mutation publication descriptor exposed by the fixed Product
/// Runtime definition.  It may name a Product Kernel or Engine operation;
/// the generated root passes the closed descriptor through the ordinary
/// `runtime-mutation` compiler before a live composition exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductKernelRuntimeMutationDescriptor {
    binding_id: &'static str,
    target: &'static str,
    publication_domain: &'static str,
    owner: &'static str,
    operation_type: &'static str,
}

impl ProductKernelRuntimeMutationDescriptor {
    pub const fn new(
        binding_id: &'static str,
        target: &'static str,
        publication_domain: &'static str,
        owner: &'static str,
        operation_type: &'static str,
    ) -> Self {
        Self {
            binding_id,
            target,
            publication_domain,
            owner,
            operation_type,
        }
    }

    pub const fn binding_id(self) -> &'static str {
        self.binding_id
    }

    pub const fn target(self) -> &'static str {
        self.target
    }

    pub const fn publication_domain(self) -> &'static str {
        self.publication_domain
    }

    pub const fn owner(self) -> &'static str {
        self.owner
    }

    pub const fn operation_type(self) -> &'static str {
        self.operation_type
    }
}

/// Fixed source-linked runtime definition convention consumed by generated
/// Product Assembly.
///
/// A product's `kernel/entry.rs` must expose the concrete type
/// `RustyProductRuntime` implementing this trait.  The associated adapter and
/// product fact types remain downstream-owned concrete types.  Engine never
/// stores this definition, calls a function pointer, performs a registry
/// lookup, or erases one of the associated types; generated source invokes
/// `build` directly and places the returned adapter into
/// `runtime_composition::RuntimeComposition`.
pub trait ProductKernelRuntimeDefinition {
    type Adapter;
    type Error;
    type ProductState;
    type ObserverComponent;
    type TargetComponent;

    /// Static descriptor aggregate corresponding to the Product Assembly
    /// capability slice.
    fn capabilities() -> &'static [ProductKernelCapabilityDescriptor];

    /// Static concrete owner selections used by the product adapter.
    fn selections() -> &'static [ProductKernelRuntimeSelection];

    /// Static mutation publication descriptors used to compile the
    /// instance-owned mutation catalog.  The descriptors contain no handler
    /// or mutable state and may name Engine or Product Kernel targets.
    fn mutation_descriptors() -> &'static [ProductKernelRuntimeMutationDescriptor];

    /// Builds one concrete product adapter from immutable generated bytes.
    fn build(resources: ProductRuntimeResources<'_>) -> Result<Self::Adapter, Self::Error>;

    /// Receives the bounded, Assembly-compiled plans for the named Engine
    /// standard mechanisms used by this product. A product must override this
    /// hook to retain and execute any supplied plan; the default is a typed,
    /// actionable rejection rather than silently accepting declaration-only
    /// capability linkage.
    fn bind_standard_capabilities(
        _adapter: &mut Self::Adapter,
        plans: BoundStandardCapabilities,
    ) -> Result<(), ProductKernelStandardCapabilityBindError> {
        if plans.is_empty() {
            Ok(())
        } else {
            Err(
                ProductKernelStandardCapabilityBindError::UnhandledObservePairs {
                    received: plans.observe_pairs().len(),
                },
            )
        }
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
                    $(, execution: $execution_kind:ident => $execute:path)?
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

        const EXECUTION_LINKS: &[$crate::ProductKernelExecutionLink<$owner>] = &[
            $(
                $(
                    $crate::__rusty_product_kernel_execution_link!(
                        owner: $owner::$variant,
                        identity: $identity,
                        target: concat!("kernel.", $identity),
                        contract: $contract,
                        kind: $kind,
                        execution: $execution_kind => $execute
                    ),
                )?
            )+
        ];
        }

        $(
            $crate::__rusty_product_kernel_execution_impl!(
                $($execution_kind => $contract, $execute)?
            );
        )+

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

            fn execution_links() -> &'static [$crate::ProductKernelExecutionLink<Self::Owner>] {
                $declaration::EXECUTION_LINKS
            }

            fn contract_json() -> Result<String, $crate::ProductAssemblyError> {
                $crate::render_contract_json::<$declaration>()
            }
        }
    };
}

/// Internal expansion helper for one optional executable capability link.
#[doc(hidden)]
#[macro_export]
macro_rules! __rusty_product_kernel_execution_link {
    (
        owner: $owner:expr,
        identity: $identity:expr,
        target: $target:expr,
        contract: $contract:ty,
        kind: $declared_kind:expr,
        execution: system => $execute:path
    ) => {
        $crate::ProductKernelExecutionLink::new(
            $owner,
            $identity,
            $target,
            <$contract as $crate::ProductKernelCapabilityContract>::TYPE_ID,
            stringify!($contract),
            stringify!($execute),
            $crate::product_model::CapabilityKind::System,
        )
    };
    (
        owner: $owner:expr,
        identity: $identity:expr,
        target: $target:expr,
        contract: $contract:ty,
        kind: $declared_kind:expr,
        execution: operation => $execute:path
    ) => {
        $crate::ProductKernelExecutionLink::new(
            $owner,
            $identity,
            $target,
            <$contract as $crate::ProductKernelCapabilityContract>::TYPE_ID,
            stringify!($contract),
            stringify!($execute),
            $crate::product_model::CapabilityKind::Operation,
        )
    };
    (
        owner: $owner:expr,
        identity: $identity:expr,
        target: $target:expr,
        contract: $contract:ty,
        kind: $declared_kind:expr,
        execution: projection => $execute:path
    ) => {
        $crate::ProductKernelExecutionLink::new(
            $owner,
            $identity,
            $target,
            <$contract as $crate::ProductKernelCapabilityContract>::TYPE_ID,
            stringify!($contract),
            stringify!($execute),
            $crate::product_model::CapabilityKind::Projection,
        )
    };
    (
        owner: $owner:expr,
        identity: $identity:expr,
        target: $target:expr,
        contract: $contract:ty,
        kind: $declared_kind:expr
    ) => {};
}

/// Internal expansion helper for the typed executor implementation. The
/// explicit lane token keeps a declaration's lifecycle phase visible and
/// prevents an operation function from being silently treated as a system.
#[doc(hidden)]
#[macro_export]
macro_rules! __rusty_product_kernel_execution_impl {
    () => {};
    (system => $contract:ty, $execute:path) => {
        impl $crate::ProductKernelSystemExecutor for $contract {
            fn execute_system(
                context: $crate::ProductSystemContext<'_, Self::Snapshot, Self::Request>,
            ) -> Result<Self::Result, Self::Error> {
                $execute(context)
            }
        }
    };
    (operation => $contract:ty, $execute:path) => {
        impl $crate::ProductKernelOperationExecutor for $contract {
            fn execute_operation(
                context: $crate::ProductOperationContext<'_, Self::Snapshot, Self::Request>,
            ) -> Result<Self::Result, Self::Error> {
                $execute(context)
            }
        }
    };
    (projection => $contract:ty, $execute:path) => {
        impl $crate::ProductKernelProjectionExecutor for $contract {
            fn execute_projection(
                context: $crate::ProductProjectionContext<'_, Self::Snapshot>,
            ) -> Result<Self::Result, Self::Error> {
                $execute(context)
            }
        }
    };
}
