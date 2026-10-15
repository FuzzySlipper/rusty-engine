//! Inspection views as Engine live-debug commands (`engine.inspect.view`):
//! Engine cameras beside the player view, posed by number or framed on a
//! committed scene object. The renderer draws them; nothing here changes the
//! product's cameras, poses or aim.
use super::*;

use render_host_contracts::{RendererCameraPose, RendererCameraProjection};
use render_wgpu::InspectionView;

pub(super) const VIEW_COMMAND: &str = "engine.inspect.view";
/// The product command `InspectionTargetsDebugModule` serves.
pub(super) const TARGETS_COMMAND: &str = "inspect.targets";

const VIEW_USAGE: &str = "usage: engine.inspect.view [list] | create ID [anchor NAME] | close ID | select ID | camera ID X Y Z YAW PITCH [FOV] | orbit ID DYAW DPITCH | pan ID DX DY | zoom ID FACTOR | frame ID front|back|left|right|side|top|underside|close HANDLE|camera|target NAME | capture ID PATH [WIDTH HEIGHT] [overlays]";

/// A new view's vertical field of view, near and far planes.
const VIEW_FOV_DEGREES: f64 = 50.0;
const VIEW_NEAR: f64 = 0.02;
const VIEW_FAR: f64 = 500.0;
/// Where a new view looks: this far in front of the player's camera, from
/// this far to its right and above.
const NEW_VIEW_FOCUS_AHEAD: f64 = 0.4;
const NEW_VIEW_SIDE: f64 = 1.0;
const NEW_VIEW_RISE: f64 = 0.2;
/// A framing leaves this much room around the object's bounds; a close
/// one comes this much nearer.
const FRAME_MARGIN: f64 = 1.15;
const CLOSE_FACTOR: f64 = 0.55;
/// What `frame ... camera` frames: the space just in front of the player's
/// camera, where a held assembly is.
const CAMERA_FRAME_AHEAD: f64 = 0.35;
const CAMERA_FRAME_RADIUS: f64 = 0.35;
/// A framing never looks straight up or down, where yaw is undefined.
const MAX_PITCH_DEGREES: f64 = 89.0;
/// A capture's size when it names none.
const DEFAULT_CAPTURE: (u32, u32) = (960, 540);

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn length(a: V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// The camera's forward, right and up for an Engine yaw (zero faces -Z,
/// positive turns toward +X) and pitch, in degrees.
fn basis(yaw: f64, pitch: f64) -> (V3, V3, V3) {
    let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians());
    let forward = [
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ];
    let right = [yaw.cos(), 0.0, yaw.sin()];
    let up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];
    (forward, right, up)
}

/// A pose at `eye` looking at `focus`.
fn looking_at(eye: V3, focus: V3) -> RendererCameraPose {
    let to = sub(focus, eye);
    let distance = length(to).max(1e-9);
    RendererCameraPose {
        position: eye,
        yaw_degrees: to[0].atan2(-to[2]).to_degrees(),
        pitch_degrees: (to[1] / distance)
            .clamp(-1.0, 1.0)
            .asin()
            .to_degrees()
            .clamp(-MAX_PITCH_DEGREES, MAX_PITCH_DEGREES),
    }
}

/// `point` turned by the unit quaternion `[x, y, z, w]`.
fn rotate([x, y, z, w]: [f64; 4], point: V3) -> V3 {
    let q = [x, y, z];
    let cross = |a: V3, b: V3| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let t = scale(cross(q, point), 2.0);
    add(add(point, scale(t, w)), cross(q, t))
}

fn fov_degrees(projection: &RendererCameraProjection) -> f64 {
    match projection {
        RendererCameraProjection::Perspective { fov_y_degrees, .. } => *fov_y_degrees,
        RendererCameraProjection::Orthographic { .. } => VIEW_FOV_DEGREES,
    }
}

fn number(word: &str) -> Result<f64, String> {
    word.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| VIEW_USAGE.to_owned())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewAnswer {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    anchor: Option<String>,
    pose: RendererCameraPose,
    projection: RendererCameraProjection,
    focus: V3,
    selected: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewsAnswer {
    views: Vec<ViewAnswer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected: Option<String>,
    /// The player's camera, which framings and new views are placed by.
    #[serde(skip_serializing_if = "Option::is_none")]
    player: Option<RendererCameraPose>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureAnswer {
    path: String,
    width: u32,
    height: u32,
    step: u64,
    held: bool,
    presentation_seconds: f64,
    view: ViewAnswer,
    #[serde(skip_serializing_if = "Option::is_none")]
    overlays: Option<std::collections::BTreeMap<String, u64>>,
}

pub(crate) fn view_answer(view: &InspectionView, selected: Option<&str>) -> ViewAnswer {
    ViewAnswer {
        id: view.id.clone(),
        anchor: view.anchor.clone(),
        pose: view.pose,
        projection: view.projection,
        focus: view.focus,
        selected: selected == Some(view.id.as_str()),
    }
}

impl CsharpProductRuntime {
    /// Runs a command through the product's debug catalog, as one product
    /// call.
    pub(super) fn execute_product_debug(
        &mut self,
        command: &str,
    ) -> Result<ProductHostRuntimeReceipt<ProductHostDebugResult>, ProductHostRuntimeError> {
        let (execute, release) = (self.api.execute_debug, self.api.release_debug_result);

        // Debug commands may use ordinary generated Engine services, and their
        // changes stay like any other call's.
        self.services.begin_call(ui_binding(&self.lifecycle));
        let callback_result = call_debug(execute, release, self.handle, command);
        let finished = self.finish_product_call(callback_result.as_ref().err().cloned());
        self.settle_gameplay_time();
        match (callback_result, finished.failure) {
            (Ok(result), None) => Ok(ProductHostRuntimeReceipt::new(result, finished.outputs)),
            (_, Some(failure)) => {
                // The next receipt carries what the command did publish.
                self.pending_recovery_outputs.extend(finished.outputs);
                Err(self.runtime_error(failure))
            }
            (Err(_), None) => unreachable!("a callback error is the call's failure"),
        }
    }

    /// Answers `engine.inspect.view`; none for any other command. `targets`
    /// is the product's `inspect.targets` answer, for a framing on a target.
    pub(super) fn execute_inspect_view(
        &self,
        command: &str,
        targets: Option<&str>,
    ) -> Option<ProductHostDebugResult> {
        let words: Vec<&str> = command.split_whitespace().collect();
        let (first, rest) = words.split_first()?;
        if *first != VIEW_COMMAND {
            return None;
        }
        let Some(frames) = self.frame_output.as_ref() else {
            return Some(ProductHostDebugResult::new(
                false,
                "INSPECT_NO_RENDERER: this runtime draws no frames".to_owned(),
            ));
        };
        let driver = frames.driver();
        let answer = match rest {
            ["capture", id, path, more @ ..] => self.capture_view(&driver, id, path, more),
            _ => self.change_views(&driver, rest, targets).and_then(|()| {
                let selected = driver.selected_inspection_view();
                serde_json::to_string(&ViewsAnswer {
                    views: driver
                        .inspection_views()
                        .iter()
                        .map(|view| view_answer(view, selected.as_deref()))
                        .collect(),
                    selected,
                    player: driver.primary_camera().map(|(pose, _)| pose),
                })
                .map_err(|error| error.to_string())
            }),
        };
        Some(match answer {
            Ok(message) => ProductHostDebugResult::new(true, message),
            Err(message) => ProductHostDebugResult::new(false, message),
        })
    }

    fn change_views(
        &self,
        driver: &render_wgpu::SceneDriver,
        words: &[&str],
        targets: Option<&str>,
    ) -> Result<(), String> {
        let mut views = driver.inspection_views();
        let find = |views: &mut Vec<InspectionView>, id: &str| -> Result<usize, String> {
            views
                .iter()
                .position(|view| view.id == id)
                .ok_or_else(|| format!("INSPECT_NO_VIEW: no inspection view {id}"))
        };
        match words {
            [] | ["list"] => return Ok(()),
            ["create", id, more @ ..] => {
                let anchor = match more {
                    [] => None,
                    ["anchor", name] => Some((*name).to_owned()),
                    _ => return Err(VIEW_USAGE.to_owned()),
                };
                if views.iter().any(|view| view.id == *id) {
                    return Err(format!("INSPECT_VIEW_EXISTS: inspection view {id} exists"));
                }
                views.push(self.new_view(driver, id, anchor));
            }
            ["close", id] => {
                let index = find(&mut views, id)?;
                views.remove(index);
                if driver.selected_inspection_view().as_deref() == Some(*id) {
                    driver.select_inspection_view(None);
                }
            }
            ["select", id] => {
                find(&mut views, id)?;
                driver.select_inspection_view(Some((*id).to_owned()));
                return Ok(());
            }
            ["camera", id, x, y, z, yaw, pitch, fov @ ..] => {
                let index = find(&mut views, id)?;
                let view = &mut views[index];
                view.pose = RendererCameraPose {
                    position: [number(x)?, number(y)?, number(z)?],
                    yaw_degrees: number(yaw)?,
                    pitch_degrees: number(pitch)?,
                };
                if let [fov] = fov {
                    view.projection = RendererCameraProjection::Perspective {
                        fov_y_degrees: number(fov)?.clamp(1.0, 170.0),
                        near: VIEW_NEAR,
                        far: VIEW_FAR,
                    };
                }
                // The focus stays ahead of the camera, as far as it was.
                let distance = length(sub(view.focus, view.pose.position)).max(VIEW_NEAR);
                let (forward, ..) = basis(view.pose.yaw_degrees, view.pose.pitch_degrees);
                view.focus = add(view.pose.position, scale(forward, distance));
            }
            ["orbit", id, yaw, pitch] => {
                let index = find(&mut views, id)?;
                let view = &mut views[index];
                let distance = length(sub(view.focus, view.pose.position));
                let yaw = view.pose.yaw_degrees + number(yaw)?;
                let pitch = (view.pose.pitch_degrees + number(pitch)?)
                    .clamp(-MAX_PITCH_DEGREES, MAX_PITCH_DEGREES);
                let (forward, ..) = basis(yaw, pitch);
                view.pose = RendererCameraPose {
                    position: sub(view.focus, scale(forward, distance)),
                    yaw_degrees: yaw,
                    pitch_degrees: pitch,
                };
            }
            ["pan", id, dx, dy] => {
                let index = find(&mut views, id)?;
                let view = &mut views[index];
                let (_, right, up) = basis(view.pose.yaw_degrees, view.pose.pitch_degrees);
                let shift = add(scale(right, number(dx)?), scale(up, number(dy)?));
                view.pose.position = add(view.pose.position, shift);
                view.focus = add(view.focus, shift);
            }
            ["zoom", id, factor] => {
                let index = find(&mut views, id)?;
                let view = &mut views[index];
                let factor = number(factor)?;
                if factor <= 0.0 {
                    return Err(VIEW_USAGE.to_owned());
                }
                let offset = sub(view.pose.position, view.focus);
                view.pose.position = add(view.focus, scale(offset, factor));
            }
            ["frame", id, framing, "target", name] => {
                let index = find(&mut views, id)?;
                let (center, radius, rotation) = target_framed(driver, targets, name)?;
                frame(&mut views[index], framing, center, radius, rotation)?;
            }
            ["frame", id, framing, subject] => {
                let index = find(&mut views, id)?;
                let (center, radius, rotation) = self.framed(driver, subject)?;
                frame(&mut views[index], framing, center, radius, rotation)?;
            }
            _ => return Err(VIEW_USAGE.to_owned()),
        }
        driver.set_inspection_views(views);
        Ok(())
    }

    /// A new view to the right of the player's camera, looking at the space
    /// in front of it; at the origin without one.
    fn new_view(
        &self,
        driver: &render_wgpu::SceneDriver,
        id: &str,
        anchor: Option<String>,
    ) -> InspectionView {
        let (focus, eye) = match driver.primary_camera() {
            Some((pose, _)) => {
                let (forward, right, up) = basis(pose.yaw_degrees, pose.pitch_degrees);
                let focus = add(pose.position, scale(forward, NEW_VIEW_FOCUS_AHEAD));
                let eye = add(
                    add(focus, scale(right, NEW_VIEW_SIDE)),
                    scale(up, NEW_VIEW_RISE),
                );
                (focus, eye)
            }
            None => ([0.0; 3], [NEW_VIEW_SIDE, NEW_VIEW_RISE, NEW_VIEW_SIDE]),
        };
        InspectionView {
            id: id.to_owned(),
            pose: looking_at(eye, focus),
            projection: RendererCameraProjection::Perspective {
                fov_y_degrees: VIEW_FOV_DEGREES,
                near: VIEW_NEAR,
                far: VIEW_FAR,
            },
            anchor,
            focus,
        }
    }

    /// The world centre, radius and frame of what `subject` names: a
    /// committed object's bounds and rotation (a viewmodel object's placed
    /// by the player's camera), or the space in front of the player's
    /// camera.
    fn framed(
        &self,
        driver: &render_wgpu::SceneDriver,
        subject: &str,
    ) -> Result<(V3, f64, [f64; 4]), String> {
        let player = driver.primary_camera();
        let player_frame = |pose: &RendererCameraPose| {
            let half_yaw = -pose.yaw_degrees.to_radians() * 0.5;
            let half_pitch = pose.pitch_degrees.to_radians() * 0.5;
            // Yaw about y, then pitch about x, as camera poses turn.
            let (sy, cy, sp, cp) = (
                half_yaw.sin(),
                half_yaw.cos(),
                half_pitch.sin(),
                half_pitch.cos(),
            );
            [cy * sp, sy * cp, -sy * sp, cy * cp]
        };
        if subject == "camera" {
            let (pose, _) = player.ok_or("INSPECT_NO_CAMERA: the product has no primary camera")?;
            let (forward, ..) = basis(pose.yaw_degrees, pose.pitch_degrees);
            return Ok((
                add(pose.position, scale(forward, CAMERA_FRAME_AHEAD)),
                CAMERA_FRAME_RADIUS,
                player_frame(&pose),
            ));
        }
        let handle: u64 = subject.parse().map_err(|_| VIEW_USAGE.to_owned())?;
        let world = self.services.presentation_world();
        let object = world
            .scene_object(render_model::RenderHandle::new(handle))
            .ok_or_else(|| format!("no committed object {handle}"))?;
        let as_f64 = |value: [f32; 3]| value.map(f64::from);
        let rotation = object.world.rotation.map(f64::from);
        let (center, radius) = match object.bounds {
            Some(bounds) => {
                let (min, max) = (as_f64(bounds.world_min), as_f64(bounds.world_max));
                (scale(add(min, max), 0.5), length(sub(max, min)) * 0.5)
            }
            None => (as_f64(object.world.translation), CAMERA_FRAME_RADIUS),
        };
        if object.layer != render_model::RenderLayer::Viewmodel {
            return Ok((center, radius.max(VIEW_NEAR), rotation));
        }
        // A viewmodel object is placed in the player's camera space.
        let (pose, _) = player.ok_or("INSPECT_NO_CAMERA: the product has no primary camera")?;
        let frame = player_frame(&pose);
        let [x, y, z, w] = frame;
        let [ox, oy, oz, ow] = rotation;
        let composed = [
            w * ox + x * ow + y * oz - z * oy,
            w * oy - x * oz + y * ow + z * ox,
            w * oz + x * oy - y * ox + z * ow,
            w * ow - x * ox - y * oy - z * oz,
        ];
        Ok((
            add(pose.position, rotate(frame, center)),
            radius.max(VIEW_NEAR),
            composed,
        ))
    }

    fn capture_view(
        &self,
        driver: &render_wgpu::SceneDriver,
        id: &str,
        path: &str,
        more: &[&str],
    ) -> Result<String, String> {
        let (size, overlays) = match more {
            [] => (DEFAULT_CAPTURE, false),
            ["overlays"] => (DEFAULT_CAPTURE, true),
            [width, height, rest @ ..] => {
                let side = |word: &str| word.parse::<u32>().ok().filter(|side| *side >= 1);
                let size = side(width).zip(side(height)).ok_or(VIEW_USAGE)?;
                match rest {
                    [] => (size, false),
                    ["overlays"] => (size, true),
                    _ => return Err(VIEW_USAGE.to_owned()),
                }
            }
            _ => return Err(VIEW_USAGE.to_owned()),
        };
        let capture = driver
            .capture_view(id, Some(size), overlays)
            .ok_or_else(|| format!("INSPECT_NO_VIEW: no inspection view {id}"))?;
        let png = render_wgpu::encode_png(capture.width, capture.height, &capture.rgba)?;
        std::fs::write(path, png).map_err(|error| format!("{path}: {error}"))?;
        let view = capture
            .view
            .as_ref()
            .expect("a view capture names its view");
        serde_json::to_string(&CaptureAnswer {
            path: path.to_owned(),
            width: capture.width,
            height: capture.height,
            step: capture.step,
            held: capture.held,
            presentation_seconds: capture.presentation_seconds,
            view: view_answer(view, driver.selected_inspection_view().as_deref()),
            overlays: capture.overlays,
        })
        .map_err(|error| error.to_string())
    }
}

/// `view` framed on `center`: along `rotation`'s axes for a named framing,
/// or nearer from where it is for `close`.
fn frame(
    view: &mut InspectionView,
    framing: &str,
    center: V3,
    radius: f64,
    rotation: [f64; 4],
) -> Result<(), String> {
    let current = sub(view.pose.position, view.focus);
    let direction = match framing {
        "front" => rotate(rotation, [0.0, 0.0, 1.0]),
        "back" => rotate(rotation, [0.0, 0.0, -1.0]),
        "left" => rotate(rotation, [-1.0, 0.0, 0.0]),
        "right" | "side" => rotate(rotation, [1.0, 0.0, 0.0]),
        "top" => rotate(rotation, [0.0, 1.0, 0.0]),
        "underside" => rotate(rotation, [0.0, -1.0, 0.0]),
        "close" => scale(current, 1.0 / length(current).max(1e-9)),
        _ => return Err(VIEW_USAGE.to_owned()),
    };
    let half_fov = (fov_degrees(&view.projection) * 0.5).to_radians();
    let fit = radius / half_fov.sin() * FRAME_MARGIN;
    let distance = if framing == "close" {
        fit * CLOSE_FACTOR
    } else {
        fit
    };
    view.pose = looking_at(add(center, scale(direction, distance)), center);
    view.focus = center;
    Ok(())
}

/// The product's `inspect.targets.v1` list, as far as framing reads it.
#[derive(serde::Deserialize)]
struct TargetList {
    targets: Vec<TargetEntry>,
}

#[derive(serde::Deserialize)]
struct TargetEntry {
    name: String,
    space: TargetSpace,
    point: [f32; 3],
    rotation: [f32; 4],
    radius: f32,
}

#[derive(serde::Deserialize)]
struct TargetSpace {
    kind: String,
    #[serde(default)]
    entity: u64,
    #[serde(default)]
    joint: u32,
}

/// The world centre, radius and frame of the product's target `name`.
fn target_framed(
    driver: &render_wgpu::SceneDriver,
    targets: Option<&str>,
    name: &str,
) -> Result<(V3, f64, [f64; 4]), String> {
    let listed = targets
        .ok_or("INSPECT_NO_TARGETS: the product registers no InspectionTargetsDebugModule")?;
    let list: TargetList = serde_json::from_str(listed)
        .map_err(|error| format!("INSPECT_NO_TARGETS: inspect.targets answered {error}"))?;
    let target = list
        .targets
        .iter()
        .find(|target| target.name == name)
        .ok_or_else(|| format!("INSPECT_NO_TARGET: no inspection target {name}"))?;
    let space = match target.space.kind.as_str() {
        "world" => render_model::OverlaySpace::World,
        "model" => render_model::OverlaySpace::Model {
            entity: target.space.entity,
        },
        "joint" => render_model::OverlaySpace::Joint {
            entity: target.space.entity,
            joint: target.space.joint,
        },
        _ => render_model::OverlaySpace::Camera,
    };
    let (point, rotation) = driver
        .resolve_point(space, target.point, target.rotation)
        .ok_or_else(|| format!("INSPECT_TARGET_UNPLACED: target {name} is on nothing drawn"))?;
    Ok((
        point.map(f64::from),
        f64::from(target.radius).max(VIEW_NEAR),
        rotation.map(f64::from),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pose_looking_at_a_point_faces_it() {
        let pose = looking_at([1.0, 1.0, 1.0], [0.0, 1.0, 0.0]);
        let (forward, ..) = basis(pose.yaw_degrees, pose.pitch_degrees);
        let to = sub([0.0, 1.0, 0.0], [1.0, 1.0, 1.0]);
        let to = scale(to, 1.0 / length(to));
        assert!(
            (0..3).all(|axis| (forward[axis] - to[axis]).abs() < 1e-9),
            "{forward:?}"
        );
        let (_, right, up) = basis(0.0, 0.0);
        assert_eq!((right, up), ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]));
        assert!(
            (looking_at([0.0, 5.0, 0.0], [0.0; 3]).pitch_degrees + MAX_PITCH_DEGREES).abs() < 1e-9
        );
    }

    #[test]
    fn a_quarter_turn_about_y_takes_z_to_x() {
        let half = std::f64::consts::FRAC_PI_4;
        let turned = rotate([0.0, half.sin(), 0.0, half.cos()], [0.0, 0.0, 1.0]);
        assert!(
            (turned[0] - 1.0).abs() < 1e-9 && turned[2].abs() < 1e-9,
            "{turned:?}"
        );
    }
}
