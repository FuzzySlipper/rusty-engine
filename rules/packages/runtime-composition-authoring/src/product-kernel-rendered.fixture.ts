// Generated from the Rust Product Kernel declaration. Do not edit by hand.
import { bindProductKernelCatalog } from '@rusty-engine/runtime-composition-authoring';
import type {
  ProductKernelCatalogWire,
  ProductKernelTarget as ProductKernelTargetFor,
} from '@rusty-engine/runtime-composition-authoring';
export const PRODUCT_KERNEL_CATALOG = {
  "artifact": "product-kernel",
  "capabilities": [
    {
      "access": {
        "reads": [
          "stealth.observations"
        ],
        "writes": [
          "stealth.alerts"
        ]
      },
      "availability": "linkable",
      "budget": {
        "maximumCompactJsonPayloadBytes": 4096
      },
      "contractType": "stealth.operation.v1",
      "identity": "stealth.advance-alert",
      "kind": "operation",
      "provenance": {
        "logicalPath": "advanceAlert",
        "owner": "stealth.product.alerts",
        "source": "src/alerts.ts"
      },
      "target": "kernel.stealth.advance-alert",
      "uses": [
        "schedule"
      ]
    },
    {
      "access": {
        "reads": [
          "stealth.snapshot"
        ],
        "writes": [
          "stealth.observations"
        ]
      },
      "availability": "linkable",
      "budget": {
        "maximumCompactJsonPayloadBytes": 4096
      },
      "contractType": "stealth.system.v1",
      "identity": "stealth.detect",
      "kind": "system",
      "provenance": {
        "logicalPath": "detect",
        "owner": "stealth.product.detection",
        "source": "src/detection.ts"
      },
      "target": "kernel.stealth.detect",
      "uses": [
        "schedule"
      ]
    }
  ],
  "migrations": [
    {
      "contractType": "stealth.migration.v1-to-v2",
      "from": "stealth.schema.v1",
      "identity": "stealth.migration.v1-to-v2",
      "to": "stealth.schema.v2"
    }
  ],
  "schemas": [
    {
      "contractType": "stealth.schema.v1",
      "identity": "stealth.schema.v1"
    },
    {
      "contractType": "stealth.schema.v2",
      "identity": "stealth.schema.v2"
    }
  ]
} as const satisfies ProductKernelCatalogWire;
export const productKernel = bindProductKernelCatalog(PRODUCT_KERNEL_CATALOG);
export type ProductKernelTarget = ProductKernelTargetFor<typeof PRODUCT_KERNEL_CATALOG>;
export const productKernelCapability = (id: string, target: ProductKernelTarget) =>
  productKernel.capability(id, target);
