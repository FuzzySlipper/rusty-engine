//! What inspection shows, as Engine live-debug commands
//! (`engine.inspect.overlay`): the product's overlay sets, the Debug layer,
//! and the renderer's own inspection drawing. The renderer holds the state;
//! these commands read and set it. `engine.inspect.landmarks` reads the
//! sets' landmark pairs as the renderer measures them.
//!
//! Collision wires are the Spatial sessions' retained collision, which the
//! services read ([`EngineServiceSet::inspect_collision`]) after each
//! product call and inspection command while they show, and give the
//! renderer as lines. `engine.inspect.collision` lists what they draw.
use super::*;

pub(super) const OVERLAY_COMMAND: &str = "engine.inspect.overlay";
pub(super) const LANDMARKS_COMMAND: &str = "engine.inspect.landmarks";
pub(super) const COLLISION_COMMAND: &str = "engine.inspect.collision";

const OVERLAY_USAGE: &str = "usage: engine.inspect.overlay [list] | show|hide [sets [in VIEW] | set NAME [in VIEW] | debug-layer | camera | motion | joints HANDLE | bounds HANDLE | ik HANDLE | collision [RADIUS] | collision at X Y Z [RADIUS]]";
const COLLISION_USAGE: &str = "usage: engine.inspect.collision [RADIUS | X Y Z RADIUS]";
/// Metres along each axis collision wires and readouts reach by default.
const COLLISION_RADIUS: f64 = 12.0;
/// Collision wire colours by kind; a face steeper than the character's
/// slope limit draws at `STEEP_SHADE` of its kind's.
const VOXEL_COLOR: [f32; 3] = [1.0, 0.55, 0.1];
const STATIC_MESH_COLOR: [f32; 3] = [0.2, 0.85, 1.0];
const OBSTACLE_COLOR: [f32; 3] = [1.0, 0.3, 0.9];
const BODY_COLOR: [f32; 3] = [1.0, 0.9, 0.2];
const CHARACTER_COLOR: [f32; 3] = [0.3, 1.0, 0.4];
const TRIGGER_COLOR: [f32; 3] = [0.6, 0.5, 1.0];
const STEEP_SHADE: f32 = 0.4;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayAnswer {
    /// Inspection draws something.
    on: bool,
    all_sets: bool,
    debug_layer: bool,
    camera: bool,
    /// The primary frame's motion target in place of its picture.
    motion: bool,
    /// Sets shown only in one view, by set.
    only_in: std::collections::BTreeMap<String, String>,
    /// Views that show every set.
    sets_in: Vec<String>,
    /// Sets hidden by name from every set.
    hidden: Vec<String>,
    joints: Vec<u64>,
    bounds: Vec<u64>,
    ik: Vec<u64>,
    /// Where collision wires are read while they show: around `at`, or the
    /// observer or primary camera without it.
    #[serde(skip_serializing_if = "Option::is_none")]
    collision: Option<CollisionShownAnswer>,
    sets: Vec<OverlaySetAnswer>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CollisionShownAnswer {
    #[serde(skip_serializing_if = "Option::is_none")]
    at: Option<[f64; 3]>,
    radius: f64,
}

/// What a collision read found: every collider meeting the region, nearest
/// first after the characters, obstacles, triggers and bodies.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CollisionAnswer {
    center: [f64; 3],
    radius: f64,
    lines: usize,
    /// Parts left out because the lines ran out.
    omitted: usize,
    colliders: Vec<ColliderAnswer>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ColliderAnswer {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<u64>,
    /// A spatial content artifact's path.
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    /// Whether a content artifact is the session's base, not a placement.
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    asset: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    world: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chunk: Option<[i64; 3]>,
    min: [f64; 3],
    max: [f64; 3],
    lines: usize,
    omitted: usize,
}

fn collider_answer(collider: csharp_engine_services::InspectedCollision) -> ColliderAnswer {
    use csharp_engine_services::CollisionSource;
    let mut answer = ColliderAnswer {
        kind: "",
        session: collider.session,
        content: None,
        base: None,
        instance: None,
        asset: None,
        entity: None,
        world: None,
        body: None,
        chunk: None,
        min: collider.min,
        max: collider.max,
        lines: collider.lines,
        omitted: collider.omitted,
    };
    answer.kind = match collider.source {
        CollisionSource::VoxelChunk(chunk) => {
            answer.chunk = Some(chunk);
            "voxel"
        }
        CollisionSource::ContentArtifact {
            path,
            instance,
            base,
        } => {
            answer.content = Some(path);
            answer.instance = Some(instance);
            answer.base = Some(base);
            "static-mesh"
        }
        CollisionSource::StaticMesh { instance, asset } => {
            answer.instance = Some(instance);
            answer.asset = Some(asset);
            "static-mesh"
        }
        CollisionSource::Obstacle { entity } => {
            answer.entity = Some(entity);
            "obstacle"
        }
        CollisionSource::Trigger { entity } => {
            answer.entity = Some(entity);
            "trigger"
        }
        CollisionSource::Body { world, body } => {
            answer.world = Some(world);
            answer.body = Some(body);
            "body"
        }
        CollisionSource::Character => "character",
    };
    answer
}

/// A collision line as inspection draws it.
fn overlay_line(line: &csharp_engine_services::CollisionLine) -> render_wgpu::OverlayLine {
    use csharp_engine_services::CollisionLineKind;
    let [r, g, b] = match line.kind {
        CollisionLineKind::Voxel => VOXEL_COLOR,
        CollisionLineKind::StaticMesh => STATIC_MESH_COLOR,
        CollisionLineKind::Obstacle => OBSTACLE_COLOR,
        CollisionLineKind::Body => BODY_COLOR,
        CollisionLineKind::Character => CHARACTER_COLOR,
        CollisionLineKind::Trigger => TRIGGER_COLOR,
    }
    .map(|channel| {
        if line.steep {
            channel * STEEP_SHADE
        } else {
            channel
        }
    });
    render_wgpu::OverlayLine {
        from: line.from,
        to: line.to,
        color: [r, g, b, 1.0],
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LandmarkSetAnswer {
    name: String,
    revision: u64,
    shown: bool,
    pairs: Vec<LandmarkPairAnswer>,
}

/// One pair as drawn. `drawn` is false while either landmark is on nothing
/// drawn, and then only the judging is answered.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LandmarkPairAnswer {
    name: String,
    /// Metres; 0 leaves the distance unjudged.
    tolerance: f32,
    aim: &'static str,
    angle_tolerance: f32,
    drawn: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    within: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    distance: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    angle: Option<f32>,
    /// Where `from` is from `to` along the `to` landmark's frame: x right,
    /// y up, z back, metres.
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<[f32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    to: Option<[f32; 3]>,
}

fn landmark_set_answer(set: render_wgpu::LandmarkSetReadout) -> LandmarkSetAnswer {
    LandmarkSetAnswer {
        name: set.name,
        revision: set.revision,
        shown: set.shown,
        pairs: set
            .pairs
            .into_iter()
            .map(|pair| LandmarkPairAnswer {
                name: pair.name,
                tolerance: pair.tolerance,
                aim: match pair.aim {
                    render_model::LandmarkAim::None => "none",
                    render_model::LandmarkAim::Axis => "axis",
                    render_model::LandmarkAim::Point => "point",
                    render_model::LandmarkAim::Line => "line",
                },
                angle_tolerance: pair.angle_tolerance,
                drawn: pair.drawn.is_some(),
                within: pair.drawn.map(|drawn| drawn.within),
                distance: pair.drawn.map(|drawn| drawn.distance),
                angle: pair.drawn.and_then(|drawn| drawn.angle),
                offset: pair.drawn.map(|drawn| drawn.offset),
                from: pair.drawn.map(|drawn| drawn.from),
                to: pair.drawn.map(|drawn| drawn.to),
            })
            .collect(),
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlaySetAnswer {
    name: String,
    items: usize,
    /// Landmark pairs (`engine.inspect.landmarks`).
    pairs: usize,
    revision: u64,
    shown: bool,
}

/// `shown` with one `show` or `hide` applied.
fn change(
    mut shown: render_wgpu::OverlayShown,
    show: bool,
    words: &[&str],
) -> Result<render_wgpu::OverlayShown, String> {
    let handle = |word: &str| word.parse::<u64>().map_err(|_| OVERLAY_USAGE.to_owned());
    let metres = |word: &str| {
        word.parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| OVERLAY_USAGE.to_owned())
    };
    let positive = |word: &str| {
        metres(word).and_then(|value| {
            (value > 0.0)
                .then_some(value)
                .ok_or_else(|| OVERLAY_USAGE.to_owned())
        })
    };
    let toggle = |set: &mut std::collections::BTreeSet<u64>, value: u64| {
        if show {
            set.insert(value);
        } else {
            set.remove(&value);
        }
    };
    match words {
        [] if show => {
            shown.all_sets = true;
            shown.debug_layer = true;
            shown.hidden.clear();
        }
        [] => shown = render_wgpu::OverlayShown::default(),
        ["sets"] => {
            shown.all_sets = show;
            shown.sets.clear();
            shown.hidden.clear();
        }
        ["sets", "in", view] => {
            if show {
                shown.sets_in.insert((*view).to_owned());
            } else {
                shown.sets_in.remove(*view);
            }
        }
        ["set", name] => {
            if show {
                shown.sets.insert((*name).to_owned());
                shown.hidden.remove(*name);
            } else {
                shown.sets.remove(*name);
                shown.hidden.insert((*name).to_owned());
            }
            shown.only_in.remove(*name);
        }
        ["set", name, "in", view] => {
            if show {
                shown.sets.insert((*name).to_owned());
                shown.hidden.remove(*name);
                shown.only_in.insert((*name).to_owned(), (*view).to_owned());
            } else {
                shown.sets.remove(*name);
                shown.hidden.insert((*name).to_owned());
                shown.only_in.remove(*name);
            }
        }
        ["debug-layer"] => shown.debug_layer = show,
        ["camera"] => shown.camera = show,
        ["motion"] => shown.motion = show,
        ["joints", value] => toggle(&mut shown.joints, handle(value)?),
        ["bounds", value] => toggle(&mut shown.bounds, handle(value)?),
        ["ik", value] => toggle(&mut shown.ik, handle(value)?),
        ["collision", ..] if !show => shown.collision = None,
        ["collision"] => {
            shown.collision = Some(render_wgpu::CollisionShown {
                at: None,
                radius: COLLISION_RADIUS,
            })
        }
        ["collision", radius] => {
            shown.collision = Some(render_wgpu::CollisionShown {
                at: None,
                radius: positive(radius)?,
            })
        }
        ["collision", "at", x, y, z, radius @ ..] if radius.len() <= 1 => {
            shown.collision = Some(render_wgpu::CollisionShown {
                at: Some([metres(x)?, metres(y)?, metres(z)?]),
                radius: radius
                    .first()
                    .map_or(Ok(COLLISION_RADIUS), |radius| positive(radius))?,
            })
        }
        _ => return Err(OVERLAY_USAGE.to_owned()),
    }
    Ok(shown)
}

impl CsharpProductRuntime {
    /// Where collision is read around while it shows: its own point, else
    /// the observer, else the primary camera; none without any.
    fn collision_center(
        driver: &render_wgpu::SceneDriver,
        shown: render_wgpu::CollisionShown,
    ) -> Option<[f64; 3]> {
        shown
            .at
            .or_else(|| driver.view_state().observer.map(|pose| pose.position))
            .or_else(|| driver.primary_camera().map(|camera| camera.pose.position))
    }

    /// Reads the collision wires again while they show and gives the
    /// renderer their lines. Nothing is read while they are hidden.
    pub(super) fn refresh_collision_wires(&self) {
        let Some(frames) = self.frame_output.as_ref() else {
            return;
        };
        let driver = frames.driver();
        let Some(shown) = driver.collision_shown() else {
            return;
        };
        let lines: Vec<render_wgpu::OverlayLine> = Self::collision_center(&driver, shown)
            .map(|center| {
                self.services
                    .inspect_collision(
                        center,
                        shown.radius,
                        csharp_engine_services::COLLISION_INSPECTION_LINES,
                    )
                    .lines
                    .iter()
                    .map(overlay_line)
                    .collect()
            })
            .unwrap_or_default();
        driver.set_collision_lines(&lines);
    }

    /// Answers `engine.inspect.collision`: around the collision wires'
    /// place, or the camera's with the default reach, or the given ones.
    fn execute_collision_debug(
        &self,
        driver: &render_wgpu::SceneDriver,
        rest: &[&str],
    ) -> ProductHostDebugResult {
        let number = |word: &&str| word.parse::<f64>().ok().filter(|value| value.is_finite());
        let shown = driver
            .collision_shown()
            .unwrap_or(render_wgpu::CollisionShown {
                at: None,
                radius: COLLISION_RADIUS,
            });
        let asked = match rest {
            [] => Some(shown),
            [radius] => {
                number(radius).map(|radius| render_wgpu::CollisionShown { at: None, radius })
            }
            [x, y, z, radius] => match (number(x), number(y), number(z), number(radius)) {
                (Some(x), Some(y), Some(z), Some(radius)) => Some(render_wgpu::CollisionShown {
                    at: Some([x, y, z]),
                    radius,
                }),
                _ => None,
            },
            _ => None,
        };
        let Some(asked) = asked.filter(|asked| asked.radius > 0.0) else {
            return ProductHostDebugResult::new(false, COLLISION_USAGE.to_owned());
        };
        let Some(center) = Self::collision_center(driver, asked) else {
            return ProductHostDebugResult::new(
                false,
                "INSPECT_NO_CAMERA: no observer or primary camera to read collision around; give X Y Z".to_owned(),
            );
        };
        let inspection = self.services.inspect_collision(
            center,
            asked.radius,
            csharp_engine_services::COLLISION_INSPECTION_LINES,
        );
        let answer = CollisionAnswer {
            center,
            radius: asked.radius,
            lines: inspection.lines.len(),
            omitted: inspection.omitted(),
            colliders: inspection
                .colliders
                .into_iter()
                .map(collider_answer)
                .collect(),
        };
        match serde_json::to_string(&answer) {
            Ok(message) => ProductHostDebugResult::new(true, message),
            Err(error) => ProductHostDebugResult::new(false, error.to_string()),
        }
    }

    /// Answers `engine.inspect.overlay`, `engine.inspect.landmarks` and
    /// `engine.inspect.collision`; none for any other command.
    pub(super) fn execute_inspect_debug(&self, command: &str) -> Option<ProductHostDebugResult> {
        let words: Vec<&str> = command.split_whitespace().collect();
        let (first, rest) = words.split_first()?;
        if ![OVERLAY_COMMAND, LANDMARKS_COMMAND, COLLISION_COMMAND].contains(first) {
            return None;
        }
        let Some(frames) = self.frame_output.as_ref() else {
            return Some(ProductHostDebugResult::new(
                false,
                "INSPECT_NO_RENDERER: this runtime draws no frames".to_owned(),
            ));
        };
        let driver = frames.driver();
        if *first == COLLISION_COMMAND {
            return Some(self.execute_collision_debug(&driver, rest));
        }
        if *first == LANDMARKS_COMMAND {
            let (step, sets) = driver.landmark_sets();
            let sets: Vec<LandmarkSetAnswer> = match rest {
                [] => sets.into_iter().map(landmark_set_answer).collect(),
                [name] => match sets.into_iter().find(|set| set.name == *name) {
                    Some(set) => vec![landmark_set_answer(set)],
                    None => {
                        return Some(ProductHostDebugResult::new(
                            false,
                            format!(
                                "INSPECT_NO_LANDMARKS: no overlay set {name} has landmark pairs"
                            ),
                        ))
                    }
                },
                _ => {
                    return Some(ProductHostDebugResult::new(
                        false,
                        "usage: engine.inspect.landmarks [SET]".to_owned(),
                    ))
                }
            };
            return Some(
                match serde_json::to_string(&serde_json::json!({ "step": step, "sets": sets })) {
                    Ok(message) => ProductHostDebugResult::new(true, message),
                    Err(error) => ProductHostDebugResult::new(false, error.to_string()),
                },
            );
        }
        let changed = match rest {
            [] | ["list"] => Ok(()),
            ["show", words @ ..] | ["hide", words @ ..] => {
                change(driver.overlays().0, rest[0] == "show", words)
                    .map(|shown| driver.set_overlay_shown(shown))
            }
            _ => Err(OVERLAY_USAGE.to_owned()),
        };
        if let Err(detail) = changed {
            return Some(ProductHostDebugResult::new(false, detail));
        }
        if driver.collision_shown().is_none() {
            driver.set_collision_lines(&[]);
        }
        self.refresh_collision_wires();
        let (shown, sets) = driver.overlays();
        let answer = OverlayAnswer {
            on: shown.any(),
            all_sets: shown.all_sets,
            debug_layer: shown.debug_layer,
            camera: shown.camera,
            motion: shown.motion,
            only_in: shown.only_in.clone(),
            sets_in: shown.sets_in.iter().cloned().collect(),
            hidden: shown.hidden.iter().cloned().collect(),
            joints: shown.joints.iter().copied().collect(),
            bounds: shown.bounds.iter().copied().collect(),
            ik: shown.ik.iter().copied().collect(),
            collision: shown.collision.map(|collision| CollisionShownAnswer {
                at: collision.at,
                radius: collision.radius,
            }),
            sets: sets
                .into_iter()
                .map(|set| OverlaySetAnswer {
                    name: set.name,
                    items: set.items,
                    pairs: set.pairs,
                    revision: set.revision,
                    shown: set.shown,
                })
                .collect(),
        };
        Some(match serde_json::to_string(&answer) {
            Ok(message) => ProductHostDebugResult::new(true, message),
            Err(error) => ProductHostDebugResult::new(false, error.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_and_hide_change_one_thing_and_bare_forms_cover_everything() {
        let none = render_wgpu::OverlayShown::default();
        let all = change(none.clone(), true, &[]).unwrap();
        assert!(all.all_sets && all.debug_layer && !all.camera);
        let joints = change(all.clone(), true, &["joints", "7"]).unwrap();
        assert_eq!(joints.joints.iter().copied().collect::<Vec<_>>(), [7]);
        let in_view = change(none.clone(), true, &["sets", "in", "grip"]).unwrap();
        assert!(in_view.sets_in.contains("grip") && in_view.any() && !in_view.all_sets);
        assert!(!change(in_view, false, &["sets", "in", "grip"])
            .unwrap()
            .any());
        let only = change(none.clone(), true, &["set", "guide", "in", "side"]).unwrap();
        assert_eq!(only.only_in.get("guide").map(String::as_str), Some("side"));
        assert!(change(only, true, &["set", "guide"])
            .unwrap()
            .only_in
            .is_empty());
        let named = change(none.clone(), true, &["set", "guide"]).unwrap();
        assert!(named.sets.contains("guide") && named.any());
        assert!(!change(named, false, &["set", "guide"]).unwrap().any());
        assert!(!change(joints, false, &[]).unwrap().any());
        // A set hidden by name stays out of every set, and comes back by
        // name or with every set.
        let hidden = change(all.clone(), false, &["set", "guide"]).unwrap();
        assert!(hidden.all_sets && hidden.hidden.contains("guide"));
        let back = change(hidden.clone(), true, &["set", "guide"]).unwrap();
        assert!(back.hidden.is_empty() && back.sets.contains("guide"));
        assert!(change(hidden.clone(), true, &["sets"])
            .unwrap()
            .hidden
            .is_empty());
        assert!(change(hidden, true, &[]).unwrap().hidden.is_empty());
        // The motion target shows on its own, and a bare show leaves it.
        let motion = change(none.clone(), true, &["motion"]).unwrap();
        assert!(motion.motion && motion.any());
        assert!(!change(motion, false, &[]).unwrap().motion);
        assert!(!all.motion);
        // Collision wires around the camera, or a point, and gone with a
        // bare hide.
        let collision = change(none.clone(), true, &["collision"]).unwrap();
        assert_eq!(
            collision.collision,
            Some(render_wgpu::CollisionShown {
                at: None,
                radius: COLLISION_RADIUS
            })
        );
        assert!(collision.any() && all.collision.is_none());
        let at = change(
            collision.clone(),
            true,
            &["collision", "at", "1", "2", "3", "4"],
        )
        .unwrap();
        assert_eq!(
            at.collision,
            Some(render_wgpu::CollisionShown {
                at: Some([1.0, 2.0, 3.0]),
                radius: 4.0
            })
        );
        assert!(change(at.clone(), false, &["collision"])
            .unwrap()
            .collision
            .is_none());
        assert!(!change(at, false, &[]).unwrap().any());
        assert!(change(none.clone(), true, &["collision", "0"]).is_err());
        assert!(change(none.clone(), true, &["collision", "at", "1", "2"]).is_err());
        assert!(change(none.clone(), true, &["joints", "x"]).is_err());
        assert!(change(none, true, &["floor"]).is_err());
    }
}
