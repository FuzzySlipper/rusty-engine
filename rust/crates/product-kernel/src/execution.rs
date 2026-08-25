//! Static execution linkage for Product Kernel capabilities.
//!
//! Product Model linkage deliberately stops at immutable capability metadata.
//! This module is the next, source-linked step: a downstream declaration can
//! name an ordinary Rust function for each live Product Kernel capability.
//! The declaration expands direct implementations of the typed executor
//! traits; it never stores a function pointer, callback, erased value, or
//! service lookup key.

use std::fmt;

use product_model::{
    CapabilityKind, LinkedCapabilityTarget, LinkedProductComposition, SchedulePhase,
};

use crate::{
    ProductKernelCapabilityContract, ProductKernelCapabilityEntry, ProductKernelDeclaration,
};

/// A statically linked Product Kernel system.
///
/// The associated types are inherited from the concrete capability contract.
/// A generated implementation calls the ordinary downstream function directly
/// and keeps the lifecycle-gated [`crate::ProductSystemContext`] boundary.
pub trait ProductKernelSystemExecutor: ProductKernelCapabilityContract {
    fn execute_system(
        context: crate::ProductSystemContext<'_, Self::Snapshot, Self::Request>,
    ) -> Result<Self::Result, Self::Error>;
}

/// A statically linked Product Kernel operation.
pub trait ProductKernelOperationExecutor: ProductKernelCapabilityContract {
    fn execute_operation(
        context: crate::ProductOperationContext<'_, Self::Snapshot, Self::Request>,
    ) -> Result<Self::Result, Self::Error>;
}

/// A statically linked Product Kernel projection.
pub trait ProductKernelProjectionExecutor: ProductKernelCapabilityContract {
    fn execute_projection(
        context: crate::ProductProjectionContext<'_, Self::Snapshot>,
    ) -> Result<Self::Result, Self::Error>;
}

/// Product-owned phase adapter convention for a generated composition root.
///
/// A concrete adapter owns the product snapshot, request construction, and
/// any other product state needed to build the typed context for each owner.
/// The generated root supplies only the closed owner and the lifecycle token
/// for the phase. Implementations normally match the owner and call one of
/// the declaration-generated facade functions; no context, result, or error
/// is erased here. The adapter is an ordinary product value, not a scheduler,
/// registry, service locator, or retained callback.
pub trait ProductKernelRuntimeAdapter {
    type Owner: crate::ProductKernelOwner;
    type Output;
    type Error;

    fn dispatch_system(
        &mut self,
        owner: Self::Owner,
        lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<Self::Output, Self::Error>;

    fn dispatch_operation(
        &mut self,
        owner: Self::Owner,
        lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<Self::Output, Self::Error>;

    fn dispatch_projection(
        &mut self,
        owner: Self::Owner,
        lifecycle: &runtime_lifecycle::RuntimeLifecycle,
        token: runtime_lifecycle::RuntimePhaseToken,
    ) -> Result<Self::Output, Self::Error>;
}

/// Source-only metadata for one generated executable link.
///
/// `function_path` and `contract_path` are used only while emitting a closed
/// generated Rust source file. They are not interpreted at runtime and are
/// never used as dynamic method names. The link contains no callable value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductKernelExecutionLink<O> {
    owner: O,
    identity: &'static str,
    target: &'static str,
    contract_type: &'static str,
    contract_path: &'static str,
    function_path: &'static str,
    kind: CapabilityKind,
}

impl<O: Copy> ProductKernelExecutionLink<O> {
    /// Creates one source-only executable link. The constructor is public so
    /// generated declarations and compile-time fixture tests can construct
    /// the same immutable shape without a registry.
    pub const fn new(
        owner: O,
        identity: &'static str,
        target: &'static str,
        contract_type: &'static str,
        contract_path: &'static str,
        function_path: &'static str,
        kind: CapabilityKind,
    ) -> Self {
        Self {
            owner,
            identity,
            target,
            contract_type,
            contract_path,
            function_path,
            kind,
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

    /// Rust source path for the concrete contract type, retained only for
    /// generated source diagnostics and inspection.
    pub const fn contract_path(self) -> &'static str {
        self.contract_path
    }

    /// Rust source path for the ordinary concrete executor function, retained
    /// only for generated source emission.
    pub const fn function_path(self) -> &'static str {
        self.function_path
    }

    pub const fn kind(self) -> CapabilityKind {
        self.kind
    }
}

/// Failure while validating or rendering source-linked execution links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductKernelExecutionError {
    EmptyFunctionPath {
        target: String,
    },
    EmptyContractPath {
        target: String,
    },
    InvalidTarget {
        target: String,
    },
    DuplicateTarget {
        target: String,
    },
    UnknownDeclarationTarget {
        target: String,
    },
    OwnerMismatch {
        target: String,
    },
    TargetMismatch {
        target: String,
        expected: String,
        received: String,
    },
    KindMismatch {
        target: String,
        expected: CapabilityKind,
        received: CapabilityKind,
    },
    ContractTypeMismatch {
        target: String,
        expected: String,
        received: String,
    },
    UnsupportedSystemPhase {
        target: String,
        phase: SchedulePhase,
        path: String,
    },
    UnsupportedExecutionKind {
        target: String,
        kind: CapabilityKind,
    },
    MissingExecutableLink {
        target: String,
        kind: CapabilityKind,
        path: String,
    },
    InvalidSourceFragment {
        field: &'static str,
    },
}

impl fmt::Display for ProductKernelExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Product Kernel execution linkage rejected: {self:?}"
        )
    }
}

impl std::error::Error for ProductKernelExecutionError {}

/// Validates all source-linked executable metadata and every live Product
/// Kernel reference in an admitted composition.
///
/// Unused declarations may remain metadata-only. Every Product Kernel target
/// that appears in input intent descriptors, the system schedule, or timeline
/// steps must have one matching executable link. This is the generation-time
/// fail-closed boundary that prevents a generated schedule arm from becoming a
/// silent no-op.
pub fn validate_product_kernel_execution<D: ProductKernelDeclaration>(
    linked: &LinkedProductComposition,
) -> Result<(), ProductKernelExecutionError> {
    validate_declared_links::<D>()?;

    for (phase_index, phase) in linked.admitted().schedule().iter().enumerate() {
        let expected_kind = match phase.phase() {
            SchedulePhase::Projection => CapabilityKind::Projection,
            SchedulePhase::Input
            | SchedulePhase::Simulation
            | SchedulePhase::Consequences
            | SchedulePhase::Commit => CapabilityKind::System,
        };
        for system in phase.systems() {
            let binding = linked
                .capability_binding(system.capability().binding_index())
                .expect("Product Model admitted references have linked indices");
            if matches!(
                binding.resolved_target(),
                LinkedCapabilityTarget::ProductKernel(_)
            ) {
                if expected_kind == CapabilityKind::System
                    && phase.phase() != SchedulePhase::Simulation
                {
                    return Err(ProductKernelExecutionError::UnsupportedSystemPhase {
                        target: binding.target().to_owned(),
                        phase: phase.phase(),
                        path: format!("schedule[{phase_index}].systems[{}]", system.source_index()),
                    });
                }
                require_live_link::<D>(
                    binding.target(),
                    expected_kind,
                    format!("schedule[{phase_index}].systems[{}]", system.source_index()),
                )?;
            }
        }
    }

    for (timeline_index, timeline) in linked.admitted().timelines().iter().enumerate() {
        for (step_index, step) in timeline.steps().iter().enumerate() {
            let binding = linked
                .capability_binding(step.capability().binding_index())
                .expect("Product Model admitted references have linked indices");
            if matches!(
                binding.resolved_target(),
                LinkedCapabilityTarget::ProductKernel(_)
            ) {
                require_live_link::<D>(
                    binding.target(),
                    CapabilityKind::Operation,
                    format!("timelines[{timeline_index}].steps[{step_index}]"),
                )?;
            }
        }
    }

    for (intent_index, intent) in linked.admitted().intent_descriptors().iter().enumerate() {
        let Some(reference) = intent.capability() else {
            // VM-local intents have no Product Kernel execution link to
            // validate. Kernel manifests cannot reach this branch because
            // Product Model admission still requires their linkage.
            continue;
        };
        let binding = linked
            .capability_binding(reference.binding_index())
            .expect("Product Model admitted references have linked indices");
        if matches!(
            binding.resolved_target(),
            LinkedCapabilityTarget::ProductKernel(_)
        ) {
            // Input descriptors are normally operations. Preserve the actual
            // linked kind in the diagnostic for products that intentionally use
            // another currently admitted Product Kernel kind.
            let expected_kind = binding.metadata().kind();
            require_live_link::<D>(
                binding.target(),
                expected_kind,
                format!("intentDescriptors[{intent_index}].capability"),
            )?;
        }
    }
    Ok(())
}

/// Checks the declaration's executable links without requiring a composition.
pub fn validate_product_kernel_execution_declaration<D: ProductKernelDeclaration>(
) -> Result<(), ProductKernelExecutionError> {
    validate_declared_links::<D>()
}

fn validate_declared_links<D: ProductKernelDeclaration>() -> Result<(), ProductKernelExecutionError>
{
    let mut targets = std::collections::BTreeSet::new();
    for link in D::execution_links() {
        if link.function_path().is_empty() {
            return Err(ProductKernelExecutionError::EmptyFunctionPath {
                target: link.target().to_owned(),
            });
        }
        if link.contract_path().is_empty() {
            return Err(ProductKernelExecutionError::EmptyContractPath {
                target: link.target().to_owned(),
            });
        }
        if !is_rust_path(link.contract_path()) {
            return Err(ProductKernelExecutionError::InvalidSourceFragment {
                field: "contract_path",
            });
        }
        if !is_relative_rust_path(link.function_path()) {
            return Err(ProductKernelExecutionError::InvalidSourceFragment {
                field: "function_path",
            });
        }
        if matches!(
            link.kind(),
            CapabilityKind::Query | CapabilityKind::Migration
        ) {
            return Err(ProductKernelExecutionError::UnsupportedExecutionKind {
                target: link.target().to_owned(),
                kind: link.kind(),
            });
        }
        let expected_target = format!("kernel.{}", link.identity());
        if link.target() != expected_target {
            return Err(ProductKernelExecutionError::InvalidTarget {
                target: link.target().to_owned(),
            });
        }
        if !targets.insert(link.target()) {
            return Err(ProductKernelExecutionError::DuplicateTarget {
                target: link.target().to_owned(),
            });
        }
        let Some(entry) = D::entries()
            .iter()
            .find(|entry| entry.target() == link.target())
        else {
            return Err(ProductKernelExecutionError::UnknownDeclarationTarget {
                target: link.target().to_owned(),
            });
        };
        validate_link_against_entry(*link, entry)?;
    }
    Ok(())
}

fn validate_link_against_entry<O: Copy + Eq>(
    link: ProductKernelExecutionLink<O>,
    entry: &ProductKernelCapabilityEntry<O>,
) -> Result<(), ProductKernelExecutionError> {
    if link.owner() != entry.owner() {
        return Err(ProductKernelExecutionError::OwnerMismatch {
            target: link.target().to_owned(),
        });
    }
    if link.target() != entry.target() {
        return Err(ProductKernelExecutionError::TargetMismatch {
            target: link.target().to_owned(),
            expected: entry.target().to_owned(),
            received: link.target().to_owned(),
        });
    }
    if link.kind() != entry.contract_kind() || link.kind() != entry.metadata().kind() {
        return Err(ProductKernelExecutionError::KindMismatch {
            target: link.target().to_owned(),
            expected: entry.contract_kind(),
            received: link.kind(),
        });
    }
    if link.contract_type() != entry.contract_type() {
        return Err(ProductKernelExecutionError::ContractTypeMismatch {
            target: link.target().to_owned(),
            expected: entry.contract_type().to_owned(),
            received: link.contract_type().to_owned(),
        });
    }
    Ok(())
}

fn require_live_link<D: ProductKernelDeclaration>(
    target: &str,
    expected_kind: CapabilityKind,
    path: String,
) -> Result<(), ProductKernelExecutionError> {
    let Some(link) = D::execution_links()
        .iter()
        .find(|link| link.target() == target)
    else {
        return Err(ProductKernelExecutionError::MissingExecutableLink {
            target: target.to_owned(),
            kind: expected_kind,
            path,
        });
    };
    if link.kind() != expected_kind {
        return Err(ProductKernelExecutionError::KindMismatch {
            target: target.to_owned(),
            expected: expected_kind,
            received: link.kind(),
        });
    }
    Ok(())
}

/// Renders direct Rust call fragments for the live Product Kernel references
/// in a linked composition. This is only a source-inspection aid for a
/// caller-owned, already-typed adapter: the caller must construct the named
/// context, route the concrete result/error, and own the phase dispatch. The
/// returned text is not a heterogeneous runtime dispatcher and must not be
/// pasted into a generic schedule closure as if every capability shared one
/// context or result type. It contains direct concrete function paths and
/// never a method-name lookup or callback table.
pub fn render_product_kernel_execution_arms<D: ProductKernelDeclaration>(
    linked: &LinkedProductComposition,
) -> Result<String, ProductKernelExecutionError> {
    validate_product_kernel_execution::<D>(linked)?;

    let mut targets = std::collections::BTreeSet::new();
    let mut arms = String::new();
    for link in D::execution_links() {
        if !linked
            .capability_bindings()
            .iter()
            .any(|binding| binding.target() == link.target())
        {
            continue;
        }
        if !targets.insert(link.target()) {
            continue;
        }
        let context = match link.kind() {
            CapabilityKind::System => "system_context",
            CapabilityKind::Operation => "operation_context",
            CapabilityKind::Projection => "projection_context",
            CapabilityKind::Query | CapabilityKind::Migration => {
                unreachable!("unsupported Product Kernel execution kind was rejected above")
            }
        };
        arms.push_str(&format!(
            "        {:?} => product_kernel::{}({}),\n",
            link.target(),
            link.function_path(),
            context
        ));
    }
    Ok(arms)
}

fn is_rust_path(value: &str) -> bool {
    let mut components = value.split("::");
    let Some(first) = components.next() else {
        return false;
    };
    if first.is_empty() || (!is_rust_ident(first) && !matches!(first, "crate" | "self" | "super")) {
        return false;
    }
    components.all(is_rust_ident)
}

fn is_relative_rust_path(value: &str) -> bool {
    if !is_rust_path(value) {
        return false;
    }
    let first = value.split("::").next().unwrap_or_default();
    !matches!(first, "crate" | "self" | "super" | "Self")
}

fn is_rust_ident(value: &str) -> bool {
    if matches!(
        value,
        "as" | "async"
            | "await"
            | "become"
            | "box"
            | "break"
            | "const"
            | "continue"
            | "do"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "yield"
    ) {
        return false;
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

/// Maps the source-level execution lane used by the declaration/facade macros
/// to the public Product Model kind without evaluating any user-provided
/// expression. Keeping this mapping in one exported helper also lets a
/// downstream crate invoke `product_kernel_execution_facade!` while the
/// expansion remains hygienic across crate boundaries.
#[doc(hidden)]
#[macro_export]
macro_rules! __rusty_product_kernel_execution_kind {
    (system) => {
        $crate::CapabilityKind::System
    };
    (operation) => {
        $crate::CapabilityKind::Operation
    };
    (projection) => {
        $crate::CapabilityKind::Projection
    };
}

/// Generates a closed, declaration-specific heterogeneous execution facade.
///
/// The facade is useful at a generated composition root: each phase dispatch
/// accepts a typed context enum and returns a typed result/error enum. The
/// enum variants are all concrete associated types from the declaration; no
/// `Any`, JSON value, trait object, callback, or runtime method lookup is
/// involved. Context construction remains caller-owned so product state and
/// mutation authority stay outside this crate.
///
/// The capability list is intentionally repeated at this source boundary. It
/// makes the generated Rust inspectable and gives the compiler a direct proof
/// that every branch calls the declared executor trait for the matching
/// concrete contract. `validate()` ties the facade back to the declaration's
/// static linkage metadata.
#[macro_export]
macro_rules! product_kernel_execution_facade {
    (
        declaration: $declaration:ty,
        owner: $owner:ident,
        context: $context:ident,
        result: $result:ident,
        error: $error:ident,
        capabilities: [
            $(
                $variant:ident => $contract:ty {
                    execution: $execution_kind:ident,
                    context: $context_variant:ident,
                    result: $result_variant:ident,
                    error: $error_variant:ident
                }
            ),+ $(,)?
        ]
    ) => {
        #[allow(dead_code)]
        pub enum $context<'a> {
            $(
                $context_variant(
                    $crate::__rusty_product_kernel_facade_context_type!(
                        $execution_kind,
                        $contract,
                        'a
                    )
                ),
            )+
        }

        impl<'a> $context<'a> {
            pub const fn kind(&self) -> $crate::CapabilityKind {
                match self {
                    $(Self::$context_variant(_) =>
                        $crate::__rusty_product_kernel_execution_kind!($execution_kind),)+
                }
            }
        }

        #[allow(dead_code)]
        pub enum $result {
            $(
                $result_variant(
                    <$contract as $crate::ProductKernelCapabilityContract>::Result
                ),
            )+
        }

        #[allow(dead_code)]
        pub enum $error {
            $(
                $error_variant(
                    <$contract as $crate::ProductKernelCapabilityContract>::Error
                ),
            )+
            WrongOwnerKind {
                expected: $crate::CapabilityKind,
                received: $crate::CapabilityKind,
            },
            WrongContextKind {
                expected: $crate::CapabilityKind,
                received: $crate::CapabilityKind,
            },
        }

        pub fn validate() -> Result<(), $crate::ProductKernelExecutionError> {
            $crate::validate_product_kernel_execution_declaration::<$declaration>()
        }

        pub fn execute_system(
            owner: $owner,
            context: $context<'_>,
        ) -> Result<$result, $error> {
            match owner {
                $(
                    $owner::$variant =>
                        $crate::__rusty_product_kernel_facade_system_arm!(
                            $execution_kind,
                            $contract,
                            $context,
                            $context_variant,
                            $result,
                            $result_variant,
                            $error,
                            $error_variant,
                            context
                        ),
                )+
            }
        }

        pub fn execute_operation(
            owner: $owner,
            context: $context<'_>,
        ) -> Result<$result, $error> {
            match owner {
                $(
                    $owner::$variant =>
                        $crate::__rusty_product_kernel_facade_operation_arm!(
                            $execution_kind,
                            $contract,
                            $context,
                            $context_variant,
                            $result,
                            $result_variant,
                            $error,
                            $error_variant,
                            context
                        ),
                )+
            }
        }

        pub fn execute_projection(
            owner: $owner,
            context: $context<'_>,
        ) -> Result<$result, $error> {
            match owner {
                $(
                    $owner::$variant =>
                        $crate::__rusty_product_kernel_facade_projection_arm!(
                            $execution_kind,
                            $contract,
                            $context,
                            $context_variant,
                            $result,
                            $result_variant,
                            $error,
                            $error_variant,
                            context
                        ),
                )+
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rusty_product_kernel_facade_context_type {
    (system, $contract:ty, $lifetime:tt) => {
        $crate::ProductSystemContext<
            $lifetime,
            <$contract as $crate::ProductKernelCapabilityContract>::Snapshot,
            <$contract as $crate::ProductKernelCapabilityContract>::Request,
        >
    };
    (operation, $contract:ty, $lifetime:tt) => {
        $crate::ProductOperationContext<
            $lifetime,
            <$contract as $crate::ProductKernelCapabilityContract>::Snapshot,
            <$contract as $crate::ProductKernelCapabilityContract>::Request,
        >
    };
    (projection, $contract:ty, $lifetime:tt) => {
        $crate::ProductProjectionContext<
            $lifetime,
            <$contract as $crate::ProductKernelCapabilityContract>::Snapshot,
        >
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rusty_product_kernel_facade_system_arm {
    (system, $contract:ty, $context:ident, $context_variant:ident, $result:ident, $result_variant:ident, $error:ident, $error_variant:ident, $value:ident) => {{
        let received_kind = $value.kind();
        match $value {
            $context::$context_variant(context) => {
                <$contract as $crate::ProductKernelSystemExecutor>::execute_system(context)
                    .map($result::$result_variant)
                    .map_err($error::$error_variant)
            }
            _ => Err($error::WrongContextKind {
                expected: $crate::CapabilityKind::System,
                received: received_kind,
            }),
        }
    }};
    ($other:ident, $contract:ty, $context:ident, $context_variant:ident, $result:ident, $result_variant:ident, $error:ident, $error_variant:ident, $value:ident) => {
        Err($error::WrongOwnerKind {
            expected: $crate::CapabilityKind::System,
            received: <$contract as $crate::ProductKernelCapabilityContract>::KIND,
        })
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rusty_product_kernel_facade_operation_arm {
    (operation, $contract:ty, $context:ident, $context_variant:ident, $result:ident, $result_variant:ident, $error:ident, $error_variant:ident, $value:ident) => {{
        let received_kind = $value.kind();
        match $value {
            $context::$context_variant(context) => {
                <$contract as $crate::ProductKernelOperationExecutor>::execute_operation(context)
                    .map($result::$result_variant)
                    .map_err($error::$error_variant)
            }
            _ => Err($error::WrongContextKind {
                expected: $crate::CapabilityKind::Operation,
                received: received_kind,
            }),
        }
    }};
    ($other:ident, $contract:ty, $context:ident, $context_variant:ident, $result:ident, $result_variant:ident, $error:ident, $error_variant:ident, $value:ident) => {
        Err($error::WrongOwnerKind {
            expected: $crate::CapabilityKind::Operation,
            received: <$contract as $crate::ProductKernelCapabilityContract>::KIND,
        })
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rusty_product_kernel_facade_projection_arm {
    (projection, $contract:ty, $context:ident, $context_variant:ident, $result:ident, $result_variant:ident, $error:ident, $error_variant:ident, $value:ident) => {{
        let received_kind = $value.kind();
        match $value {
            $context::$context_variant(context) => {
                <$contract as $crate::ProductKernelProjectionExecutor>::execute_projection(context)
                    .map($result::$result_variant)
                    .map_err($error::$error_variant)
            }
            _ => Err($error::WrongContextKind {
                expected: $crate::CapabilityKind::Projection,
                received: received_kind,
            }),
        }
    }};
    ($other:ident, $contract:ty, $context:ident, $context_variant:ident, $result:ident, $result_variant:ident, $error:ident, $error_variant:ident, $value:ident) => {
        Err($error::WrongOwnerKind {
            expected: $crate::CapabilityKind::Projection,
            received: <$contract as $crate::ProductKernelCapabilityContract>::KIND,
        })
    };
}
