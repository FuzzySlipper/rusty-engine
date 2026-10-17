//! What inspection shows, as Engine live-debug commands
//! (`engine.inspect.overlay`): the product's overlay sets, the Debug layer,
//! and the renderer's own inspection drawing. The renderer holds the state;
//! these commands read and set it. `engine.inspect.landmarks` reads the
//! sets' landmark pairs as the renderer measures them.
use super::*;

pub(super) const OVERLAY_COMMAND: &str = "engine.inspect.overlay";
pub(super) const LANDMARKS_COMMAND: &str = "engine.inspect.landmarks";

const OVERLAY_USAGE: &str = "usage: engine.inspect.overlay [list] | show|hide [sets [in VIEW] | set NAME [in VIEW] | debug-layer | camera | joints HANDLE | bounds HANDLE | ik HANDLE]";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OverlayAnswer {
    /// Inspection draws something.
    on: bool,
    all_sets: bool,
    debug_layer: bool,
    camera: bool,
    /// Sets shown only in one view, by set.
    only_in: std::collections::BTreeMap<String, String>,
    /// Views that show every set.
    sets_in: Vec<String>,
    /// Sets hidden by name from every set.
    hidden: Vec<String>,
    joints: Vec<u64>,
    bounds: Vec<u64>,
    ik: Vec<u64>,
    sets: Vec<OverlaySetAnswer>,
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
        ["joints", value] => toggle(&mut shown.joints, handle(value)?),
        ["bounds", value] => toggle(&mut shown.bounds, handle(value)?),
        ["ik", value] => toggle(&mut shown.ik, handle(value)?),
        _ => return Err(OVERLAY_USAGE.to_owned()),
    }
    Ok(shown)
}

impl CsharpProductRuntime {
    /// Answers `engine.inspect.overlay` and `engine.inspect.landmarks`; none
    /// for any other command.
    pub(super) fn execute_inspect_debug(&self, command: &str) -> Option<ProductHostDebugResult> {
        let words: Vec<&str> = command.split_whitespace().collect();
        let (first, rest) = words.split_first()?;
        if *first != OVERLAY_COMMAND && *first != LANDMARKS_COMMAND {
            return None;
        }
        let Some(frames) = self.frame_output.as_ref() else {
            return Some(ProductHostDebugResult::new(
                false,
                "INSPECT_NO_RENDERER: this runtime draws no frames".to_owned(),
            ));
        };
        let driver = frames.driver();
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
        let (shown, sets) = driver.overlays();
        let answer = OverlayAnswer {
            on: shown.any(),
            all_sets: shown.all_sets,
            debug_layer: shown.debug_layer,
            camera: shown.camera,
            only_in: shown.only_in.clone(),
            sets_in: shown.sets_in.iter().cloned().collect(),
            hidden: shown.hidden.iter().cloned().collect(),
            joints: shown.joints.iter().copied().collect(),
            bounds: shown.bounds.iter().copied().collect(),
            ik: shown.ik.iter().copied().collect(),
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
        assert!(change(none.clone(), true, &["joints", "x"]).is_err());
        assert!(change(none, true, &["floor"]).is_err());
    }
}
