# Task 8560 — published GLB material extensions

The woodland shrine is core triangle geometry with an embedded base-color image,
KHR_materials_specular factors, a declared KHR_materials_volume extension, and
optional FB_ngon_encoding polygon metadata. Its SHA-256 is
be114f1eb91f4113a07193db11f00091008d2b389cc8b9cfbfb34cbd8a2519c3.

The animated importer now admits specular/volume and optional polygon hints.
Required unknown extensions and required polygon hints remain unsupported.
Closure packing shares the animated importer's required-extension capability
list. Authored source/material data is preserved, not stripped or flattened.

## Existing implementation consulted

- Three 0.184 GLTFLoader already registers GLTFMaterialsSpecularExtension and
  GLTFMaterialsVolumeExtension and creates MeshPhysicalMaterial. The Engine's
  loadAnimatedMeshGlbResource already routes source bytes to it. Reused unchanged.
- gltf/gltf-json 1.4.1 implements both material schemas behind Cargo features,
  but omits their names from ENABLED_EXTENSIONS. The shared conversion parser
  runs the same full Validate pass and filters only Unsupported diagnostics at
  the exact required-array entries for those two enabled schemas. No authored
  bytes or declarations change. Import, closure packing and geometry metadata
  use this one parser.
- Khronos glTF 2.0 section 3.12 distinguishes extensionsUsed from
  extensionsRequired: https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#specifying-extensions
  The optional empty FB_ngon_encoding metadata sits on already-triangulated
  primitives; the existing renderer uses that core triangle representation.

## Verification

Focused import coverage checks exact runtime bytes, material/extension survival
through closure packing, required specular/volume, unknown required rejection,
and continued schema rejection. Renderer coverage checks actual physical
material factors, base texture and clips. Browser evidence and matched release
identity are recorded in the Den closeout and receiving workbench packet.

Checks passed: 44 asset-import tests, 14 conversion library tests and the focused
renderer physical-material test. Engine capability reuse, existing-owner reuse
and runtime trust all passed with no findings. This task does not claim all glTF
extensions or full gallery parity.
