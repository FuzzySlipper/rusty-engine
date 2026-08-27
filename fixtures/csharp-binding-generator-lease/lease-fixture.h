#pragma once
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

typedef struct NativeLeaseFixtureItemLease {
  NativeLeaseFixtureItemLeaseHandle handle;
  const NativeLeaseFixtureItem *entries;
  size_t entries_len;
} NativeLeaseFixtureItemLease;

typedef int32_t (*NativeReadLeaseFixtureItems)(void *, NativeLeaseFixtureRequest, NativeLeaseFixtureItemLease *);
typedef int32_t (*NativeDestroyLeaseFixtureItemLease)(void *, NativeLeaseFixtureItemLeaseHandle);

typedef struct NativeLeaseFixtureApi {
  void *context;
  NativeReadLeaseFixtureItems read_items;
  NativeDestroyLeaseFixtureItemLease destroy_item_lease;
} NativeLeaseFixtureApi;

typedef struct NativeEngineApi {
  NativeLeaseFixtureApi lease_fixture;
} NativeEngineApi;
