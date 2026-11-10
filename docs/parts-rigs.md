# Parts rigs

`IEngineContext.PartsRigs` plays 2D parts rigs built by
[rig-c](https://github.com/rig-c/core) and draws them as their posed pieces
on one upright plane in the world, the way a sprite stands. The Engine reads
rig-c's own model document (`skeleton.model.json`, `rigc-compiled/3`) and
the pages it names, and poses each rig once per call. It reads no Spine JSON
or atlas and links no Spine runtime.

The product loads rigs, makes appearances that show them, publishes those as
objects, and chooses clips, skins and driving values per object. The Engine
evaluates the poses, keeps the pages as textures and draws the pieces in
every reading of the scene.

```csharp
PartsRig front = engine.PartsRigs.Load(new("people/townsperson/front/skeleton.model.json", TextureFilter.Linear)).Rig;
PartsRig side = engine.PartsRigs.Load(new("people/townsperson/side/skeleton.model.json", TextureFilter.Linear)).Rig;
PartsRig back = engine.PartsRigs.Load(new("people/townsperson/back/skeleton.model.json", TextureFilter.Linear)).Rig;
Appearance person = engine.PartsRigs.CreateAppearance(new PartsRigAppearanceRequest(new PartsRigView[]
{
    new(front, 0, false),
    new(side, -MathF.PI / 2, false),
    new(back, MathF.PI, false),
    new(side, MathF.PI / 2, true),
}, metresPerUnit: 1.75f / 896));
engine.Graphics.PublishSnapshot([new(PersonId, false, 0, transform, person, true, RenderLayer.Scene)]);
engine.PartsRigs.Play(new PartsRigPlayRequest(PersonId, "walk", crossfadeSeconds: .2f));
// The idle's breathing added over the walk on track 1.
engine.PartsRigs.Play(PartsRigPlayRequest.Layer(PersonId, 1, "idle", alpha: 1, additive: true));
engine.PartsRigs.DriveBone(PartsRigDriveRequest.Hold(PersonId, "turn", PartsRigBoneChannel.X, 18));
```

See `fixtures/csharp-parts-rigs` for a townsperson of four views walking in
place with its breathing added over the walk and an orbiting camera, a
gallery rig whose clip fires events with a second clip added over it, and a
dialogue bust placed in a DOM frame's window.

## Loading

`Load` reads a model document from product content (loose content or a
bundle) and each page it names from the file beside it. `Filter` is how the
pages are sampled: `Nearest` for pixel art. It returns the rig with its bone,
slot, clip, skin and page counts. A document of another `rigc-compiled`
version is refused by name. IK, transform, path, physics and slider
constraints and clipping attachments are evaluated as rig-c's core does. A
rig whose slider animation keys another constraint's timelines, or deforms a
path a path constraint walks, is refused by name: rig-c's core does not pose
those either.

Disposing a `PartsRig` frees its pages; it is refused while an appearance
shows the rig.

## Appearances and views

`CreateAppearance` makes an appearance from one or more `PartsRigView`s, each
a rig with the heading it is seen from and a mirror flag. The other fields
are a sprite's ([upright sprites](csharp-capabilities.md#upright-sprites)):
`Scale` in metres per rig unit (CSS pixels for a `Pixel`-sized rig),
billboard mode, render order, depth policy, linear tint, and a sprite
material for lighting, alpha, shadow policy, emission and
`AuthoredFacing`. `TexelSnap`, in rig units per texel, draws a pixel-art
figure on its texel grid. The convenience constructor makes a world-sized
`Cylindrical` rig, unlit and masked at half alpha.

Lighting reads the plane's side the camera sees, whichever way a piece's
triangles wind. `Synthetic` lighting domes each piece over its own page
region, as a sprite is domed over its frame, so at a `NormalStrength` near 1
neighbouring pieces shade apart at their seams. `FigureDome` spans one dome
across the whole figure instead: the bounds of the drawn view's setup pose
(default skin), or a placed rig's frame. Pieces then shade as one surface
however they are cut, and breathing and sway move through the dome rather
than carrying it. Other lighting modes ignore it: an authored normal map
keeps each piece's page orientation.

```csharp
SpriteMaterialDescriptor lit = new(SpriteLightingMode.Synthetic, default, default, 1, 0,
    SpriteAlphaMode.Mask, .5f, SpriteShadowPolicy.None);
Appearance keeper = engine.PartsRigs.CreateAppearance(new PartsRigAppearanceRequest(
    new PartsRigView[] { new(rig, 0, false) }, metresPerUnit) with { Material = lit, FigureDome = true });
```

A heading is the bearing, about the object's up axis, of a camera that sees
that view: radians from the object's +z toward its +x, so 0 is a camera in
front. Each reading of the scene (the window, the stream, a capture, a
snapshot, an offline render) draws the view whose heading is nearest the
bearing its own camera sees the object from: toward the eye, or against the
view's heading for an orthographic or `CylindricalView` camera. A mirrored
view is drawn flipped across the rig's vertical axis, so one side's rig shows
the other side.

One view shows from every side.

## On a screen-pixel grid

`PixelGrid` draws a rig in the world as pixel art on the screen's own grid
rather than its pages' texel grids. The figure is drawn smooth (load its
pages with `TextureFilter.Linear`), lit, depth-tested and finished as it is
without one. Then it is cut into square blocks of that many output pixels:

- **One grid, anchored at the origin.** The grid has a corner at the
  projection of the rig's origin, so a turned or scaling bone never turns
  or resizes its pixels. Neighbouring parts share one grid, and a camera or
  object move never makes blocks crawl across the figure.
- **Coverage-weighted blocks.** Each block is the coverage-weighted average
  of the smooth figure under it. It draws where the figure covers at least
  `PixelCoverage` of it (one half for a hard pixel-art edge).
- **Depth per pixel.** Each block lies on the rig's plane, so a post or step
  in front hides it pixel by pixel.
- **Palette.** With a `Palette` of linear colours, each block takes the
  nearest by their sRGB values. The match is made after the lighting and
  before the view's finish, so lamp light and night still pick their
  colours. The finish (exposure, grading, tone mapping, fog) then applies to
  the blocks as to the rest of the scene. Under a neutral finish every pixel
  drawn is a palette colour.

```csharp
// Mara at her door: 2-pixel blocks of her lit, smooth figure in 32 colours,
// posed at 8 poses a second.
Appearance keeper = engine.PartsRigs.CreateAppearance(new PartsRigAppearanceRequest(
    new PartsRigView[] { new(door, 0, false) }, metresPerUnit) with
{
    Material = lit, PixelGrid = 2, Palette = keeperColours, PoseRate = 8,
});
```

A grid is refused beside a `TexelSnap`, the two being different pixel
grids. A rig placed in the viewport draws as it does without one: its texel
snap to whole screen pixels per texel is its pixel art.

Each reading draws its own grid. A view whose picture is drawn at a render
scale below 1 makes each block the nearest whole number of render pixels;
the upscale then softens the blocks. Under a temporal stage the blocks mark
its reactive mask, so they keep no history of what moves behind them.

The view draws its grid rigs into regions of a scratch image the size of the
view, one region per rig, packed in draw order. A rig that finds no room
left, or one standing across the eye's plane, draws smooth.
`gpu.partsRigGrid` counts the rigs drawn as blocks and those drawn smooth in
the last frame. The smooth drawing is the `parts-rig-grid` pass in
`gpu.passes`, and the blocks are part of the `world` pass's time. The
scratch image exists only while a frame draws grid rigs.

## Shadows

A rig whose material's `Shadow` is `Cast` or `CastAndReceive` casts its
posed pieces into the lights' shadow maps, as a [sprite](csharp-capabilities.md#sprite-shadows)
does: where their alpha reaches the cutoff, on its plane turned to the light.
It casts the view whose heading is nearest the light's bearing, mirrored as
that view is, in the pose its views share, so a walker's shadow walks and
shows its profile to a sun at its side whichever view the camera sees. A
rig placed in the viewport or sized in pixels casts nothing; a rig on a
screen-pixel grid casts its smooth figure.

## Placing in the viewport

`SetViewport` places a rig appearance in a rectangle of each reading's
viewport, as [sprite viewport placement](csharp-capabilities.md#sprite-viewport-placement)
places a sprite: a lower-left `Minimum`, a `Size` and an `Alignment` in
CSS-viewport units (0 to 1), with `Stretch` or `Contain` fitting. A dialogue
portrait drawn live inside a DOM frame is the use: the UI leaves a window in
its frame and the rig is fitted to it, either by following the window
itself ([anchored](#following-a-ui-element)) or at a rectangle the product
computes.

```csharp
// A 127-texel bust in a 381 CSS-pixel square, 32 pixels in from the lower left,
// its frame 47 texels up from the rig's origin (TexelSnap = texel).
CameraSurfaceReadout surface = engine.CameraView.ReadSurface();
engine.PartsRigs.SetViewport(new PartsRigViewportRequest(bust,
    new Vector2(32 / (float)surface.CssWidth, 32 / (float)surface.CssHeight),
    new Vector2(381 / (float)surface.CssWidth, 381 / (float)surface.CssHeight),
    new Vector2(.5f, .5f), SpriteViewportFit.Contain,
    frameMinimum: new Vector2(0, 47 * texel), frameSize: new Vector2(127 * texel)));
var anywhere = new Transform(Vector3.Zero, Quaternion.Identity, Vector3.One);
engine.Graphics.PublishChanges(new([new(BustId, false, 0, anywhere, bust, true, RenderLayer.Viewmodel)],
    ReadOnlyMemory<ulong>.Empty, ReadOnlyMemory<MeshJointAttachment>.Empty));
```

- **A steady frame.** What is fitted is a fixed rectangle of the rig's
  plane in rig units, not each pose's bounds, so breathing and sway move
  inside the frame rather than rescaling it, and it draws only inside the
  frame, as through a window. The short constructor fits the
  bounds of the first view's setup pose (default skin); the long one fits
  the frame the product gives, such as a bust's square.
- **The placement owns the place.** The object's transform, the
  appearance's scale, size mode and billboard mode are not read. The rig
  faces the camera and draws its first view. An object placed this way
  takes no physics swing from its object's motion, and `ReadBone` refuses
  it, as it does a `Pixel`-sized rig.
- **UI art, after the finish.** A placed rig draws after its layer's
  finish, into the finished image without depth, over everything in its
  layer in render order, one sample a pixel. Its pages' colours reach the
  screen as authored: not fogged, exposed (auto exposure included),
  bloomed, graded or tone mapped, whatever the camera's finish does to the
  scene. Its material's lighting still applies; an `Unlit` bust shows its
  texels exactly. Publish it in `RenderLayer.Viewmodel` to draw it over the
  scene in every reading (the window, the stream, captures). Under a
  temporal stage it marks the reactive mask, as labels do, so it never
  smears with what moves behind it.
- **Pixel art.** With a `TexelSnap`, the fit is the largest whole number of
  the target's pixels per texel that fits (the exact fit when not even one
  does), and the texel grid lies on pixel edges. A 127-texel frame in a
  381-pixel square draws each texel as exactly 3 × 3 CSS pixels; at a pixel
  ratio of 2, 6 × 6 device pixels. The grid starts at the rig's origin, so a
  frame whose corners are whole texels from the origin shows whole texels at
  its edges. Load the pages with `TextureFilter.Nearest`.
  A texel-snapped rig's alpha cutoff is hard, so its edge pixels are whole.
- **Playing.** Clips, layered tracks, skins and `DriveBone` work as they do
  in the world.

`PartsRigViewportRequest.InWorld(appearance)` returns it to the world. Each
call is shown at the next publish. The rectangle is in CSS-viewport units, so
a product that wants a size in CSS pixels divides by the surface's CSS size
(`CameraView.ReadSurface`) and places the rig again when the surface's
`Revision` changes, unless it follows a UI element.

### Following a UI element

`SetViewportAnchor(new(appearance, "bust"))` makes a placed rig's rectangle
follow the UI element the product UI anchors under the same name with
`context.viewport.anchor("bust", element)`, the anchor a camera's views
follow ([views that follow the product UI](csharp-lifecycle.md#views-that-follow-the-product-ui)).
The shell reports the element's rect on every layout change and the renderer
fits the rig's frame to it at once, in stream and window output alike, with
no product call: a resize runs no product code. The alignment, fit, frame
and texel snap still apply, so a 127-texel bust in a 381-pixel element is 3
pixels a texel and in a 254-pixel one 2. The `SetViewport` rectangle stands
while no page reports the element (a snapshot or offline render, or before
the page lays out), and again after `SetViewportAnchor(new(appearance, ""))`;
a later `SetViewport` keeps the anchor. A rig not placed with `SetViewport`
is refused. Anchor the window inside the frame, not the bordered frame, so
the rect is the window's: a child filling the frame's content box does.

```javascript
const bustWindow = document.createElement('div');
bustWindow.style.cssText = 'position:absolute;inset:0';
frame.append(bustWindow);
const unanchor = context.viewport.anchor('bust', bustWindow);
```

## Playing

Every call that plays names a published object. All of an object's views
play together: the same clips on the same tracks at the same clip times,
one speed and one set of driven values, so a stride carries across a change
of view. Their rigs need clips of the same names, cut to the same length.

An object plays its clips on layered tracks, as rig-c's live tracks do.
Track 0 is the base; each track above applies over what the tracks below
posed, lowest first. A clip on a track moves the pose toward its own by its
`Alpha` (0 to 1). An `Additive` clip instead adds its keyed values (times
its alpha) to what the tracks below posed: bone rotations, translations,
scales and shears, mesh deforms, transform constraint mixes, and physics
wind and gravity. Its colours, attachments and other values replace, as a
replacing clip's do. That is an effect layer: a flinch, a glow pulse or a
breathing layer over a walk. Each track crossfades on its own, and every
track's clip fires its events. Draw order and sequence frames follow track
0's clip.

- `Play`: a clip by name on a `Track` from its start, looping or once,
  fading in over `CrossfadeSeconds` from what that track played, at `Alpha`,
  `Additive` or replacing. A play on track 0 sets the object's `Speed` (1 as
  authored), which all its tracks share; a play on a higher track leaves it.
  Every view's rig must have the clip. `new PartsRigPlayRequest(id, clip)`
  loops a clip on track 0; `PartsRigPlayRequest.Layer(id, track, clip,
  alpha, additive)` layers one.
- `SetTrackAlpha`: change how strongly a track's clip applies while it
  plays; a clip it fades from keeps its own. Refused when the track plays
  nothing.
- `Stop`: fade a track out: track 0 back to the setup pose, a higher track
  to what the tracks below it pose.
- `SetSkin`: a skin by name in every view, or empty for the default skin
  alone.
- `PoseRate` (on the appearance): pose its clips that many times a second,
  each pose a whole step of clip time after the last, rather than at every
  call. A figure on a pixel grid posed at 8 a second keeps its blocks still
  between poses. A play, stop, skin change or driven value still poses at
  the next call, on the same beat.
- `DriveBone`: hold one of a bone's local values (rotation, x, y, scale or
  shear) over what the clips pose and before the constraints read it, in
  every view whose rig has the bone: a product's driving value such as
  Dagger's `turn` bone, which transform constraints read for the head turn,
  or a slider's dial, whose value picks the time its animation applies at
  (rig-c's value-driven face turn). `Release` lets the clips pose it again.

An object's motion in the world swings its physics (hair, cloth): what it
moved since the last call along its own x and y axes, in rig units, moves
every view's physics before its step, mirrored for a mirrored view. A
`Pixel`-sized rig takes none.

An object keeps its clip when it is published again, even when its renderer
node is made again (a new parent or layer). A new appearance over different
rigs starts from the setup pose.

## Reading back

The poses are evaluated as each call settles, at the Engine's presentation
time, so reads describe the pose of the previous call.

- `Read`: one track's clip as it stands (a clip played this call reads at
  clip time 0), its clip time, whether it loops, its alpha and whether it
  adds; and the object's views, speed, and whether it has been posed. `new
  PartsRigTrackRequest(id)` reads track 0.
- `ReadEvents`: the events rig objects' clips, on every track, passed since
  the previous update, object by object in time order: name (cut to 96
  bytes), key time, int and float values, volume and balance. A view set fires its first
  view's events. Events' string values are not carried.
- `ReadBone`: a named bone as one camera's reading draws it: the view that
  camera draws, the bone's position, rotation and scale in that view's rig
  units as authored, and its world position with the view's mirror and the
  plane turned to that camera by the billboard mode, the object where it
  was last posed. Publish an attachment there each update to keep it on the
  bone in that camera's view (the fixture keeps a marker on the head). The
  camera is taken as the call leaves it. A `Pixel`-sized rig is refused: its
  size follows each reading's viewport.

## Cost

Each view is posed every call that moves time, so a figure of four views
costs four poses. A rusty-dagger prototype townsperson rig (122 bones, 26
transform constraints) poses in about 8 µs on den-agents. On rig-c's
gallery rigs a path constraint on one bone adds about 0.5 µs a pose, and
`look`'s two sliders, each applying a clip of 16 bones and mesh deforms,
about 3 µs. An unchanged pose is not sent again, and each reading draws one
view per figure.

A figure on a pixel grid takes its smooth drawing out of the world pass into
`parts-rig-grid`, and adds the clear of the view-sized scratch image and its
block quad. Measured at 1920 × 1080 with 4× MSAA, with CraftSurvive's door
figure (21 pages) posed every frame (GPU milliseconds):

| | RTX 3080 | RX 9070 XT |
| --- | --- | --- |
| One figure, about 320 px tall, smooth | 0.12 | 0.10 |
| The same in 2-pixel blocks (world + `parts-rig-grid`) | 0.10 + 0.07 | 0.08 + 0.07 |
| Ten figures, 320 down to 100 px, smooth | 0.21 | 0.16 |
| The same in 2-pixel blocks | 0.10 + 0.16 | 0.09 + 0.14 |

So blocks cost about 0.05 ms more than smooth, for one figure or ten. A
32-colour palette adds up to 0.03 ms.
