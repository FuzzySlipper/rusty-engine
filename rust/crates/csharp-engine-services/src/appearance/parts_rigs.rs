//! Parts rigs the Engine poses (`animation-parts-rig`) and realizers draw.
//!
//! A rig read from its model document is kept by its drawing's asset id;
//! [`PartsRigScene::define`] gives the `DefinePartsRig` its poses draw with.
//! Each `CreatePartsRig` the call pushes gets a player on that rig. As each
//! call settles, every player advances to the Engine's presentation time
//! and its pose is published as `SetPartsRigPose`, once per change: a pose
//! equal to the one last published sends nothing.

use std::collections::BTreeMap;
use std::sync::Arc;

use ::animation_parts_rig::{BlendMode, Play, Player, Pose, Rig};
use render_model::{
    PartsRigAsset, PartsRigBlend, PartsRigDrawable, PartsRigPage, PartsRigPiece, PartsRigPose,
    RenderDiff, RenderFrameDiff, RenderHandle,
};

/// The parts rigs the Engine poses, and their players.
#[derive(Clone, Default)]
pub(crate) struct PartsRigScene {
    rigs: BTreeMap<String, Arc<Rig>>,
    instances: BTreeMap<RenderHandle, RigInstance>,
}

#[derive(Clone)]
struct RigInstance {
    asset: String,
    /// None until its rig is defined.
    player: Option<Player>,
    pose: Pose,
    /// The presentation time it was last posed at; None before its first
    /// pose.
    posed_at: Option<f64>,
    /// Played, stopped or reskinned since it was last posed.
    dirty: bool,
    published: Option<PartsRigPose>,
}

#[allow(
    dead_code,
    reason = "defining rigs, playing, stopping, skins and pose reads are reached from tests until a named service calls them"
)]
impl PartsRigScene {
    /// Keep `rig` as asset `id`'s, drawing from `pages` (one per rig page,
    /// in its order: its texture and any normal or height map), and give the
    /// definition its realizers draw with. Its instances start again from
    /// the setup pose.
    pub(crate) fn define(
        &mut self,
        id: String,
        rig: Arc<Rig>,
        pages: Vec<(String, Option<String>)>,
    ) -> RenderDiff {
        let asset = PartsRigAsset {
            id: id.clone(),
            pages: rig
                .pages()
                .iter()
                .zip(pages)
                .map(|(page, (texture, detail))| PartsRigPage {
                    texture,
                    premultiplied: page.pma,
                    detail,
                })
                .collect(),
            drawables: rig
                .drawables()
                .iter()
                .map(|drawable| PartsRigDrawable {
                    page: drawable.page as u32,
                    uvs: drawable.uvs.clone(),
                    triangles: drawable.triangles.clone(),
                })
                .collect(),
        };
        for instance in self.instances.values_mut() {
            if instance.asset == id {
                instance.player = Some(Player::new(&rig));
                instance.posed_at = None;
                instance.dirty = true;
            }
        }
        self.rigs.insert(id, rig);
        RenderDiff::DefinePartsRig { asset }
    }

    /// Forget asset `id`'s rig; its instances hold their last pose.
    pub(crate) fn release(&mut self, id: &str) -> RenderDiff {
        self.rigs.remove(id);
        for instance in self.instances.values_mut() {
            if instance.asset == id {
                instance.player = None;
            }
        }
        RenderDiff::ReleasePartsRig {
            asset: id.to_owned(),
        }
    }

    /// Take the rig nodes a pushed frame creates, and drop the players of
    /// nodes it replaces or destroys.
    pub(crate) fn observe_frame(&mut self, frame: &RenderFrameDiff) {
        for op in &frame.ops {
            match op {
                RenderDiff::CreatePartsRig { handle, rig, .. } => {
                    self.instances.insert(
                        *handle,
                        RigInstance {
                            asset: rig.asset.clone(),
                            player: self.rigs.get(&rig.asset).map(|rig| Player::new(rig)),
                            pose: Pose::default(),
                            posed_at: None,
                            dirty: true,
                            published: None,
                        },
                    );
                }
                RenderDiff::Create { handle, .. }
                | RenderDiff::CreateStaticMeshInstance { handle, .. }
                | RenderDiff::CreateAnimatedMeshInstance { handle, .. }
                | RenderDiff::CreateVoxelObjectInstance { handle, .. }
                | RenderDiff::CreateSprite { handle, .. }
                | RenderDiff::CreateSpriteBatch { handle, .. }
                | RenderDiff::CreateScatterPatch { handle, .. }
                | RenderDiff::CreateLight { handle, .. }
                | RenderDiff::Destroy { handle } => {
                    self.instances.remove(handle);
                }
                _ => {}
            }
        }
    }

    /// A rig node's player, marked to pose again at the next settle.
    fn player(&mut self, handle: RenderHandle) -> Option<(&mut Player, &Rig)> {
        let instance = self.instances.get_mut(&handle)?;
        let rig = self.rigs.get(&instance.asset)?;
        let player = instance.player.as_mut()?;
        instance.dirty = true;
        Some((player, rig))
    }

    /// Play clip `animation` on rig node `handle`, from its start, fading in
    /// over what played before. False when the node has no rig, or the rig
    /// no such clip.
    pub(crate) fn play(&mut self, handle: RenderHandle, animation: usize, play: Play) -> bool {
        match self.player(handle) {
            Some((player, rig)) if animation < rig.animation_count() => {
                player.play(animation, play);
                true
            }
            _ => false,
        }
    }

    /// Fade rig node `handle` back to its setup pose.
    pub(crate) fn stop(&mut self, handle: RenderHandle, crossfade: f64) -> bool {
        self.player(handle)
            .map(|(player, _)| player.stop(crossfade))
            .is_some()
    }

    /// Show skin `skin` (`None`: the default skin alone) on rig node
    /// `handle`.
    pub(crate) fn set_skin(&mut self, handle: RenderHandle, skin: Option<usize>) -> bool {
        match self.player(handle) {
            Some((player, rig)) if skin.is_none_or(|skin| skin < rig.skin_count()) => {
                player.set_skin(rig, skin);
                true
            }
            _ => false,
        }
    }

    /// The pose rig node `handle` was last posed in: its bones, pieces and
    /// the events its last step fired.
    pub(crate) fn pose(&self, handle: RenderHandle) -> Option<&Pose> {
        let instance = self.instances.get(&handle)?;
        instance.posed_at.map(|_| &instance.pose)
    }

    /// Advance every player to presentation time `now` and pose it once;
    /// the poses that changed since they were last published, for
    /// realizers. Nodes `world` no longer holds as parts rigs are dropped.
    pub(crate) fn settle(
        &mut self,
        now: f64,
        world: &render_presentation::PresentationWorld,
    ) -> Vec<RenderDiff> {
        self.instances
            .retain(|handle, _| world.parts_rig(*handle).is_some());
        let mut ops = Vec::new();
        for (handle, instance) in &mut self.instances {
            let (Some(player), Some(rig)) = (&mut instance.player, self.rigs.get(&instance.asset))
            else {
                continue;
            };
            let step = instance.posed_at.map_or(0.0, |at| (now - at).max(0.0));
            if step == 0.0 && !instance.dirty {
                continue;
            }
            player.advance(rig, step, &mut instance.pose);
            instance.posed_at = Some(now);
            instance.dirty = false;
            let pieces: Arc<[PartsRigPiece]> = instance
                .pose
                .pieces
                .iter()
                .map(|piece| PartsRigPiece {
                    drawable: piece.drawable as u32,
                    blend: match piece.blend {
                        BlendMode::Normal => PartsRigBlend::Normal,
                        BlendMode::Additive => PartsRigBlend::Additive,
                        BlendMode::Multiply => PartsRigBlend::Multiply,
                        BlendMode::Screen => PartsRigBlend::Screen,
                    },
                    color: piece.color,
                    dark: piece.dark.unwrap_or_default(),
                })
                .collect();
            let unchanged = instance.published.as_ref().is_some_and(|published| {
                *published.pieces == *pieces && *published.vertices == *instance.pose.vertices
            });
            if unchanged {
                continue;
            }
            let pose = PartsRigPose {
                seconds: now,
                pieces,
                vertices: instance.pose.vertices.as_slice().into(),
            };
            instance.published = Some(pose.clone());
            ops.push(RenderDiff::SetPartsRigPose {
                handle: *handle,
                pose,
            });
        }
        ops
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_model::{
        BillboardMode, PartsRigInstanceDescriptor, RenderLayer, RenderMetadata, SpriteDepthPolicy,
        SpriteSizeMode, Transform,
    };
    use render_presentation::PresentationWorld;

    const SQUASH: &str =
        include_str!("../../../animation-parts-rig/tests/fixtures/squash.model.json");
    const RIG: &str = "parts-rig/squash";

    fn create(handle: u64) -> RenderDiff {
        RenderDiff::CreatePartsRig {
            handle: RenderHandle::new(handle),
            parent: None,
            rig: PartsRigInstanceDescriptor {
                asset: RIG.to_owned(),
                scale: 0.002,
                size_mode: SpriteSizeMode::World,
                billboard: BillboardMode::Cylindrical,
                tint: [1.0; 4],
                render_order: 0,
                depth: SpriteDepthPolicy::Default,
                layer: RenderLayer::Scene,
                material: Default::default(),
                texel_snap: 0.0,
                visible: true,
                transform: Transform::default(),
                metadata: RenderMetadata::default(),
            },
        }
    }

    /// One call: push `ops` as the services do, apply them, move the clock
    /// by `seconds` and settle the rigs into the world.
    fn call(
        scene: &mut PartsRigScene,
        world: &mut PresentationWorld,
        ops: Vec<RenderDiff>,
        seconds: f64,
    ) -> Vec<RenderDiff> {
        let frame = RenderFrameDiff::from_ops(ops);
        scene.observe_frame(&frame);
        world.apply(frame).expect("the world admits the call");
        world.advance_elapsed(seconds);
        let poses = scene.settle(world.elapsed_seconds(), world);
        world
            .apply(RenderFrameDiff::from_ops(poses.clone()))
            .expect("the world admits the poses");
        poses
    }

    fn handles(poses: &[RenderDiff]) -> Vec<u64> {
        poses
            .iter()
            .map(|op| match op {
                RenderDiff::SetPartsRigPose { handle, .. } => handle.raw(),
                other => panic!("a settle publishes poses only: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn rigs_pose_once_per_call_and_publish_only_changed_poses() {
        let rig = Arc::new(Rig::from_json(SQUASH).expect("squash reads"));
        let bounce = rig.find_animation("bounce").expect("bounce");
        let (mut scene, mut world) = (PartsRigScene::default(), PresentationWorld::default());
        let define = scene.define(
            RIG.to_owned(),
            rig,
            vec![("texture/squash".to_owned(), None)],
        );
        let RenderDiff::DefinePartsRig { asset } = &define else {
            panic!("a definition");
        };
        assert_eq!(asset.pages.len(), 1);
        assert!(!asset.drawables.is_empty());

        // Created: each rig's setup pose, once.
        let first = call(
            &mut scene,
            &mut world,
            vec![define, create(1), create(2)],
            0.0,
        );
        assert_eq!(handles(&first), [1, 2]);
        assert!(
            call(&mut scene, &mut world, Vec::new(), 0.0).is_empty(),
            "held time"
        );
        assert!(
            call(&mut scene, &mut world, Vec::new(), 0.1).is_empty(),
            "a setup pose that time does not move sends nothing again"
        );

        // Playing rig 1 poses it each call that moves time; rig 2 holds.
        assert!(scene.play(
            RenderHandle::new(1),
            bounce,
            Play {
                looping: true,
                crossfade: 0.0,
            }
        ));
        assert!(!scene.play(
            RenderHandle::new(2),
            99,
            Play {
                looping: true,
                crossfade: 0.0
            }
        ));
        assert_eq!(handles(&call(&mut scene, &mut world, Vec::new(), 0.0)), [1]);
        assert_eq!(
            handles(&call(&mut scene, &mut world, Vec::new(), 1.0 / 12.0)),
            [1]
        );
        let time = scene.pose(RenderHandle::new(1)).unwrap().animation_time;
        assert!((time - 1.0 / 12.0).abs() < 1e-9, "{time}");

        // Stopped, it fades back to the setup pose and then sends nothing.
        assert!(scene.stop(RenderHandle::new(1), 0.2));
        assert_eq!(handles(&call(&mut scene, &mut world, Vec::new(), 0.1)), [1]);
        assert_eq!(
            handles(&call(&mut scene, &mut world, Vec::new(), 0.15)),
            [1]
        );
        assert!(call(&mut scene, &mut world, Vec::new(), 0.1).is_empty());

        // The world keeps the last pose for every reading of it.
        let snapshot = world.snapshot().frame;
        assert_eq!(
            snapshot
                .ops
                .iter()
                .filter(|op| matches!(op, RenderDiff::SetPartsRigPose { .. }))
                .count(),
            2
        );

        // A destroyed rig is forgotten.
        call(
            &mut scene,
            &mut world,
            vec![RenderDiff::Destroy {
                handle: RenderHandle::new(1),
            }],
            0.1,
        );
        assert!(scene.pose(RenderHandle::new(1)).is_none());
        assert!(!scene.stop(RenderHandle::new(1), 0.0));
    }
}
