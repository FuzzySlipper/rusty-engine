import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from 'three';
import { MeshInspection, wholeVoxelNormalGeometry } from './mesh-inspection.js';

void test('inspection preserves textures and original physical surface; clones are instance-owned', () => {
  const texture = new THREE.Texture();
  const original = new THREE.MeshPhysicalMaterial({ map: texture, roughness: 0.2, metalness: 0.8, clearcoat: 0.6 });
  const geometry = new THREE.BoxGeometry();
  const a = new THREE.Mesh(geometry, original), b = new THREE.Mesh(geometry, original);
  const inspection = new MeshInspection(a);
  let sourceDisposals = 0, cloneDisposals = 0;
  original.addEventListener('dispose', () => sourceDisposals++);
  texture.addEventListener('dispose', () => sourceDisposals++);
  geometry.addEventListener('dispose', () => sourceDisposals++);
  inspection.apply({ wireframe: true, matte: true, wholeVoxelNormals: false, boundsRequest: 1 });
  const changed = a.material as THREE.MeshPhysicalMaterial;
  changed.addEventListener('dispose', () => cloneDisposals++);
  assert.notEqual(changed, original); assert.equal(changed.map, texture);
  assert.equal(changed.roughness, 1); assert.equal(changed.metalness, 0); assert.equal(changed.wireframe, true);
  assert.equal(b.material, original); assert.equal(original.roughness, 0.2);
  inspection.apply({ wireframe: false, matte: false, wholeVoxelNormals: false, boundsRequest: 1 });
  assert.equal(a.material, original); assert.equal(cloneDisposals, 1);
  inspection.dispose(); assert.equal(sourceDisposals, 0);
});

void test('whole-voxel normals accept integer unit faces, reject ordinary meshes and restore borrowed geometry', () => {
  const geometry = new THREE.BoxGeometry().translate(0.5, 0.5, 0.5);
  const normals = wholeVoxelNormalGeometry(geometry);
  assert.ok(normals); assert.notEqual(normals, geometry);
  assert.equal(wholeVoxelNormalGeometry(new THREE.SphereGeometry()), null);
  const mesh = new THREE.Mesh(geometry);
  const inspection = new MeshInspection(mesh);
  inspection.apply({ wireframe: false, matte: false, wholeVoxelNormals: true, boundsRequest: 1 });
  assert.equal(inspection.voxelNormalMeshes, 1); assert.notEqual(mesh.geometry, geometry);
  inspection.apply({ wireframe: false, matte: false, wholeVoxelNormals: false, boundsRequest: 2 });
  assert.equal(mesh.geometry, geometry); assert.equal(inspection.voxelNormalMeshes, 0);
  inspection.dispose(); normals.dispose();
});

void test('capture owns active inspection geometry and materials across source toggles and release', () => {
  const original = new THREE.BoxGeometry().translate(0.5,0.5,0.5);
  const mesh = new THREE.Mesh(original, new THREE.MeshStandardMaterial());
  const inspection = new MeshInspection(mesh);
  inspection.apply({ wireframe: true, matte: true, wholeVoxelNormals: true, boundsRequest: 1 });
  const capture = mesh.clone();
  const owned = inspection.cloneForCapture(capture);
  assert.notEqual(capture.geometry, mesh.geometry); assert.notEqual(capture.material, mesh.material);
  let disposed = 0;
  capture.geometry.addEventListener('dispose', () => disposed++);
  capture.material.addEventListener('dispose', () => disposed++);
  inspection.apply({ wireframe: false, matte: false, wholeVoxelNormals: false, boundsRequest: 2 });
  inspection.dispose(); assert.equal(disposed, 0);
  owned.dispose(); assert.equal(disposed, 2);
});
