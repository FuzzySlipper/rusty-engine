#pragma once
#include <stddef.h>
#include <stdint.h>

typedef struct NativeByteSlice {
  const uint8_t *bytes;
  size_t len;
} NativeByteSlice;

typedef struct NativeLeaseFixtureItemLeaseHandle {
  uint64_t value;
} NativeLeaseFixtureItemLeaseHandle;

typedef struct NativeLeaseFixtureItem {
  uint32_t ordinal;
} NativeLeaseFixtureItem;

typedef struct NativeLeaseFixtureItemLease {
  NativeLeaseFixtureItemLeaseHandle handle;
  const NativeLeaseFixtureItem *entries;
  size_t entries_len;
  NativeByteSlice source;
} NativeLeaseFixtureItemLease;

typedef int32_t (*NativeReadLeaseFixtureItems)(void *, NativeLeaseFixtureItemLease *);
typedef int32_t (*NativeDestroyLeaseFixtureItemLease)(void *, NativeLeaseFixtureItemLeaseHandle);

typedef struct NativeLeaseFixtureApi {
  void *context;
  NativeReadLeaseFixtureItems read_items;
  NativeDestroyLeaseFixtureItemLease destroy_item_lease;
} NativeLeaseFixtureApi;

typedef struct NativeEngineApi {
  NativeLeaseFixtureApi lease_fixture;
} NativeEngineApi;
