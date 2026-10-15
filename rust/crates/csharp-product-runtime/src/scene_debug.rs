//! The committed scene as Engine live-debug commands (`engine.scene.*`):
//! readings of the services' presentation world and camera composition,
//! never of the renderer's own tables.
use super::*;

use render_model::{RenderHandle, RenderLayer};
use render_presentation::{SceneObject, SceneObjectDetail, SceneResource};

pub(super) const OBJECTS_COMMAND: &str = "engine.scene.objects";
pub(super) const OBJECT_COMMAND: &str = "engine.scene.object";
pub(super) const COMPOSITION_COMMAND: &str = "engine.scene.composition";
pub(super) const RESOURCES_COMMAND: &str = "engine.scene.resources";

/// Objects in one `engine.scene.objects` answer, so a large scene's listing
/// stays a bounded answer.
const OBJECTS_PER_PAGE: usize = 100;

const OBJECTS_USAGE: &str =
    "[ROOT|*] [DEPTH] [--layer scene|debug|ui|viewmodel|backdrop] [--page N]";

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectsAnswer<'a> {
    revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    root: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layer: Option<RenderLayer>,
    page: usize,
    pages: usize,
    /// Objects the listing matched, across its pages.
    total: usize,
    objects: Vec<ListedObject<'a>>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ListedObject<'a> {
    /// Levels below the listing's root (or below the scene's roots).
    depth: u32,
    #[serde(flatten)]
    object: SceneObject<'a>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectAnswer<'a> {
    revision: u64,
    #[serde(flatten)]
    object: SceneObjectDetail<'a>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourcesAnswer<'a> {
    revision: u64,
    /// The descriptors the committed scene defines.
    descriptors: Vec<SceneResource<'a>>,
    /// The admitted resource bodies a renderer of it may read.
    bodies: Vec<ResourceBody<'a>>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceBody<'a> {
    identity: &'a str,
    kind: String,
    bytes: usize,
}

#[derive(Default)]
struct ObjectsQuery {
    root: Option<RenderHandle>,
    depth: Option<u32>,
    layer: Option<RenderLayer>,
    page: usize,
}

fn parse_objects(arguments: &str) -> Result<ObjectsQuery, String> {
    let mut query = ObjectsQuery::default();
    let mut positional = 0;
    let mut words = arguments.split_whitespace();
    let usage = || format!("usage: {OBJECTS_COMMAND} {OBJECTS_USAGE}");
    while let Some(word) = words.next() {
        match word {
            "--layer" => {
                query.layer = Some(match words.next().ok_or_else(usage)? {
                    "scene" => RenderLayer::Scene,
                    "debug" => RenderLayer::Debug,
                    "ui" => RenderLayer::Ui,
                    "viewmodel" => RenderLayer::Viewmodel,
                    "backdrop" => RenderLayer::Backdrop,
                    _ => return Err(usage()),
                })
            }
            "--page" => {
                query.page = words
                    .next()
                    .and_then(|page| page.parse().ok())
                    .ok_or_else(usage)?
            }
            "*" if positional == 0 => positional += 1,
            _ if positional == 0 => {
                query.root = Some(RenderHandle::new(word.parse().map_err(|_| usage())?));
                positional += 1;
            }
            _ if positional == 1 => {
                query.depth = Some(word.parse().map_err(|_| usage())?);
                positional += 1;
            }
            _ => return Err(usage()),
        }
    }
    Ok(query)
}

impl CsharpProductRuntime {
    /// Answers the `engine.scene.*` readings; none for any other command.
    pub(super) fn execute_scene_debug(&self, command: &str) -> Option<ProductHostDebugResult> {
        let command = command.trim();
        let (name, rest) = command
            .split_once(char::is_whitespace)
            .unwrap_or((command, ""));
        let world = self.services.presentation_world();
        let revision = world.revision();
        let answer = match name {
            OBJECTS_COMMAND => parse_objects(rest).and_then(|query| {
                let (total, objects) = list_objects(world, &query)?;
                json(&ObjectsAnswer {
                    revision,
                    root: query.root.map(RenderHandle::raw),
                    depth: query.depth,
                    layer: query.layer,
                    page: query.page,
                    pages: total.div_ceil(OBJECTS_PER_PAGE),
                    total,
                    objects,
                })
            }),
            OBJECT_COMMAND => rest
                .trim()
                .parse()
                .map_err(|_| format!("usage: {OBJECT_COMMAND} <handle>"))
                .and_then(|handle| {
                    let object = world
                        .scene_object_detail(RenderHandle::new(handle))
                        .ok_or_else(|| format!("no committed object {handle}"))?;
                    json(&ObjectAnswer { revision, object })
                }),
            COMPOSITION_COMMAND => json(&self.services.view_composition()),
            RESOURCES_COMMAND => json(&ResourcesAnswer {
                revision,
                descriptors: world.scene_resources(),
                bodies: self
                    .services
                    .renderer_resources()
                    .map(|resource| ResourceBody {
                        identity: resource.identity(),
                        kind: format!("{:?}", resource.kind()),
                        bytes: resource.bytes().len(),
                    })
                    .collect(),
            }),
            _ => return None,
        };
        Some(match answer {
            Ok(message) => ProductHostDebugResult::new(true, message),
            Err(message) => ProductHostDebugResult::new(false, message),
        })
    }
}

/// The page of objects `query` names, walked depth first in handle order,
/// with how many it matched in all. Transforms and bounds are composed only
/// for the objects on the page.
fn list_objects<'a>(
    world: &'a render_presentation::PresentationWorld,
    query: &ObjectsQuery,
) -> Result<(usize, Vec<ListedObject<'a>>), String> {
    let starts: Vec<RenderHandle> = match query.root {
        Some(root) => {
            if world.scene_object(root).is_none() {
                return Err(format!("no committed object {}", root.raw()));
            }
            vec![root]
        }
        None => world.scene_roots().collect(),
    };
    let first = query.page * OBJECTS_PER_PAGE;
    let mut matched = 0;
    let mut page = Vec::new();
    let mut stack: Vec<(RenderHandle, u32)> =
        starts.into_iter().rev().map(|start| (start, 0)).collect();
    while let Some((handle, depth)) = stack.pop() {
        if query.depth.is_none_or(|limit| depth < limit) {
            let children: Vec<RenderHandle> = world.scene_children(handle).collect();
            stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
        }
        if query
            .layer
            .is_some_and(|layer| world.scene_layer(handle) != layer)
        {
            continue;
        }
        if (first..first + OBJECTS_PER_PAGE).contains(&matched) {
            page.extend(
                world
                    .scene_object(handle)
                    .map(|object| ListedObject { depth, object }),
            );
        }
        matched += 1;
    }
    Ok((matched, page))
}

fn json(value: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_arguments_parse_in_any_order_after_the_root() {
        let query = parse_objects("12 2 --layer viewmodel --page 3").unwrap();
        assert_eq!(
            (
                query.root.map(RenderHandle::raw),
                query.depth,
                query.layer,
                query.page
            ),
            (Some(12), Some(2), Some(RenderLayer::Viewmodel), 3)
        );
        let all = parse_objects("* 1").unwrap();
        assert_eq!((all.root, all.depth), (None, Some(1)));
        assert!(parse_objects("--layer floor").is_err());
        assert!(parse_objects("1 2 3").is_err());
    }
}
