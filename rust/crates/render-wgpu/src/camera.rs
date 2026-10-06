//! Cameras: view and projection matrices from the committed `CameraView`
//! facts, the camera-relative viewmodel camera, and renderer-only camera
//! motion between product updates.
//!
//! There is one camera authority, the composition the runtime publishes. This
//! module only turns those facts into matrices. Motion interpolates between
//! published samples on the presentation time the host passes in; the backend
//! reads no clock, so a held simulation (no new samples) presents its last
//! pose.

use std::collections::VecDeque;

use glam::{Mat3, Mat4, Quat, Vec3};

use crate::convert::{world_array, world_vec3};
use render_host_contracts::{
    RendererCameraBasis, RendererCameraInterpolation, RendererCameraMotion, RendererCameraPose,
    RendererCameraProjection, RendererCompositionCamera,
};

/// Matrices and eye position for one view pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CameraMatrices {
    pub view_proj: Mat4,
    /// World to view.
    pub view: Mat4,
    pub projection: Mat4,
    pub eye: Vec3,
}

/// Position and orientation of a camera. Orientation maps camera-local axes
/// to world axes; the camera looks down its local -Z with +Y up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CameraPose {
    pub position: Vec3,
    pub orientation: Quat,
}

/// The descriptor's published pose. Engine yaw zero faces -Z and positive yaw
/// turns toward +X; an explicit basis overrides yaw and pitch.
pub(crate) fn descriptor_pose(camera: &RendererCompositionCamera) -> CameraPose {
    match &camera.basis {
        Some(basis) => {
            let forward = world_vec3(basis.forward).normalize_or(Vec3::NEG_Z);
            let right = forward.cross(world_vec3(basis.up)).normalize_or(Vec3::X);
            let up = right.cross(forward);
            CameraPose {
                position: world_vec3(camera.pose.position),
                orientation: Quat::from_mat3(&Mat3::from_cols(right, up, -forward)),
            }
        }
        None => pose_from_degrees(&camera.pose),
    }
}

/// Yaw 0 faces -Z, positive yaw turns right and positive pitch looks up.
pub(crate) fn pose_from_degrees(pose: &RendererCameraPose) -> CameraPose {
    CameraPose {
        position: world_vec3(pose.position),
        orientation: Quat::from_rotation_y(-(pose.yaw_degrees as f32).to_radians())
            * Quat::from_rotation_x((pose.pitch_degrees as f32).to_radians()),
    }
}

/// A pose as the Engine describes cameras: position, yaw (zero faces -Z,
/// positive turns toward +X) and pitch in degrees, and its basis. The inverse
/// of [`pose_from_degrees`] for poses without roll.
pub(crate) fn pose_readout(pose: CameraPose) -> (RendererCameraPose, RendererCameraBasis) {
    let forward = pose.orientation * Vec3::NEG_Z;
    let right = pose.orientation * Vec3::X;
    let up = pose.orientation * Vec3::Y;
    (
        RendererCameraPose {
            position: world_array(pose.position),
            yaw_degrees: f64::from(forward.x.atan2(-forward.z).to_degrees()),
            pitch_degrees: f64::from(forward.y.clamp(-1.0, 1.0).asin().to_degrees()),
        },
        RendererCameraBasis {
            forward: world_array(forward),
            right: world_array(right),
            up: world_array(up),
        },
    )
}

pub(crate) fn projection_matrix(projection: &RendererCameraProjection, aspect: f32) -> Mat4 {
    match *projection {
        RendererCameraProjection::Perspective {
            fov_y_degrees,
            near,
            far,
        } => Mat4::perspective_rh(
            (fov_y_degrees as f32).to_radians(),
            aspect,
            near as f32,
            far as f32,
        ),
        RendererCameraProjection::Orthographic {
            vertical_size,
            near,
            far,
        } => {
            let half_height = vertical_size as f32 * 0.5;
            let half_width = half_height * aspect;
            Mat4::orthographic_rh(
                -half_width,
                half_width,
                -half_height,
                half_height,
                near as f32,
                far as f32,
            )
        }
    }
}

pub(crate) fn camera_matrices(
    pose: CameraPose,
    projection: &RendererCameraProjection,
    aspect: f32,
) -> CameraMatrices {
    let view = Mat4::look_to_rh(
        pose.position,
        pose.orientation * Vec3::NEG_Z,
        pose.orientation * Vec3::Y,
    );
    let projection = projection_matrix(projection, aspect);
    CameraMatrices {
        view_proj: projection * view,
        view,
        projection,
        eye: pose.position,
    }
}

/// The viewmodel camera: camera-relative content is authored in camera-local
/// coordinates, so it sits at the origin looking down -Z with the world
/// camera's projection, or with `fov_y_degrees` in place of a perspective
/// camera's field of view when it is above 0 (hands and a weapon
/// conventionally draw narrower than the world).
pub(crate) fn viewmodel_matrices(
    projection: &RendererCameraProjection,
    fov_y_degrees: f64,
    aspect: f32,
) -> CameraMatrices {
    let projection = match *projection {
        RendererCameraProjection::Perspective { near, far, .. } if fov_y_degrees > 0.0 => {
            RendererCameraProjection::Perspective {
                fov_y_degrees,
                near,
                far,
            }
        }
        projection => projection,
    };
    camera_matrices(
        CameraPose {
            position: Vec3::ZERO,
            orientation: Quat::IDENTITY,
        },
        &projection,
        aspect,
    )
}

/// Renderer storage for motion history. If the requested delay needs more,
/// the oldest retained sample holds.
const HISTORY_CAPACITY: usize = 64;

#[derive(Debug, Clone, Copy)]
struct Sample {
    time: f64,
    pose: CameraPose,
}

/// Interpolation history for one camera. It never feeds interpolated state
/// back to the simulation.
#[derive(Debug, Default)]
pub(crate) struct CameraMotion {
    samples: VecDeque<Sample>,
    motion: Option<RendererCameraMotion>,
    offset: f64,
    cursor: f64,
    latest: Option<Sample>,
    received_at: f64,
}

/// What a readout reports about one camera's motion.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraSampleReadout {
    pub camera_id: String,
    pub sample_id: Option<String>,
    pub source_time_seconds: Option<f64>,
    pub presentation_time_seconds: Option<f64>,
    pub retained_samples: usize,
}

impl CameraMotion {
    /// Take a published descriptor that arrived at `arrival_seconds`
    /// (presentation time).
    pub fn receive(&mut self, camera: &RendererCompositionCamera, arrival_seconds: f64) {
        let motion = camera.motion.as_ref();
        if let (Some(motion), Some(current)) = (motion, &self.motion) {
            if motion.sample_id == current.sample_id {
                return;
            }
        }
        let next = Sample {
            time: motion.map_or(0.0, |motion| motion.sample_time_seconds),
            pose: descriptor_pose(camera),
        };
        // A paused source clock, or a delivery stall longer than the buffer,
        // must not leave the local clock permanently ahead of the samples.
        let discontinuity = match (motion, &self.latest) {
            (Some(motion), Some(latest)) => {
                arrival_seconds - self.received_at - (next.time - latest.time)
                    > 2.0 * motion.delay_seconds
            }
            _ => false,
        };
        let reset = match (motion, &self.motion) {
            (Some(motion), Some(current)) => {
                discontinuity
                    || motion.cut
                    || self.latest.is_some_and(|latest| next.time <= latest.time)
                    || motion.interpolation != current.interpolation
                    || motion.delay_seconds != current.delay_seconds
            }
            _ => true,
        };
        self.motion = motion.cloned();
        self.received_at = arrival_seconds;
        self.latest = Some(next);
        if reset {
            self.samples.clear();
            self.samples.push_back(next);
            self.offset = arrival_seconds - next.time;
            self.cursor = next.time;
        } else {
            self.offset = self.offset.min(arrival_seconds - next.time);
            self.samples.push_back(next);
            if self.samples.len() > HISTORY_CAPACITY {
                self.samples.pop_front();
            }
        }
    }

    /// The pose to present at `now_seconds`. Without motion this is the
    /// latest published pose.
    pub fn pose(&mut self, now_seconds: f64) -> Option<CameraPose> {
        let latest = self.latest?;
        let Some(motion) = &self.motion else {
            return Some(latest.pose);
        };
        self.cursor = self.cursor.max(
            latest
                .time
                .min(now_seconds - self.offset - motion.delay_seconds),
        );
        while self.samples.len() > 2 && self.samples[1].time <= self.cursor {
            self.samples.pop_front();
        }
        let first = self.samples[0];
        let second = self.samples.get(1).copied().unwrap_or(first);
        let fraction = if second.time > first.time {
            ((self.cursor - first.time) / (second.time - first.time)).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };
        let orientation = match motion.interpolation {
            RendererCameraInterpolation::Pose => first
                .pose
                .orientation
                .slerp(second.pose.orientation, fraction),
            RendererCameraInterpolation::Position => latest.pose.orientation,
        };
        Some(CameraPose {
            position: first.pose.position.lerp(second.pose.position, fraction),
            orientation,
        })
    }

    pub fn readout(&self, camera_id: &str) -> CameraSampleReadout {
        CameraSampleReadout {
            camera_id: camera_id.to_owned(),
            sample_id: self.motion.as_ref().map(|motion| motion.sample_id.clone()),
            source_time_seconds: self
                .motion
                .as_ref()
                .map(|motion| motion.sample_time_seconds),
            presentation_time_seconds: self.motion.as_ref().map(|_| self.cursor),
            retained_samples: self.samples.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_host_contracts::{RendererCameraBasis, RendererCameraPose};

    fn camera(
        position: [f64; 3],
        yaw: f64,
        motion: Option<(&str, f64)>,
    ) -> RendererCompositionCamera {
        RendererCompositionCamera {
            id: "camera".to_owned(),
            pose: RendererCameraPose {
                position,
                pitch_degrees: 0.0,
                yaw_degrees: yaw,
            },
            basis: None,
            projection: RendererCameraProjection::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 100.0,
            },
            motion: motion.map(|(id, time)| RendererCameraMotion {
                sample_id: id.to_owned(),
                sample_time_seconds: time,
                delay_seconds: 0.1,
                interpolation: RendererCameraInterpolation::Pose,
                cut: false,
            }),
            viewmodel_fov_y_degrees: 0.0,
        }
    }

    #[test]
    fn yaw_pitch_and_basis_agree() {
        let mut from_yaw = camera([0.0; 3], 90.0, None);
        from_yaw.pose.pitch_degrees = 30.0;
        let pose = descriptor_pose(&from_yaw);
        let forward = pose.orientation * Vec3::NEG_Z;
        let up = pose.orientation * Vec3::Y;
        // Yaw 90 turns toward +X; pitch 30 looks up.
        assert!((forward - Vec3::new(0.866, 0.5, 0.0)).length() < 1e-3);
        let mut from_basis = from_yaw.clone();
        from_basis.basis = Some(RendererCameraBasis {
            forward: forward.as_dvec3().to_array(),
            right: (forward.cross(up)).as_dvec3().to_array(),
            up: up.as_dvec3().to_array(),
        });
        let other = descriptor_pose(&from_basis);
        assert!(pose.orientation.angle_between(other.orientation) < 1e-3);
    }

    #[test]
    fn motion_interpolates_on_the_host_clock_and_holds_the_last_sample() {
        let mut motion = CameraMotion::default();
        motion.receive(&camera([0.0; 3], 0.0, Some(("a", 1.0))), 10.0);
        motion.receive(&camera([2.0, 0.0, 0.0], 0.0, Some(("b", 1.1))), 10.1);
        // Delay 0.1: at 10.15 the cursor is at source time 1.05, halfway.
        let halfway = motion.pose(10.15).unwrap();
        assert!((halfway.position.x - 1.0).abs() < 1e-3);
        // No new sample (a held simulation): the latest pose holds forever.
        let held = motion.pose(99.0).unwrap();
        assert!((held.position.x - 2.0).abs() < 1e-6);
        // The same sample id again is not a new sample.
        motion.receive(&camera([9.0; 3], 0.0, Some(("b", 1.1))), 100.0);
        assert!((motion.pose(100.0).unwrap().position.x - 2.0).abs() < 1e-6);
    }

    #[test]
    fn looking_around_a_held_world_presents_each_new_view_at_once() {
        // World time stops at 1.1 (gameplay held) while the player keeps
        // turning: each new sample shares the source time, so it replaces
        // the history and presents now, without waiting for a world step or
        // easing back toward an earlier view.
        let mut motion = CameraMotion::default();
        motion.receive(&camera([0.0; 3], 0.0, Some(("a", 1.0))), 10.0);
        motion.receive(&camera([2.0, 0.0, 0.0], 0.0, Some(("b", 1.1))), 10.1);
        for (index, yaw) in [30.0, 60.0, 90.0].into_iter().enumerate() {
            let arrival = 10.2 + index as f64 * 0.016;
            let id = format!("held-{index}");
            motion.receive(&camera([2.0, 0.0, 0.0], yaw, Some((&id, 1.1))), arrival);
            let pose = motion.pose(arrival).unwrap();
            let expected = descriptor_pose(&camera([2.0, 0.0, 0.0], yaw, None));
            assert!(pose.orientation.angle_between(expected.orientation) < 1e-4);
            assert!((pose.position.x - 2.0).abs() < 1e-6);
        }
        // World time resumes: interpolation continues from the held view.
        motion.receive(&camera([4.0, 0.0, 0.0], 90.0, Some(("c", 1.2))), 10.3);
        let resumed = motion.pose(10.35).unwrap();
        assert!(resumed.position.x >= 2.0 && resumed.position.x <= 4.0);
    }

    #[test]
    fn a_camera_without_motion_presents_its_latest_pose() {
        let mut motion = CameraMotion::default();
        motion.receive(&camera([1.0, 2.0, 3.0], 0.0, None), 0.0);
        motion.receive(&camera([4.0, 5.0, 6.0], 0.0, None), 0.0);
        assert_eq!(motion.pose(0.0).unwrap().position, Vec3::new(4.0, 5.0, 6.0));
    }
}
