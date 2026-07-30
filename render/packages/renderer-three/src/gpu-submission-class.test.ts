import assert from 'node:assert/strict';
import test from 'node:test';
import { classifyGpuSubmissionRendererName } from './gpu-submission-class.js';

void test('known software WebGL renderers select software submission duty', () => {
  assert.equal(
    classifyGpuSubmissionRendererName(
      'ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)), SwiftShader driver)',
    ),
    'software',
  );
  assert.equal(
    classifyGpuSubmissionRendererName('Mesa/X.org (LLVMpipe 17.0.6, 256 bits)'),
    'software',
  );
  assert.equal(
    classifyGpuSubmissionRendererName('Microsoft Basic Render Driver'),
    'software',
  );
});

void test('hardware and unavailable WebGL identities remain distinct', () => {
  assert.equal(
    classifyGpuSubmissionRendererName('ANGLE (NVIDIA, NVIDIA GeForce RTX 4080)'),
    'accelerated',
  );
  assert.equal(classifyGpuSubmissionRendererName(''), 'unknown');
  assert.equal(classifyGpuSubmissionRendererName(undefined), 'unknown');
});
