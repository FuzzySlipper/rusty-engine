//! Pose controls (`AnimatedMeshPose`) over a sampled clip pose: two-bone IK,
//! then joint overrides, composed into instance-space node transforms.
//! Skinning and joint attachments read the result.

use std::collections::HashMap;

use glam::{Mat4, Quat, Vec3};
use render_model::{AnimatedMeshPose, JointOverride, PoseSpace, TwoBoneIkConstraint};

use crate::convert;
use crate::glb::{GlbModel, Trs};

/// Lengths below this are degenerate (metres, or the rig's units).
const DEGENERATE: f32 = 1e-6;
/// How far short of full extension (and of full fold) an IK chain stops, as
/// a fraction of its length, so the bend plane stays defined.
const REACH_MARGIN: f32 = 1e-4;

/// Instance-space transforms of `locals`, parents first. Nodes outside the
/// scene keep the identity.
pub(crate) fn compose(model: &GlbModel, locals: &[Trs]) -> Vec<Mat4> {
    let mut posed = vec![Mat4::IDENTITY; model.nodes.len()];
    for &index in &model.order {
        let local = locals[index].matrix();
        posed[index] = match model.nodes[index].parent {
            Some(parent) => posed[parent] * local,
            None => local,
        };
    }
    posed
}

/// The sampled `locals` under `pose`: IK in order, then overrides, then
/// composed. `rig` maps rig joint indices to nodes; `world` places the
/// instance, for `World` values. A joint the rig does not name is skipped.
pub(crate) fn controlled(
    model: &GlbModel,
    rig: &[Option<usize>],
    mut locals: Vec<Trs>,
    pose: &AnimatedMeshPose,
    world: Mat4,
) -> Vec<Mat4> {
    if !pose.controls() {
        return compose(model, &locals);
    }
    let to_model = world.inverse();
    let node = |joint: u32| rig.get(joint as usize).copied().flatten();
    for ik in &pose.two_bone_ik {
        if let (Some(root), Some(mid), Some(end)) = (node(ik.root), node(ik.mid), node(ik.end)) {
            solve_two_bone(model, &mut locals, [root, mid, end], ik, to_model);
        }
    }
    let mut by_node: HashMap<usize, Vec<&JointOverride>> = HashMap::new();
    for joint in &pose.overrides {
        if let Some(index) = node(joint.joint) {
            by_node.entry(index).or_default().push(joint);
        }
    }
    let places = |joint: &JointOverride| joint.space != PoseSpace::Local;
    if !by_node.values().flatten().any(|joint| places(joint)) {
        return evaluate(model, &locals, &by_node, true, world, to_model).0;
    }
    // A placing value's weight mixes its joint's local transform between the
    // pose without placements and the placed pose, so placed joints blend as
    // one pose and descendants keep their shape.
    let (_, unplaced) = evaluate(model, &locals, &by_node, false, world, to_model);
    let (_, placed) = evaluate(model, &locals, &by_node, true, world, to_model);
    let blended: Vec<Trs> = (0..model.nodes.len())
        .map(|index| {
            let weight = by_node
                .get(&index)
                .and_then(|overrides| overrides.iter().rfind(|joint| places(joint)))
                .map(|joint| joint.weight);
            match weight {
                Some(weight) => Trs {
                    translation: unplaced[index]
                        .translation
                        .lerp(placed[index].translation, weight),
                    rotation: unplaced[index]
                        .rotation
                        .slerp(placed[index].rotation, weight)
                        .normalize(),
                    scale: unplaced[index].scale.lerp(placed[index].scale, weight),
                },
                None => placed[index],
            }
        })
        .collect();
    compose(model, &blended)
}

/// Each node's overrides in list order, parents first: a `Local` value at its
/// weight, a `Model` or `World` value at full weight when `placing` (else
/// skipped). Returns the placed transforms and the local transforms that
/// produce them.
fn evaluate(
    model: &GlbModel,
    locals: &[Trs],
    by_node: &HashMap<usize, Vec<&JointOverride>>,
    placing: bool,
    world: Mat4,
    to_model: Mat4,
) -> (Vec<Mat4>, Vec<Trs>) {
    let mut posed = vec![Mat4::IDENTITY; model.nodes.len()];
    let mut resolved = locals.to_vec();
    for &index in &model.order {
        let parent = model.nodes[index].parent.map(|parent| posed[parent]);
        let compose = |local: &Trs| parent.map_or(local.matrix(), |parent| parent * local.matrix());
        let mut local = locals[index];
        let mut placed = compose(&local);
        for joint in by_node.get(&index).map_or(&[][..], Vec::as_slice) {
            if joint.space == PoseSpace::Local {
                override_local(&mut local, joint);
                placed = compose(&local);
            } else if placing {
                placed = override_placed(placed, joint, world, to_model, 1.0);
                local = trs(parent.map_or(placed, |parent| parent.inverse() * placed));
            }
        }
        posed[index] = placed;
        resolved[index] = local;
    }
    (posed, resolved)
}

fn trs(matrix: Mat4) -> Trs {
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    Trs {
        translation,
        rotation: rotation.normalize(),
        scale,
    }
}

fn override_local(local: &mut Trs, joint: &JointOverride) {
    if let Some(rotation) = joint.rotation {
        let rotation = convert::rotation(rotation);
        let target = if joint.additive {
            local.rotation * rotation
        } else {
            rotation
        };
        local.rotation = local.rotation.slerp(target, joint.weight).normalize();
    }
    if let Some(translation) = joint.translation {
        let translation = convert::vec3(translation);
        let target = if joint.additive {
            local.translation + translation
        } else {
            translation
        };
        local.translation = local.translation.lerp(target, joint.weight);
    }
}

/// `placed` (instance space) moved toward a `Model` or `World` value by
/// `weight`; the joint keeps its scale.
fn override_placed(
    placed: Mat4,
    joint: &JointOverride,
    world: Mat4,
    to_model: Mat4,
    weight: f32,
) -> Mat4 {
    let (scale, rotation, translation) = placed.to_scale_rotation_translation();
    let (target_rotation, target_translation) = match joint.space {
        PoseSpace::World => {
            let (_, world_rotation, world_translation) =
                (world * placed).to_scale_rotation_translation();
            let target = Mat4::from_rotation_translation(
                joint.rotation.map_or(world_rotation, convert::rotation),
                joint.translation.map_or(world_translation, convert::vec3),
            );
            let (_, rotation, translation) = (to_model * target).to_scale_rotation_translation();
            (rotation, translation)
        }
        _ => (
            joint.rotation.map_or(rotation, convert::rotation),
            joint.translation.map_or(translation, convert::vec3),
        ),
    };
    let rotation = if joint.rotation.is_some() {
        rotation.slerp(target_rotation, weight).normalize()
    } else {
        rotation
    };
    let translation = if joint.translation.is_some() {
        translation.lerp(target_translation, weight)
    } else {
        translation
    };
    Mat4::from_scale_rotation_translation(scale, rotation, translation)
}

/// Rotate `root` so `mid` lies on the bend the pole selects, then `mid` so
/// `end` reaches the target, each by the constraint's weight.
fn solve_two_bone(
    model: &GlbModel,
    locals: &mut [Trs],
    [root, mid, end]: [usize; 3],
    ik: &TwoBoneIkConstraint,
    to_model: Mat4,
) {
    if ik.weight <= 0.0 {
        return;
    }
    let posed = compose(model, locals);
    let origin = |node: usize, posed: &[Mat4]| posed[node].w_axis.truncate();
    let (a, b, c) = (
        origin(root, &posed),
        origin(mid, &posed),
        origin(end, &posed),
    );
    let (target, pole) = match ik.space {
        PoseSpace::World => (
            to_model.transform_point3(convert::vec3(ik.target)),
            to_model.transform_point3(convert::vec3(ik.pole)),
        ),
        _ => (convert::vec3(ik.target), convert::vec3(ik.pole)),
    };
    let upper = (b - a).length();
    let lower = (c - b).length();
    let reach = (target - a).length();
    if upper < DEGENERATE || lower < DEGENERATE || reach < DEGENERATE {
        return;
    }
    let direction = (target - a) / reach;
    let span = upper + lower;
    let distance = reach.clamp(
        (upper - lower).abs() + span * REACH_MARGIN,
        span * (1.0 - REACH_MARGIN),
    );
    let off_axis = |point: Vec3| {
        let offset = point - a;
        offset - direction * offset.dot(direction)
    };
    let bend = [off_axis(pole), off_axis(b)]
        .into_iter()
        .find(|bend| bend.length() > DEGENERATE)
        .map_or_else(|| direction.any_orthonormal_vector(), Vec3::normalize);
    let cos = ((upper * upper + distance * distance - lower * lower) / (2.0 * upper * distance))
        .clamp(-1.0, 1.0);
    let sin = (1.0 - cos * cos).max(0.0).sqrt();
    let mid_goal = a + (direction * cos + bend * sin) * upper;
    let end_goal = a + direction * distance;

    let turn = Quat::from_rotation_arc((b - a) / upper, (mid_goal - a) / upper);
    rotate_about(
        model,
        locals,
        &posed,
        root,
        Quat::IDENTITY.slerp(turn, ik.weight),
    );
    let posed = compose(model, locals);
    let (b, c) = (origin(mid, &posed), origin(end, &posed));
    let (from, to) = (c - b, end_goal - b);
    if from.length() < DEGENERATE || to.length() < DEGENERATE {
        return;
    }
    let turn = Quat::from_rotation_arc(from.normalize(), to.normalize());
    rotate_about(
        model,
        locals,
        &posed,
        mid,
        Quat::IDENTITY.slerp(turn, ik.weight),
    );
}

/// Turn `node` by the instance-space rotation `turn` about its own origin,
/// rewriting its local transform; its descendants follow.
fn rotate_about(model: &GlbModel, locals: &mut [Trs], posed: &[Mat4], node: usize, turn: Quat) {
    let placed = posed[node];
    let pivot = placed.w_axis.truncate();
    let turned = Mat4::from_translation(pivot)
        * Mat4::from_quat(turn)
        * Mat4::from_translation(-pivot)
        * placed;
    let local = match model.nodes[node].parent {
        Some(parent) => posed[parent].inverse() * turned,
        None => turned,
    };
    let (scale, rotation, translation) = local.to_scale_rotation_translation();
    locals[node] = Trs {
        translation,
        rotation: rotation.normalize(),
        scale,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::GlbNode;

    /// A three-joint arm along +X, one unit per bone, with a hand at x = 2.
    fn arm() -> GlbModel {
        let node = |name: &str, parent: Option<usize>, x: f32| GlbNode {
            name: Some(name.to_owned()),
            parent,
            rest: Trs {
                translation: Vec3::new(x, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
            mesh: None,
            skin: None,
        };
        GlbModel {
            nodes: vec![
                node("shoulder", None, 0.0),
                node("elbow", Some(0), 1.0),
                node("hand", Some(1), 1.0),
            ],
            order: vec![0, 1, 2],
            meshes: Vec::new(),
            skins: Vec::new(),
            clips: Vec::new(),
            materials: Vec::new(),
            textures: Vec::new(),
        }
    }

    fn rest(model: &GlbModel) -> Vec<Trs> {
        model.nodes.iter().map(|node| node.rest).collect()
    }

    fn ik(target: Vec3, pole: Vec3, weight: f32) -> AnimatedMeshPose {
        AnimatedMeshPose {
            two_bone_ik: vec![TwoBoneIkConstraint {
                root: 0,
                mid: 1,
                end: 2,
                space: PoseSpace::Model,
                target: target.to_array(),
                pole: pole.to_array(),
                weight,
            }],
            overrides: Vec::new(),
            report_joints: false,
        }
    }

    const RIG: [Option<usize>; 3] = [Some(0), Some(1), Some(2)];

    fn position(posed: &[Mat4], node: usize) -> Vec3 {
        posed[node].w_axis.truncate()
    }

    #[test]
    fn the_hand_reaches_a_moving_target_and_the_elbow_stays_on_the_pole_side() {
        let model = arm();
        let pole = Vec3::new(0.5, 0.0, -3.0);
        let mut previous: Option<Vec3> = None;
        for step in 0..=60 {
            let angle = step as f32 / 60.0 * std::f32::consts::TAU;
            let target = Vec3::new(1.2 + 0.4 * angle.cos(), 0.6 * angle.sin(), 0.3);
            let posed = controlled(
                &model,
                &RIG,
                rest(&model),
                &ik(target, pole, 1.0),
                Mat4::IDENTITY,
            );
            assert!(
                position(&posed, 2).distance(target) < 1e-3,
                "step {step}: hand {:?} misses {target:?}",
                position(&posed, 2)
            );
            let elbow = position(&posed, 1);
            assert!(
                (elbow.length() - 1.0).abs() < 1e-4,
                "the upper arm keeps its length"
            );
            let axis = target.normalize();
            let bend = (elbow - axis * elbow.dot(axis)).normalize();
            let side = (pole - axis * pole.dot(axis)).normalize();
            assert!(
                bend.dot(side) > 0.99,
                "step {step}: the elbow bends toward the pole"
            );
            if let Some(previous) = previous {
                assert!(elbow.distance(previous) < 0.25, "step {step}: no flip");
            }
            previous = Some(elbow);
        }
    }

    #[test]
    fn an_unreachable_target_straightens_the_arm_toward_it_and_weight_zero_changes_nothing() {
        let model = arm();
        let target = Vec3::new(0.0, 5.0, 0.0);
        let posed = controlled(
            &model,
            &RIG,
            rest(&model),
            &ik(target, Vec3::Z, 1.0),
            Mat4::IDENTITY,
        );
        let hand = position(&posed, 2);
        assert!(hand.normalize().dot(Vec3::Y) > 0.999 && hand.length() > 1.99);
        let untouched = controlled(
            &model,
            &RIG,
            rest(&model),
            &ik(target, Vec3::Z, 0.0),
            Mat4::IDENTITY,
        );
        assert_eq!(untouched, compose(&model, &rest(&model)));
    }

    #[test]
    fn a_world_target_is_solved_in_the_instance_space_it_lands_in() {
        let model = arm();
        let world = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            Quat::from_rotation_y(0.7),
            Vec3::new(10.0, 0.0, -4.0),
        );
        let target = world.transform_point3(Vec3::new(1.0, 1.0, 0.0));
        let mut pose = ik(target, world.transform_point3(Vec3::Z), 1.0);
        pose.two_bone_ik[0].space = PoseSpace::World;
        let posed = controlled(&model, &RIG, rest(&model), &pose, world);
        assert!(world.transform_point3(position(&posed, 2)).distance(target) < 1e-3);
    }

    #[test]
    fn overrides_blend_locally_or_place_a_joint_whose_children_follow() {
        let model = arm();
        let curl = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let half = AnimatedMeshPose {
            overrides: vec![JointOverride {
                joint: 1,
                space: PoseSpace::Local,
                additive: true,
                rotation: Some(curl.to_array()),
                translation: None,
                weight: 0.5,
            }],
            ..AnimatedMeshPose::default()
        };
        let posed = controlled(&model, &RIG, rest(&model), &half, Mat4::IDENTITY);
        let forearm = position(&posed, 2) - position(&posed, 1);
        let quarter = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4) * Vec3::X;
        assert!(forearm.distance(quarter) < 1e-5, "half of a 90 degree curl");

        let world = Mat4::from_translation(Vec3::new(0.0, 3.0, 0.0));
        let placed = AnimatedMeshPose {
            overrides: vec![JointOverride {
                joint: 1,
                space: PoseSpace::World,
                additive: false,
                rotation: Some(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array()),
                translation: Some([5.0, 3.0, 0.0]),
                weight: 1.0,
            }],
            ..AnimatedMeshPose::default()
        };
        let posed = controlled(&model, &RIG, rest(&model), &placed, world);
        assert!(position(&posed, 1).distance(Vec3::new(5.0, 0.0, 0.0)) < 1e-5);
        assert!(
            position(&posed, 2).distance(Vec3::new(5.0, 1.0, 0.0)) < 1e-5,
            "the hand follows the placed elbow"
        );
    }

    fn shift(joint: u32, space: PoseSpace, additive: bool, x: f32, weight: f32) -> JointOverride {
        JointOverride {
            joint,
            space,
            additive,
            rotation: None,
            translation: Some([x, 0.0, 0.0]),
            weight,
        }
    }

    #[test]
    fn one_joints_overrides_apply_in_list_order_across_spaces() {
        let model = arm();
        let placed_then_added = AnimatedMeshPose {
            overrides: vec![
                shift(0, PoseSpace::Model, false, 5.0, 1.0),
                shift(0, PoseSpace::Local, true, 1.0, 1.0),
            ],
            ..AnimatedMeshPose::default()
        };
        let posed = controlled(
            &model,
            &RIG,
            rest(&model),
            &placed_then_added,
            Mat4::IDENTITY,
        );
        assert!(position(&posed, 0).distance(Vec3::new(6.0, 0.0, 0.0)) < 1e-5);

        let mut added_then_placed = placed_then_added.clone();
        added_then_placed.overrides.reverse();
        let posed = controlled(
            &model,
            &RIG,
            rest(&model),
            &added_then_placed,
            Mat4::IDENTITY,
        );
        assert!(position(&posed, 0).distance(Vec3::new(5.0, 0.0, 0.0)) < 1e-5);

        // An orientation, then a turn after it, keeps the turn.
        let quarter = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let oriented_then_turned = AnimatedMeshPose {
            overrides: vec![
                JointOverride {
                    joint: 1,
                    space: PoseSpace::Model,
                    additive: false,
                    rotation: Some(quarter.to_array()),
                    translation: None,
                    weight: 1.0,
                },
                JointOverride {
                    joint: 1,
                    space: PoseSpace::Local,
                    additive: true,
                    rotation: Some(quarter.to_array()),
                    translation: None,
                    weight: 1.0,
                },
            ],
            ..AnimatedMeshPose::default()
        };
        let posed = controlled(
            &model,
            &RIG,
            rest(&model),
            &oriented_then_turned,
            Mat4::IDENTITY,
        );
        let forearm = position(&posed, 2) - position(&posed, 1);
        assert!(forearm.distance(-Vec3::X) < 1e-5, "{forearm:?}");
    }

    #[test]
    fn placed_joints_blend_as_one_pose_without_stretching_their_bones() {
        let model = arm();
        let moved = |weight: f32| AnimatedMeshPose {
            overrides: (0..3)
                .map(|joint| JointOverride {
                    joint,
                    space: PoseSpace::World,
                    additive: false,
                    rotation: Some([0.0, 0.0, 0.0, 1.0]),
                    translation: Some([10.0 + joint as f32, 0.0, 0.0]),
                    weight,
                })
                .collect(),
            ..AnimatedMeshPose::default()
        };
        for (weight, start) in [(0.0, 0.0), (0.5, 5.0), (1.0, 10.0)] {
            let posed = controlled(&model, &RIG, rest(&model), &moved(weight), Mat4::IDENTITY);
            for joint in 0..3 {
                let expected = Vec3::new(start + joint as f32, 0.0, 0.0);
                assert!(
                    position(&posed, joint).distance(expected) < 1e-4,
                    "weight {weight}: joint {joint} at {:?}",
                    position(&posed, joint)
                );
            }
        }
    }
}
