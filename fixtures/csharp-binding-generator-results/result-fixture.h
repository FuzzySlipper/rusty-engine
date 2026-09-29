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
typedef struct NativeAnimationFeedbackText {
  uint8_t bytes[96];
  size_t len;
} NativeAnimationFeedbackText;

typedef struct NativeResultFixtureRequest {
  uint32_t include_item;
} NativeResultFixtureRequest;

typedef struct NativeResultFixtureTag {
  NativeUtf8Slice value;
  NativeByteSlice payload;
} NativeResultFixtureTag;

typedef struct NativeReplaceResultFixtureTagsRequest {
  const NativeResultFixtureTag *tags;
  size_t tags_len;
} NativeReplaceResultFixtureTagsRequest;

typedef struct NativeResultFixtureItem {
  NativeUtf8Slice label;
  NativeByteSlice payload;
  uint32_t ordinal;
} NativeResultFixtureItem;

typedef struct NativeResultFixtureObservation {
  uint64_t revision;
  uint32_t kind;
} NativeResultFixtureObservation;

typedef enum NativeResultFixtureCompleteness {
  NativeResultFixtureCompleteness_Complete = 0,
  NativeResultFixtureCompleteness_Truncated = 1,
} NativeResultFixtureCompleteness;

typedef struct NativeResultFixtureItemResult {
  const NativeResultFixtureItem *entries;
  size_t entries_len;
  const NativeResultFixtureObservation *observations;
  size_t observations_len;
  uint32_t total;
  bool truncated;
  NativeResultFixtureCompleteness completeness;
  uint64_t revision;
  uint64_t content_hash;
  NativeVec2 anchor;
} NativeResultFixtureItemResult;

typedef struct NativeOwnedFixtureHandle {
  uint64_t value;
} NativeOwnedFixtureHandle;

typedef struct NativeOwnedFixtureInfo {
  NativeOwnedFixtureHandle handle;
  uint32_t revision;
  NativeAnimationFeedbackText label;
} NativeOwnedFixtureInfo;

typedef struct NativeOptionalOwnedFixtureReceipt {
  NativeOwnedFixtureHandle handle;
  uint32_t admitted_count;
} NativeOptionalOwnedFixtureReceipt;

typedef struct NativeEngineDiagnostic {
  NativeUtf8Slice code;
  NativeUtf8Slice message;
  NativeUtf8Slice source;
} NativeEngineDiagnostic;

typedef struct NativeOperationErrorReceipt {
  const NativeEngineDiagnostic *diagnostics;
  size_t diagnostics_len;
} NativeOperationErrorReceipt;

typedef int32_t (*NativeReadResultFixtureItems)(void *, NativeResultFixtureRequest, NativeResultFixtureItemResult *, NativeOperationErrorReceipt *);
typedef int32_t (*NativeReadOwnedFixture)(void *, NativeOwnedFixtureInfo *);
typedef int32_t (*NativeReadInvalidOwnedFixture)(void *, NativeOwnedFixtureInfo *);
typedef int32_t (*NativeReadOptionalOwnedFixture)(void *, uint32_t, NativeOptionalOwnedFixtureReceipt *);
typedef int32_t (*NativeReplaceResultFixtureTags)(void *, const NativeReplaceResultFixtureTagsRequest *);
typedef int32_t (*NativeDestroyOwnedFixture)(void *, NativeOwnedFixtureHandle);

typedef struct NativeResultFixtureApi {
  void *context;
  NativeReadResultFixtureItems read_items;
  NativeReadOwnedFixture read_owned_fixture;
  NativeReadInvalidOwnedFixture read_invalid_owned_fixture;
  NativeReadOptionalOwnedFixture read_optional_owned_fixture;
  NativeReplaceResultFixtureTags replace_tags;
  NativeDestroyOwnedFixture destroy_owned_fixture;
} NativeResultFixtureApi;

typedef struct NativeEngineApi {
  NativeResultFixtureApi result_fixture;
} NativeEngineApi;
