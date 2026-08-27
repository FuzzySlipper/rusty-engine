#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct NativeUtf8Slice {
  const uint8_t *bytes;
  size_t len;
} NativeUtf8Slice;

typedef struct NativeByteSlice {
  const uint8_t *bytes;
  size_t len;
} NativeByteSlice;

typedef struct NativeVec2 { float x; float y; } NativeVec2;
typedef struct NativeVec3 { float x; float y; float z; } NativeVec3;
typedef struct NativeQuat { float x; float y; float z; float w; } NativeQuat;

typedef struct NativeLeaseFixtureRequest {
  uint32_t include_item;
} NativeLeaseFixtureRequest;

typedef struct NativeLeaseFixtureItemLeaseHandle {
  uint64_t value;
} NativeLeaseFixtureItemLeaseHandle;

typedef struct NativeLeaseFixtureItem {
  NativeUtf8Slice label;
  NativeByteSlice payload;
  uint32_t ordinal;
} NativeLeaseFixtureItem;

typedef enum NativeLeaseFixtureCompleteness {
  NativeLeaseFixtureCompleteness_Complete = 0,
  NativeLeaseFixtureCompleteness_Truncated = 1,
} NativeLeaseFixtureCompleteness;

typedef struct NativeLeaseFixtureItemLease {
  NativeLeaseFixtureItemLeaseHandle handle;
  const NativeLeaseFixtureItem *entries;
  size_t entries_len;
  uint32_t total;
  bool truncated;
  NativeLeaseFixtureCompleteness completeness;
  uint64_t revision;
  uint64_t content_hash;
  NativeVec2 anchor;
} NativeLeaseFixtureItemLease;

typedef struct NativeEngineDiagnostic {
  NativeUtf8Slice code;
  NativeUtf8Slice message;
  NativeUtf8Slice source;
} NativeEngineDiagnostic;

typedef struct NativeEngineDiagnosticLeaseHandle {
  uint64_t value;
} NativeEngineDiagnosticLeaseHandle;

typedef struct NativeEngineDiagnosticLease {
  NativeEngineDiagnosticLeaseHandle handle;
  const NativeEngineDiagnostic *diagnostics;
  size_t diagnostics_len;
} NativeEngineDiagnosticLease;

typedef struct NativeOperationErrorReceipt {
  NativeUtf8Slice service;
  NativeUtf8Slice operation;
  int32_t status;
  NativeEngineDiagnosticLease diagnostics;
} NativeOperationErrorReceipt;

typedef int32_t (*NativeReadLeaseFixtureItems)(void *, NativeLeaseFixtureRequest, NativeLeaseFixtureItemLease *, NativeOperationErrorReceipt *);
typedef int32_t (*NativeDestroyLeaseFixtureItemLease)(void *, NativeLeaseFixtureItemLeaseHandle);
typedef int32_t (*NativeDestroyLeaseFixtureOperationDiagnosticLease)(void *, NativeEngineDiagnosticLeaseHandle);

typedef struct NativeLeaseFixtureApi {
  void *context;
  NativeReadLeaseFixtureItems read_items;
  NativeDestroyLeaseFixtureItemLease destroy_item_lease;
  NativeDestroyLeaseFixtureOperationDiagnosticLease destroy_operation_diagnostic_lease;
} NativeLeaseFixtureApi;

typedef struct NativeEngineApi {
  NativeLeaseFixtureApi lease_fixture;
} NativeEngineApi;
