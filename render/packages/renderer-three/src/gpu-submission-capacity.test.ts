import assert from 'node:assert/strict';
import test from 'node:test';

import {
  ACCELERATED_AUTOMATIC_SUBMISSION_CAPACITY,
  automaticSubmissionCapacity,
} from './gpu-submission-capacity.js';

void test('accelerated timer queries own a bounded ring without requiring sync fences', () => {
  assert.equal(
    automaticSubmissionCapacity('accelerated', true),
    ACCELERATED_AUTOMATIC_SUBMISSION_CAPACITY,
  );
});

void test('non-accelerated and timing-fallback paths stay at one submission', () => {
  assert.equal(automaticSubmissionCapacity('accelerated', false), 1);
  assert.equal(automaticSubmissionCapacity('software', true), 1);
  assert.equal(automaticSubmissionCapacity('unknown', true), 1);
});
