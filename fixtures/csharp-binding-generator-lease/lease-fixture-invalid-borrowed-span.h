#pragma once
#include <stddef.h>
#include <stdint.h>

typedef struct NativeUtf8Slice {
  const uint8_t *bytes;
  size_t len;
} NativeUtf8Slice;

typedef struct NativeInvalidBorrowedTag {
  NativeUtf8Slice value;
  const uint8_t *unsupported_nested_pointer;
} NativeInvalidBorrowedTag;

typedef struct NativeReplaceInvalidBorrowedTagsRequest {
  const NativeInvalidBorrowedTag *tags;
  size_t tags_len;
} NativeReplaceInvalidBorrowedTagsRequest;

typedef int32_t (*NativeReplaceInvalidBorrowedTags)(void *, const NativeReplaceInvalidBorrowedTagsRequest *);

typedef struct NativeLeaseFixtureApi {
  void *context;
  NativeReplaceInvalidBorrowedTags replace_tags;
} NativeLeaseFixtureApi;

typedef struct NativeEngineApi {
  NativeLeaseFixtureApi lease_fixture;
} NativeEngineApi;
