#pragma once
#include <stddef.h>
#include <stdint.h>

typedef struct NativeByteSlice {
  const uint8_t *bytes;
  size_t len;
} NativeByteSlice;

typedef struct NativeResultFixtureItem {
  uint32_t ordinal;
} NativeResultFixtureItem;

typedef struct NativeResultFixtureItemResult {
  const NativeResultFixtureItem *entries;
  size_t entries_len;
  NativeByteSlice source;
} NativeResultFixtureItemResult;

typedef int32_t (*NativeReadResultFixtureItems)(void *, NativeResultFixtureItemResult *);

typedef struct NativeResultFixtureApi {
  void *context;
  NativeReadResultFixtureItems read_items;
} NativeResultFixtureApi;

typedef struct NativeEngineApi {
  NativeResultFixtureApi result_fixture;
} NativeEngineApi;
