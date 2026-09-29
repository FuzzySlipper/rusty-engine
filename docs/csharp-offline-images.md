# C# offline images and GLB export

Entry page: [C# SDK guide](csharp-sdk.md).

## Offline images and GLB export

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
`UseCameraBackground` instead selects the current `CameraView` sky/color.
Existing retained lights, material assignments, camera framing and projection
remain Engine inputs. Exposure, no tone mapping/ACES, and multisample count are
explicit capture choices. Unsupported target dimensions or sample counts fail
with a diagnostic. A zero `PoseObjectId` keeps the frozen pose; a nonzero object
selects an exact normalized clip time in `[0,1]`, including the final pose,
without advancing the live animation or wall clock.

Completion means resources loaded, pose evaluated, render/readback finished,
and PNG/GLB bytes copied to the Engine owner. Poll `Read`; use `ReadDiagnostic`
for a failed job. `Cancel` or `Dispose` prevents later completion from reviving
a job. Dispose results after use; the renderer reuses its batch render target.
Requests settle after a callback, so never block that callback waiting for one.

GLB exports current retained geometry, hierarchy, transforms, standard material
and texture assignments, normals and UVs, rather than returning the original
imported file. `IncludeAnimations` preserves supported skin/clip data. Unsupported
shader-based materials or animation features fail explicitly. Generated meshes
remain exportable after the implicit field has been disposed, as long as the
mesh appearance is retained when the request settles. Reopen output through the
`Animation.OpenAnimatedMesh` / `CreateAnimatedMeshAppearance` content path
(which also admits static GLBs with no embedded clips).

For unattended batches, launch the packaged `rusty dev --headless` or
`rusty-product-host --headless`. Chromium must be installed; `RUSTY_CHROMIUM_PATH`
selects its executable. This uses the Engine browser backend in a managed
headless process, including software WebGL support; it is not a GPU-free
renderer or a product-owned DOM screenshot path.
The host creates and removes a disposable browser profile. Its basic password
store avoids desktop keyring prompts that can stall the first page request in
an unattended Linux session; it does not use your interactive browser profile.
