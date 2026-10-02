# C# offline images and GLB export

Entry page: [C# SDK guide](csharp-sdk.md).

`engine.RenderOutput` owns asynchronous output from the current retained
appearance snapshot. `CaptureImage` and `ExportSceneGlb` select a product
`AppearanceFact.ObjectId`, including its descendants and ancestor transforms.
For an assembly, create a `PrimitiveGeometry.Group` appearance and publish a
visible root `AppearanceFact` with each part parented to that root. The group
draws no geometry; capture its object ID to include all descendants. A part's
own object ID still selects only that part's subtree.
They freeze the scene at successful callback completion; later product changes
cannot alter that job. A missing source/camera or unsupported export feature
produces a failed job with a UTF-8 diagnostic, rather than partial success.

```csharp
RenderOutput image = engine.RenderOutput.CaptureImage(new(
    sourceObjectId, camera, 512, 512, new Color(0, 0, 0, 0), false,
    1, CaptureToneMapping.AcesFilmic, 4, animatedObjectId, "run", .5));
RenderOutput glb = engine.RenderOutput.ExportSceneGlb(new(sourceObjectId, true));
// During later product callbacks:
if (engine.RenderOutput.Read(image).State == RenderOutputState.Completed)
{
    ReadOnlyMemory<byte> png = engine.RenderOutput.ReadBytes(image);
    // Product chooses a file, store, or other destination for these copied bytes.
    image.Dispose();
}
```

Image dimensions are independent of the window. The result is a top-to-bottom
8-bit sRGB RGBA PNG with straight alpha. Clear colors use linear RGB;
`UseCameraBackground` instead selects the current `CameraView` sky/color and
keeps the scene's fog. Retained lights, material assignments, camera framing
and projection are Engine inputs. Exposure, no tone mapping/ACES, and
multisample count are explicit capture choices; the scene's
`CameraView.SetToneMapping` does not apply to captures; a sample count above 1 supersamples. Dimensions
beyond the device's texture limit fail with a diagnostic. A zero
`PoseObjectId` keeps the frozen pose; a nonzero object
selects an exact normalized clip time in `[0,1]`, including the final pose,
without advancing the live animation or wall clock.

Completion means resources loaded, pose evaluated, render/readback finished,
and PNG/GLB bytes copied to the Engine owner. Poll `Read`; use `ReadDiagnostic`
for a failed job. `Cancel` or `Dispose` prevents later completion from reviving
a job. Dispose results after use.
Requests settle after a callback, so never block that callback waiting for one.

GLB exports current retained geometry, hierarchy, transforms, standard material
and texture assignments, normals and UVs, rather than returning the original
imported file. The selected object keeps its ancestor path, so its placement
survives. Primitives export as unlit materials, retained materials as
metallic-roughness with metalness 0, and textures embed their PNG bytes.
`IncludeAnimations` keeps each skinned mesh's skin and every resolved clip, clip
packs included; without it only the rig at rest is written. Hidden objects are
exported. Sprites, voxel objects and voxel-surface materials, and ambient
lights have no glTF counterpart and fail the job, naming the object. Generated meshes
remain exportable after the implicit field has been disposed, as long as the
mesh appearance is retained when the request settles. Reopen output through the
`Animation.OpenAnimatedMesh` / `CreateAnimatedMeshAppearance` content path
(which also admits static GLBs with no embedded clips).

The runtime runs output jobs itself, on a worker thread beside the product. Images
render through the Engine's wgpu renderer, on the machine's GPU or a software
Vulkan adapter such as Mesa's lavapipe. GLB export needs no GPU. No browser is
involved, so an unattended batch is an ordinary `rusty dev` or
`rusty-product-host` launch with no page attached.
