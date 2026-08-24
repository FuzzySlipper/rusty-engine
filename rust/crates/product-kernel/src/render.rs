use product_model::CapabilityUse;
use serde_json::{json, Value};

use crate::{
    ProductKernelCapabilityEntry, ProductKernelDeclaration, ProductKernelMigrationDescriptor,
    ProductKernelOwner, ProductKernelSchemaDescriptor,
};

/// Validates a complete source-linked declaration before rendering its
/// deterministic current Rust contract JSON shape.
pub fn render_contract_json<D: ProductKernelDeclaration>(
) -> Result<String, crate::ProductAssemblyError> {
    crate::validate_declaration::<D>()?;
    Ok(render_contract_json_unchecked(
        D::entries(),
        D::schemas(),
        D::migrations(),
    ))
}

/// The deterministic current Rust contract JSON shape after declaration
/// validation. This is crate-visible for reorder tests; downstream callers
/// should use [`render_contract_json`] or the declaration method so stale
/// concrete metadata cannot be emitted.
pub(crate) fn render_contract_json_unchecked<O: ProductKernelOwner>(
    entries: &[ProductKernelCapabilityEntry<O>],
    schemas: &[ProductKernelSchemaDescriptor],
    migrations: &[ProductKernelMigrationDescriptor],
) -> String {
    let mut capabilities = entries
        .iter()
        .map(|entry| {
            let metadata = entry.metadata();
            let mut uses = [
                (CapabilityUse::InputMap, "input-map"),
                (CapabilityUse::Schedule, "schedule"),
                (CapabilityUse::Timeline, "timeline"),
            ]
            .into_iter()
            .filter_map(|(usage, name)| metadata.uses().contains(usage).then_some(name))
            .collect::<Vec<_>>();
            uses.sort_unstable();
            json!({
                "identity": entry.identity(),
                "target": entry.target(),
                "kind": metadata.kind().as_str(),
                "uses": uses,
                "availability": metadata.availability().as_str(),
                "access": {
                    "reads": metadata.access().reads(),
                    "writes": metadata.access().writes()
                },
                "budget": {
                    "maximumCompactJsonPayloadBytes": metadata.budget().maximum_compact_json_payload_bytes()
                },
                "provenance": {
                    "owner": metadata.provenance().owner(),
                    "source": metadata.provenance().source(),
                    "logicalPath": metadata.provenance().logical_path()
                },
                "contractType": entry.contract_type()
            })
        })
        .collect::<Vec<Value>>();
    capabilities.sort_by(|left, right| left["target"].as_str().cmp(&right["target"].as_str()));
    let mut schema_values = schemas
        .iter()
        .map(|schema| {
            json!({
                "identity": schema.identity(),
                "contractType": schema.contract_type()
            })
        })
        .collect::<Vec<_>>();
    schema_values.sort_by(|left, right| left["identity"].as_str().cmp(&right["identity"].as_str()));
    let mut migration_values = migrations
        .iter()
        .map(|migration| {
            json!({
                "identity": migration.identity(),
                "from": migration.from_schema(),
                "to": migration.to_schema(),
                "contractType": migration.contract_type()
            })
        })
        .collect::<Vec<_>>();
    migration_values
        .sort_by(|left, right| left["identity"].as_str().cmp(&right["identity"].as_str()));
    serde_json::to_string_pretty(&json!({
        "artifact": "product-kernel",
        "capabilities": capabilities,
        "schemas": schema_values,
        "migrations": migration_values
    }))
    .expect("source-linked Product Kernel contract is valid JSON")
        + "\n"
}

/// Validates a complete source-linked declaration before rendering its
/// product-local TypeScript module. The generated module is
/// an authoring composition root: it binds the immutable catalog to the
/// existing `bindProductKernelCatalog` helper and exports the closed target
/// type plus one named capability helper. It owns no runtime state.
pub fn render_contract_typescript<D: ProductKernelDeclaration>(
) -> Result<String, crate::ProductAssemblyError> {
    crate::validate_declaration::<D>()?;
    Ok(render_contract_typescript_unchecked(
        D::entries(),
        D::schemas(),
        D::migrations(),
    ))
}

pub(crate) fn render_contract_typescript_unchecked<O: ProductKernelOwner>(
    entries: &[ProductKernelCapabilityEntry<O>],
    schemas: &[ProductKernelSchemaDescriptor],
    migrations: &[ProductKernelMigrationDescriptor],
) -> String {
    let json = render_contract_json_unchecked(entries, schemas, migrations);
    format!(
        "// Generated from the Rust Product Kernel declaration. Do not edit by hand.\n\
import {{ bindProductKernelCatalog }} from '@rusty-engine/runtime-composition-authoring';\n\
import type {{\n  ProductKernelCatalogWire,\n  ProductKernelTarget as ProductKernelTargetFor,\n}} from '@rusty-engine/runtime-composition-authoring';\n\
export const PRODUCT_KERNEL_CATALOG = {} as const satisfies ProductKernelCatalogWire;\n\
export const productKernel = bindProductKernelCatalog(PRODUCT_KERNEL_CATALOG);\n\
export type ProductKernelTarget = ProductKernelTargetFor<typeof PRODUCT_KERNEL_CATALOG>;\n\
export const productKernelCapability = (id: string, target: ProductKernelTarget) =>\n  productKernel.capability(id, target);\n",
        json.trim_end(),
    )
}
