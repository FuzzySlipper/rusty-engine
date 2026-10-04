//! Product objects projected through Engine-owned retained renderer state.
//!
//! A product names stable object identities and admitted appearance
//! identities. The projector keeps each object's last projected values and
//! turns a batch of changed objects into renderer operations for those objects
//! only. A complete snapshot is an adapter over the same path.

use std::collections::{BTreeMap, BTreeSet};

use render_model::{
    LightDescriptor, RenderDiff, RenderFrameDiff, RenderHandle, RenderLayer, RenderMetadata,
    Transform, JSON_SAFE_U64_MAX,
};
use serde::Deserialize;

use crate::appearance::{
    append_material_parameters, append_node_updates, changed_shared_ids, create_node, light_kind,
    requires_recreate, resource_diffs, validate_appearance, validate_resources, NodeUpdate,
    NodeValues, ResourceSnapshot,
};
use crate::{
    Appearance, AppearanceProjectionError, AppearanceResources, HandleAllocationError,
    JointAttachmentProblem, RenderHandleNamespace, StableHandleRegistry,
};

/// Admitted Engine content available to a trusted product runtime.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeAppearanceCatalog {
    #[serde(default)]
    pub resources: AppearanceResources,
    pub appearances: BTreeMap<String, Appearance>,
}

/// The complete current fact for one product object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeAppearanceFact<'a> {
    pub object_id: u64,
    pub parent_object_id: Option<u64>,
    pub appearance: &'a str,
    pub transform: Transform,
    pub visible: bool,
    pub layer: RenderLayer,
    /// A joint of the parent's animated mesh that this object follows; its
    /// transform is then relative to that joint.
    pub joint: Option<&'a str>,
}

/// The complete current fact for one runtime light. Light identities are a
/// separate key space from object identities.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeLightFact {
    pub light_id: u64,
    pub parent_object_id: Option<u64>,
    pub light: LightDescriptor,
}

#[derive(Debug, Clone)]
struct RetainedObject {
    parent: Option<u64>,
    appearance_id: String,
    /// The appearance as last projected; the catalog entry may since have changed.
    appearance: Appearance,
    transform: Transform,
    visible: bool,
    layer: RenderLayer,
    joint: Option<String>,
}

impl RetainedObject {
    fn values(&self) -> NodeValues<'_> {
        NodeValues {
            appearance: &self.appearance,
            transform: self.transform,
            visible: self.visible,
            layer: self.layer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RetainedKey {
    Object(u64),
    Light(u64),
}

/// An object's values after a change, borrowed from the facts or the catalog.
#[derive(Clone, Copy)]
struct NextObject<'a> {
    parent: Option<u64>,
    appearance_id: &'a str,
    appearance: &'a Appearance,
    transform: Transform,
    visible: bool,
    layer: RenderLayer,
    joint: Option<&'a str>,
}

impl NextObject<'_> {
    fn values(&self) -> NodeValues<'_> {
        NodeValues {
            appearance: self.appearance,
            transform: self.transform,
            visible: self.visible,
            layer: self.layer,
        }
    }

    fn retained(&self) -> RetainedObject {
        RetainedObject {
            parent: self.parent,
            appearance_id: self.appearance_id.to_owned(),
            appearance: self.appearance.clone(),
            transform: self.transform,
            visible: self.visible,
            layer: self.layer,
            joint: self.joint.map(str::to_owned),
        }
    }
}

enum Change<T> {
    Remove,
    Put(T),
}

/// Engine-owned retained objects, lights and resources for one product runtime.
#[derive(Debug, Clone)]
pub struct RuntimeAppearanceProjector {
    catalog: RuntimeAppearanceCatalog,
    registry: StableHandleRegistry<RetainedKey>,
    objects: BTreeMap<u64, RetainedObject>,
    lights: BTreeMap<u64, RuntimeLightFact>,
    /// Retained objects and lights under each parent object.
    children: BTreeMap<u64, BTreeSet<RetainedKey>>,
    /// Retained objects showing each appearance identity.
    users: BTreeMap<String, BTreeSet<u64>>,
    /// Resources as last projected.
    resources: ResourceSnapshot,
    resources_dirty: bool,
    /// Appearance identities changed in the catalog since the last projection.
    dirty_appearances: BTreeSet<String>,
}

impl RuntimeAppearanceProjector {
    pub fn new(catalog: RuntimeAppearanceCatalog) -> Self {
        Self {
            catalog,
            registry: StableHandleRegistry::new(RenderHandleNamespace::AUTHORED),
            objects: BTreeMap::new(),
            lights: BTreeMap::new(),
            children: BTreeMap::new(),
            users: BTreeMap::new(),
            resources: ResourceSnapshot::default(),
            resources_dirty: true,
            dirty_appearances: BTreeSet::new(),
        }
    }

    /// Adds or replaces one appearance. Objects showing it change at the next
    /// projection.
    pub fn insert_appearance(
        &mut self,
        identity: String,
        appearance: Appearance,
    ) -> Option<Appearance> {
        self.dirty_appearances.insert(identity.clone());
        self.catalog.appearances.insert(identity, appearance)
    }

    /// Removes one appearance. The caller removes its objects first.
    pub fn remove_appearance(&mut self, identity: &str) -> Option<Appearance> {
        self.catalog.appearances.remove(identity)
    }

    pub fn appearance(&self, identity: &str) -> Option<&Appearance> {
        self.catalog.appearances.get(identity)
    }

    /// Changes an appearance in place. Objects showing it change at the next
    /// projection.
    pub fn appearance_mut(&mut self, identity: &str) -> Option<&mut Appearance> {
        let appearance = self.catalog.appearances.get_mut(identity)?;
        self.dirty_appearances.insert(identity.to_owned());
        Some(appearance)
    }

    /// Whether any retained object shows this appearance.
    pub fn appearance_in_use(&self, identity: &str) -> bool {
        self.users.contains_key(identity)
    }

    pub fn resources(&self) -> &AppearanceResources {
        &self.catalog.resources
    }

    /// Changes the resource catalog. The next projection defines, redefines
    /// or releases what changed. The caller keeps resources in use by
    /// retained objects.
    pub fn resources_mut(&mut self) -> &mut AppearanceResources {
        self.resources_dirty = true;
        &mut self.catalog.resources
    }

    /// Projects catalog and resource changes made since the last projection.
    pub fn reconcile(&mut self) -> Result<RenderFrameDiff, AppearanceProjectionError> {
        self.change(&[], &[], &[], &[])
    }

    /// Creates or updates the given objects and removes the given identities.
    /// Only these objects, and objects whose appearance or resources changed,
    /// are examined. Removing an absent object does nothing. Failure changes
    /// nothing.
    pub fn apply(
        &mut self,
        facts: &[RuntimeAppearanceFact<'_>],
        removals: &[u64],
    ) -> Result<RenderFrameDiff, AppearanceProjectionError> {
        self.change(facts, removals, &[], &[])
    }

    /// Makes the given facts the complete object set: a thin adapter over
    /// [`Self::apply`] that removes every retained object the facts omit.
    pub fn project(
        &mut self,
        facts: &[RuntimeAppearanceFact<'_>],
    ) -> Result<RenderFrameDiff, AppearanceProjectionError> {
        let removals = self.omitted_objects(facts);
        self.apply(facts, &removals)
    }

    /// [`Self::apply`] (or, with no removal list, [`Self::project`]) and
    /// [`Self::apply_lights`] as one batch, validated against the state after
    /// both: a light may name a parent object created in the same batch, and
    /// a removed object's lights may be removed with it.
    pub fn change_with_lights(
        &mut self,
        facts: &[RuntimeAppearanceFact<'_>],
        removals: Option<&[u64]>,
        light_facts: &[RuntimeLightFact],
        light_removals: &[u64],
    ) -> Result<RenderFrameDiff, AppearanceProjectionError> {
        match removals {
            Some(removals) => self.change(facts, removals, light_facts, light_removals),
            None => {
                let removals = self.omitted_objects(facts);
                self.change(facts, &removals, light_facts, light_removals)
            }
        }
    }

    /// Retained objects that `facts`, taken as the complete object set, omit.
    fn omitted_objects(&self, facts: &[RuntimeAppearanceFact<'_>]) -> Vec<u64> {
        let mut present: Vec<u64> = facts.iter().map(|fact| fact.object_id).collect();
        present.sort_unstable();
        self.objects
            .keys()
            .copied()
            .filter(|id| present.binary_search(id).is_err())
            .collect()
    }

    /// Creates or updates the given lights and removes the given light identities.
    pub fn apply_lights(
        &mut self,
        facts: &[RuntimeLightFact],
        removals: &[u64],
    ) -> Result<RenderFrameDiff, AppearanceProjectionError> {
        self.change(&[], &[], facts, removals)
    }

    pub fn object_handle(&self, object_id: u64) -> Option<RenderHandle> {
        self.registry.handle_of(&RetainedKey::Object(object_id))
    }

    /// The appearance identity a retained object shows.
    pub fn object_appearance(&self, object_id: u64) -> Option<&str> {
        self.objects
            .get(&object_id)
            .map(|object| object.appearance_id.as_str())
    }

    /// The joint a retained object follows.
    pub fn object_joint(&self, object_id: u64) -> Option<&str> {
        self.objects.get(&object_id)?.joint.as_deref()
    }

    pub fn retained_objects(&self) -> usize {
        self.objects.len()
    }

    pub fn retained_lights(&self) -> usize {
        self.lights.len()
    }

    fn change(
        &mut self,
        facts: &[RuntimeAppearanceFact<'_>],
        removals: &[u64],
        light_facts: &[RuntimeLightFact],
        light_removals: &[u64],
    ) -> Result<RenderFrameDiff, AppearanceProjectionError> {
        let next_resources = if self.resources_dirty {
            Some(validate_resources(
                &self.catalog.resources,
                &self.resources,
            )?)
        } else {
            None
        };
        let resources = next_resources.as_ref().unwrap_or(&self.resources);
        let objects = &self.objects;

        reject_duplicates(facts.iter().map(|fact| fact.object_id), removals)
            .map_err(|id| AppearanceProjectionError::DuplicateObject { id })?;
        reject_duplicates(light_facts.iter().map(|fact| fact.light_id), light_removals)
            .map_err(|id| AppearanceProjectionError::DuplicateLight { id })?;

        // Collect the objects that change: removals, facts that differ from
        // the retained values, and users of appearances changed in the catalog.
        let mut changes: BTreeMap<u64, Change<NextObject<'_>>> = BTreeMap::new();
        for id in removals {
            if objects.contains_key(id) {
                changes.insert(*id, Change::Remove);
            }
        }
        for fact in facts {
            let id = fact.object_id;
            if id > JSON_SAFE_U64_MAX {
                return Err(AppearanceProjectionError::UnsafeObjectId { id });
            }
            let appearance = self
                .catalog
                .appearances
                .get(fact.appearance)
                .ok_or_else(|| AppearanceProjectionError::UnknownAppearance {
                    id,
                    appearance: fact.appearance.to_owned(),
                })?;
            let next = NextObject {
                parent: fact.parent_object_id,
                appearance_id: fact.appearance,
                appearance,
                transform: fact.transform,
                visible: fact.visible,
                layer: fact.layer,
                joint: fact.joint,
            };
            if objects
                .get(&id)
                .is_some_and(|previous| unchanged(previous, &next, &self.dirty_appearances))
            {
                continue;
            }
            changes.insert(id, Change::Put(next));
        }
        for identity in &self.dirty_appearances {
            for id in self.users.get(identity).into_iter().flatten() {
                if changes.contains_key(id) {
                    continue;
                }
                let previous = &objects[id];
                let appearance = self.catalog.appearances.get(identity).ok_or_else(|| {
                    AppearanceProjectionError::UnknownAppearance {
                        id: *id,
                        appearance: identity.clone(),
                    }
                })?;
                if *appearance != previous.appearance {
                    changes.insert(
                        *id,
                        Change::Put(NextObject {
                            parent: previous.parent,
                            appearance_id: &previous.appearance_id,
                            appearance,
                            transform: previous.transform,
                            visible: previous.visible,
                            layer: previous.layer,
                            joint: previous.joint.as_deref(),
                        }),
                    );
                }
            }
        }

        let mut light_changes: BTreeMap<u64, Change<&RuntimeLightFact>> = BTreeMap::new();
        for id in light_removals {
            if self.lights.contains_key(id) {
                light_changes.insert(*id, Change::Remove);
            }
        }
        for fact in light_facts {
            let id = fact.light_id;
            if id > JSON_SAFE_U64_MAX {
                return Err(AppearanceProjectionError::UnsafeLightId { id });
            }
            if self.lights.get(&id) != Some(fact) {
                light_changes.insert(id, Change::Put(fact));
            }
        }

        let next_parent = |id: u64| -> Option<Option<u64>> {
            match changes.get(&id) {
                Some(Change::Remove) => None,
                Some(Change::Put(next)) => Some(next.parent),
                None => objects.get(&id).map(|object| object.parent),
            }
        };
        let next_appearance = |id: u64| -> Option<&Appearance> {
            match changes.get(&id) {
                Some(Change::Remove) => None,
                Some(Change::Put(next)) => Some(next.appearance),
                None => objects.get(&id).map(|object| &object.appearance),
            }
        };

        // Validate every change against the state after the whole batch.
        for (id, change) in &changes {
            let id = *id;
            match change {
                Change::Put(next) => {
                    let previous = objects.get(&id);
                    next.transform.validate().map_err(|source| {
                        AppearanceProjectionError::InvalidNodeTransform { id, source }
                    })?;
                    let appearance_changed = previous.is_none_or(|previous| {
                        previous.appearance_id != next.appearance_id
                            || previous.appearance != *next.appearance
                    });
                    if appearance_changed || previous.is_some_and(|p| p.layer != next.layer) {
                        validate_appearance(
                            id,
                            next.values(),
                            &object_metadata(id, next.appearance_id),
                            resources,
                        )?;
                    }
                    if let Some(parent) = next.parent {
                        if next_parent(parent).is_none() {
                            return Err(AppearanceProjectionError::MissingParent { id, parent });
                        }
                        if previous.is_none_or(|previous| previous.parent != next.parent) {
                            let mut cursor = Some(parent);
                            let mut steps = 0_usize;
                            while let Some(ancestor) = cursor {
                                steps += 1;
                                if ancestor == id || steps > objects.len() + changes.len() {
                                    return Err(AppearanceProjectionError::ParentCycle { id });
                                }
                                cursor = next_parent(ancestor).flatten();
                            }
                        }
                    }
                    if let Some(joint) = next.joint {
                        validate_joint(
                            id,
                            joint,
                            next.parent,
                            next.parent.and_then(next_appearance),
                            resources,
                        )?;
                    }
                    if appearance_changed && previous.is_some() {
                        // Attached children that stay put must still find their joint.
                        for key in self.children.get(&id).into_iter().flatten() {
                            let RetainedKey::Object(child) = *key else {
                                continue;
                            };
                            if changes.contains_key(&child) {
                                continue;
                            }
                            if let Some(joint) = objects[&child].joint.as_deref() {
                                validate_joint(
                                    child,
                                    joint,
                                    Some(id),
                                    Some(next.appearance),
                                    resources,
                                )?;
                            }
                        }
                    }
                }
                Change::Remove => {
                    for key in self.children.get(&id).into_iter().flatten() {
                        match *key {
                            RetainedKey::Object(child) => {
                                if next_parent(child).is_some_and(|parent| parent == Some(id)) {
                                    return Err(AppearanceProjectionError::MissingParent {
                                        id: child,
                                        parent: id,
                                    });
                                }
                            }
                            RetainedKey::Light(light) => {
                                let reparented = match light_changes.get(&light) {
                                    Some(Change::Remove) => true,
                                    Some(Change::Put(fact)) => fact.parent_object_id != Some(id),
                                    None => false,
                                };
                                if !reparented {
                                    return Err(AppearanceProjectionError::MissingLightParent {
                                        id: light,
                                        parent: id,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        for (id, change) in &light_changes {
            let Change::Put(fact) = change else {
                continue;
            };
            let id = *id;
            if let Some(parent) = fact.parent_object_id {
                if next_parent(parent).is_none() {
                    return Err(AppearanceProjectionError::MissingLightParent { id, parent });
                }
            }
            fact.light
                .validate()
                .map_err(|source| AppearanceProjectionError::InvalidLight { id, source })?;
        }

        // A structural change, or a redefined mesh, needs a new renderer
        // object. The renderer removes a destroyed object's children with it,
        // so every retained descendant is recreated too.
        let mut recreate = BTreeSet::new();
        if let Some(next) = &next_resources {
            let static_meshes =
                changed_shared_ids(&self.resources.static_meshes, &next.static_meshes);
            let animated_meshes =
                changed_shared_ids(&self.resources.animated_meshes, &next.animated_meshes);
            if !static_meshes.is_empty() || !animated_meshes.is_empty() {
                for id in objects.keys() {
                    let redefined = match next_appearance(*id) {
                        Some(Appearance::StaticMesh { asset, .. }) => static_meshes.contains(asset),
                        Some(Appearance::AnimatedMesh { asset, .. }) => {
                            animated_meshes.contains(asset)
                        }
                        _ => false,
                    };
                    if redefined {
                        recreate.insert(*id);
                    }
                }
            }
        }
        for (id, change) in &changes {
            if let (Change::Put(next), Some(previous)) = (change, objects.get(id)) {
                if previous.parent != next.parent
                    || previous.layer != next.layer
                    || requires_recreate(&previous.appearance, next.appearance)
                {
                    recreate.insert(*id);
                }
            }
        }
        let mut recreate_lights = BTreeSet::new();
        let mut pending: Vec<u64> = recreate.iter().copied().collect();
        while let Some(id) = pending.pop() {
            for key in self.children.get(&id).into_iter().flatten() {
                match *key {
                    RetainedKey::Object(child) => {
                        if next_parent(child).is_some() && recreate.insert(child) {
                            pending.push(child);
                        }
                    }
                    RetainedKey::Light(light) => {
                        recreate_lights.insert(light);
                    }
                }
            }
        }
        for (id, change) in &light_changes {
            match (change, self.lights.get(id)) {
                (Change::Put(fact), Some(previous))
                    if previous.parent_object_id != fact.parent_object_id
                        || light_kind(&previous.light) != light_kind(&fact.light) =>
                {
                    recreate_lights.insert(*id);
                }
                (Change::Remove, _) => {
                    recreate_lights.remove(id);
                }
                _ => {}
            }
        }

        let created: Vec<u64> = changes
            .iter()
            .filter(|(id, change)| matches!(change, Change::Put(_)) && !objects.contains_key(id))
            .map(|(id, _)| *id)
            .chain(recreate.iter().copied())
            .collect();
        let created_lights: Vec<u64> = light_changes
            .iter()
            .filter(|(id, change)| {
                matches!(change, Change::Put(_)) && !self.lights.contains_key(id)
            })
            .map(|(id, _)| *id)
            .chain(recreate_lights.iter().copied())
            .collect();
        if !self
            .registry
            .can_allocate(created.len() + created_lights.len())
        {
            return Err(AppearanceProjectionError::Handle(
                HandleAllocationError::NamespaceExhausted {
                    namespace: RenderHandleNamespace::AUTHORED.raw(),
                },
            ));
        }

        // Renderer operations for retained handles. Lights go before the
        // objects they hang from, children before parents.
        let handle = |key| {
            self.registry
                .handle_of(&key)
                .expect("a retained object or light has a handle")
        };
        let mut operations = Vec::new();
        for (id, change) in &light_changes {
            if matches!(change, Change::Remove) || recreate_lights.contains(id) {
                operations.push(RenderDiff::Destroy {
                    handle: handle(RetainedKey::Light(*id)),
                });
            }
        }
        for id in &recreate_lights {
            if !light_changes.contains_key(id) {
                operations.push(RenderDiff::Destroy {
                    handle: handle(RetainedKey::Light(*id)),
                });
            }
        }
        let mut destroyed: Vec<u64> = changes
            .iter()
            .filter(|(_, change)| matches!(change, Change::Remove))
            .map(|(id, _)| *id)
            .chain(recreate.iter().copied())
            .collect();
        destroyed.sort_by_key(|id| std::cmp::Reverse(depth(*id, |id| objects.get(&id)?.parent)));
        for id in &destroyed {
            operations.push(RenderDiff::Destroy {
                handle: handle(RetainedKey::Object(*id)),
            });
        }
        // A redefined definition cannot replace a live instance: dependents
        // are destroyed above and created again below.
        if let Some(next) = &next_resources {
            operations.extend(resource_diffs(&self.resources, next));
        }
        let mut joints = Vec::new();
        for (id, change) in &changes {
            let (Change::Put(next), Some(previous)) = (change, objects.get(id)) else {
                continue;
            };
            if recreate.contains(id) {
                continue;
            }
            let object = handle(RetainedKey::Object(*id));
            append_node_updates(
                &mut operations,
                object,
                &previous.appearance,
                next.appearance,
                NodeUpdate {
                    transform: (previous.transform != next.transform).then_some(next.transform),
                    visible: (previous.visible != next.visible).then_some(next.visible),
                    metadata: (previous.appearance_id != next.appearance_id)
                        .then(|| object_metadata(*id, next.appearance_id)),
                },
            );
            if previous.joint.as_deref() != next.joint {
                joints.push(RenderDiff::SetParentJoint {
                    handle: object,
                    joint: next.joint.map(str::to_owned),
                });
            }
        }
        let mut light_updates = Vec::new();
        for (id, change) in &light_changes {
            if let Change::Put(fact) = change {
                if self.lights.contains_key(id) && !recreate_lights.contains(id) {
                    light_updates.push(RenderDiff::UpdateLight {
                        handle: handle(RetainedKey::Light(*id)),
                        light: fact.light.clone(),
                    });
                }
            }
        }

        // Everything is valid; commit.
        let object_changes: Vec<(u64, Change<RetainedObject>)> = changes
            .into_iter()
            .map(|(id, change)| match change {
                Change::Remove => (id, Change::Remove),
                Change::Put(next) => (id, Change::Put(next.retained())),
            })
            .collect();
        let light_changes: Vec<(u64, Change<RuntimeLightFact>)> = light_changes
            .into_iter()
            .map(|(id, change)| match change {
                Change::Remove => (id, Change::Remove),
                Change::Put(fact) => (id, Change::Put(fact.clone())),
            })
            .collect();
        if let Some(next) = next_resources {
            self.resources = next;
            self.resources_dirty = false;
        }
        self.dirty_appearances.clear();
        for id in &destroyed {
            self.registry.remove(&RetainedKey::Object(*id));
        }
        for (id, change) in &light_changes {
            if matches!(change, Change::Remove) {
                self.registry.remove(&RetainedKey::Light(*id));
            }
        }
        for id in &recreate_lights {
            self.registry.remove(&RetainedKey::Light(*id));
        }
        for (id, change) in object_changes {
            self.commit_object(id, change);
        }
        for (id, change) in light_changes {
            self.commit_light(id, change);
        }

        let mut created = created;
        created.sort_by_key(|id| (depth(*id, |id| self.objects.get(&id)?.parent), *id));
        for id in created {
            let object = &self.objects[&id];
            let handle = self
                .registry
                .allocate(RetainedKey::Object(id))
                .expect("handle capacity was checked");
            let parent = object.parent.map(|parent| {
                self.registry
                    .handle_of(&RetainedKey::Object(parent))
                    .expect("a parent is created before its children")
            });
            operations.push(create_node(
                handle,
                parent,
                object.values(),
                object_metadata(id, &object.appearance_id),
            ));
            if let Appearance::AnimatedMesh {
                material_parameters,
                ..
            } = &object.values().appearance
            {
                append_material_parameters(
                    &mut operations,
                    handle,
                    &BTreeMap::new(),
                    material_parameters,
                );
            }
            if let Some(joint) = &object.joint {
                joints.push(RenderDiff::SetParentJoint {
                    handle,
                    joint: Some(joint.clone()),
                });
            }
        }
        operations.extend(joints);
        for id in created_lights {
            let fact = &self.lights[&id];
            let handle = self
                .registry
                .allocate(RetainedKey::Light(id))
                .expect("handle capacity was checked");
            let parent = fact.parent_object_id.map(|parent| {
                self.registry
                    .handle_of(&RetainedKey::Object(parent))
                    .expect("a light's parent exists")
            });
            operations.push(RenderDiff::CreateLight {
                handle,
                parent,
                light: fact.light.clone(),
            });
        }
        operations.extend(light_updates);
        Ok(RenderFrameDiff {
            ops: operations,
            ..RenderFrameDiff::default()
        })
    }

    fn commit_object(&mut self, id: u64, change: Change<RetainedObject>) {
        let previous = match change {
            Change::Remove => self.objects.remove(&id),
            Change::Put(next) => {
                if let Some(parent) = next.parent {
                    link(&mut self.children, parent, RetainedKey::Object(id));
                }
                link(&mut self.users, next.appearance_id.clone(), id);
                self.objects.insert(id, next)
            }
        };
        let Some(previous) = previous else {
            return;
        };
        let current = self.objects.get(&id);
        if current.is_none_or(|current| current.appearance_id != previous.appearance_id) {
            unlink(&mut self.users, &previous.appearance_id, &id);
        }
        if let Some(parent) = previous.parent {
            if current.is_none_or(|current| current.parent != Some(parent)) {
                unlink(&mut self.children, &parent, &RetainedKey::Object(id));
            }
        }
    }

    fn commit_light(&mut self, id: u64, change: Change<RuntimeLightFact>) {
        let parent = match &change {
            Change::Remove => None,
            Change::Put(fact) => fact.parent_object_id,
        };
        let previous = match change {
            Change::Remove => self.lights.remove(&id),
            Change::Put(fact) => self.lights.insert(id, fact),
        };
        let previous_parent = previous.and_then(|light| light.parent_object_id);
        if previous_parent != parent {
            if let Some(old) = previous_parent {
                unlink(&mut self.children, &old, &RetainedKey::Light(id));
            }
            if let Some(parent) = parent {
                link(&mut self.children, parent, RetainedKey::Light(id));
            }
        }
    }
}

fn unchanged(
    previous: &RetainedObject,
    next: &NextObject<'_>,
    dirty_appearances: &BTreeSet<String>,
) -> bool {
    previous.parent == next.parent
        && previous.appearance_id == next.appearance_id
        && previous.transform == next.transform
        && previous.visible == next.visible
        && previous.layer == next.layer
        && previous.joint.as_deref() == next.joint
        && (!dirty_appearances.contains(next.appearance_id)
            || previous.appearance == *next.appearance)
}

/// Returns the first identity named twice across the facts and removals.
fn reject_duplicates(facts: impl Iterator<Item = u64>, removals: &[u64]) -> Result<(), u64> {
    let mut ids: Vec<u64> = facts.chain(removals.iter().copied()).collect();
    ids.sort_unstable();
    match ids.windows(2).find(|pair| pair[0] == pair[1]) {
        Some(pair) => Err(pair[0]),
        None => Ok(()),
    }
}

fn depth(id: u64, parent: impl Fn(u64) -> Option<u64>) -> usize {
    let mut result = 0;
    let mut cursor = parent(id);
    while let Some(ancestor) = cursor {
        result += 1;
        cursor = parent(ancestor);
    }
    result
}

fn object_metadata(object_id: u64, appearance: &str) -> RenderMetadata {
    RenderMetadata {
        source_entity: Some(object_id),
        source_scene_node: None,
        tags: Vec::new(),
        label: Some(appearance.to_owned()),
    }
}

fn validate_joint(
    id: u64,
    joint: &str,
    parent: Option<u64>,
    parent_appearance: Option<&Appearance>,
    resources: &ResourceSnapshot,
) -> Result<(), AppearanceProjectionError> {
    let refuse = |problem| {
        Err(AppearanceProjectionError::JointAttachment {
            id,
            joint: joint.to_owned(),
            problem,
        })
    };
    if parent.is_none() {
        return refuse(JointAttachmentProblem::NoParent);
    }
    let Some(Appearance::AnimatedMesh { asset, .. }) = parent_appearance else {
        return refuse(JointAttachmentProblem::ParentNotAnimated);
    };
    let matches = resources
        .animated_meshes
        .get(asset)
        .and_then(|mesh| mesh.rig.as_ref())
        .map_or(0, |rig| {
            rig.joints
                .iter()
                .filter(|candidate| candidate.id == joint)
                .count()
        });
    match matches {
        1 => Ok(()),
        0 => refuse(JointAttachmentProblem::MissingJoint),
        _ => refuse(JointAttachmentProblem::AmbiguousJoint),
    }
}

fn link<K: Ord, V: Ord>(index: &mut BTreeMap<K, BTreeSet<V>>, key: K, value: V) {
    index.entry(key).or_default().insert(value);
}

fn unlink<K, V, Q>(index: &mut BTreeMap<K, BTreeSet<V>>, key: &Q, value: &V)
where
    K: Ord + std::borrow::Borrow<Q>,
    V: Ord,
    Q: Ord + ?Sized,
{
    if let Some(values) = index.get_mut(key) {
        values.remove(value);
        if values.is_empty() {
            index.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use render_model::{
        AnimatedMeshAsset, AnimatedMeshRuntimeFormat, AnimationBindRestConvention,
        AnimationClipDescriptor, AnimationRigJoint, AnimationRigSignature, AnimationRootConvention,
        Geometry, LightShadowIntent, Material, MaterialUvStrategy, MeshAttribute,
        MeshAttributeKind, MeshAttributeName, MeshBoundsDescriptor, MeshBufferLayout,
        MeshCollisionPolicy, MeshGroupDescriptor, MeshIndexWidth, MeshMaterialSlot,
        MeshPayloadDescriptor, MeshPayloadSource, MeshProvenance, RenderMaterialDescriptor,
        StaticMeshAsset,
    };

    use super::*;

    const CUBE: &str = "appearance/cube";
    const SPHERE: &str = "appearance/sphere";
    const MESH: &str = "appearance/mesh";
    const BODY: &str = "appearance/body";

    fn material() -> RenderMaterialDescriptor {
        RenderMaterialDescriptor {
            shader: None,
            id: "material/plain".to_string(),
            color: [0.4, 0.5, 0.6, 1.0],
            texture: None,
            roughness: 1.0,
            metalness: 0.0,
            texture_tint: [1.0; 4],
            emission_color: [0.0; 3],
            emission_intensity: 0.0,
            uv_strategy: MaterialUvStrategy::Flat,
            alpha_mode: Default::default(),
            double_sided: false,
            voxel_surface: None,
            normal_map: None,
            triplanar: None,
        }
    }

    fn mesh() -> StaticMeshAsset {
        StaticMeshAsset {
            asset: "mesh/triangle".to_string(),
            payload: MeshPayloadDescriptor {
                texture_space: None,
                layout: MeshBufferLayout {
                    vertex_count: 3,
                    index_count: 3,
                    index_width: MeshIndexWidth::U32,
                    attributes: vec![
                        MeshAttribute {
                            name: MeshAttributeName::Position,
                            components: 3,
                            kind: MeshAttributeKind::F32,
                        },
                        MeshAttribute {
                            name: MeshAttributeName::Normal,
                            components: 3,
                            kind: MeshAttributeKind::F32,
                        },
                    ],
                },
                groups: vec![MeshGroupDescriptor {
                    material_slot: 0,
                    start: 0,
                    count: 3,
                }],
                bounds: MeshBoundsDescriptor {
                    min: [0.0; 3],
                    max: [1.0, 1.0, 0.0],
                },
                source: MeshPayloadSource::Inline {
                    positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
                    uvs: None,
                    colors: None,
                    indices: vec![0, 1, 2],
                },
                provenance: MeshProvenance::StaticAsset,
            },
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/plain".to_string(),
            }],
            collision: MeshCollisionPolicy::VisualOnly,
        }
    }

    fn animated_mesh() -> AnimatedMeshAsset {
        let hash = format!("sha256:{}", "a".repeat(64));
        AnimatedMeshAsset {
            asset: "mesh-animation/character".to_string(),
            runtime_format: AnimatedMeshRuntimeFormat::Glb,
            content_hash: Some("first".to_string()),
            clips: vec![AnimationClipDescriptor {
                id: "idle".to_string(),
                name: Some("Idle".to_string()),
                duration_seconds: Some(1.0),
            }],
            rig: Some(AnimationRigSignature {
                joints: vec![
                    AnimationRigJoint {
                        id: "Root".to_owned(),
                        parent: None,
                    },
                    AnimationRigJoint {
                        id: "Hand".to_owned(),
                        parent: Some("Root".to_owned()),
                    },
                ],
                bind_rest_hash: hash,
                bind_rest_convention: AnimationBindRestConvention::LocalMatrixV1,
                root_convention: AnimationRootConvention::InPlace,
                root_joint_id: "Root".to_owned(),
                structural_root_ids: vec!["Root".to_owned()],
                designated_motion_root_ids: Vec::new(),
                authored_pose_translation_joint_ids: Vec::new(),
            }),
            clip_packs: vec![],
            default_clip: Some("idle".to_string()),
            embedded_material_slots: vec![],
            material_slots: vec![MeshMaterialSlot {
                slot: 0,
                material: "material/plain".to_string(),
            }],
            bounds: MeshBoundsDescriptor {
                min: [-0.5, 0.0, -0.5],
                max: [0.5, 2.0, 0.5],
            },
        }
    }

    fn primitive(geometry: Geometry) -> Appearance {
        Appearance::Primitive {
            geometry,
            material: Material::DEFAULT,
        }
    }

    /// A projector whose catalog resources are already defined.
    fn projector() -> RuntimeAppearanceProjector {
        let mut projector = RuntimeAppearanceProjector::new(RuntimeAppearanceCatalog {
            resources: AppearanceResources {
                materials: vec![material()],
                static_meshes: vec![Arc::new(mesh())],
                animated_meshes: vec![Arc::new(animated_mesh())],
                ..AppearanceResources::default()
            },
            appearances: BTreeMap::from([
                (CUBE.to_owned(), primitive(Geometry::Cube)),
                (SPHERE.to_owned(), primitive(Geometry::Sphere)),
                (
                    MESH.to_owned(),
                    Appearance::StaticMesh {
                        asset: "mesh/triangle".to_owned(),
                        material_overrides: Vec::new(),
                    },
                ),
                (
                    BODY.to_owned(),
                    Appearance::AnimatedMesh {
                        inspection: Default::default(),
                        asset: "mesh-animation/character".to_owned(),
                        material_overrides: Vec::new(),
                        playback: None,
                        material_parameters: BTreeMap::new(),
                    },
                ),
            ]),
        });
        assert_eq!(
            kinds(&projector.reconcile().unwrap()),
            ["define-material", "define-mesh", "define-animated"]
        );
        projector
    }

    fn fact(id: u64) -> RuntimeAppearanceFact<'static> {
        RuntimeAppearanceFact {
            object_id: id,
            parent_object_id: None,
            appearance: CUBE,
            transform: Transform::IDENTITY,
            visible: true,
            layer: RenderLayer::Scene,
            joint: None,
        }
    }

    fn child(id: u64, parent: u64) -> RuntimeAppearanceFact<'static> {
        RuntimeAppearanceFact {
            parent_object_id: Some(parent),
            ..fact(id)
        }
    }

    fn moved(id: u64, x: f32) -> RuntimeAppearanceFact<'static> {
        let mut fact = fact(id);
        fact.transform.translation = [x, 0.0, 0.0];
        fact
    }

    fn ambient_light(id: u64, parent_object_id: Option<u64>) -> RuntimeLightFact {
        RuntimeLightFact {
            light_id: id,
            parent_object_id,
            light: LightDescriptor::Ambient {
                color: [0.2, 0.3, 0.4],
                intensity: 0.5,
                enabled: true,
                shadow_intent: LightShadowIntent::Requested,
            },
        }
    }

    fn kinds(frame: &RenderFrameDiff) -> Vec<&'static str> {
        frame
            .ops
            .iter()
            .map(|operation| match operation {
                RenderDiff::Create { .. } => "create",
                RenderDiff::CreateStaticMeshInstance { .. } => "create-mesh",
                RenderDiff::CreateAnimatedMeshInstance { .. } => "create-animated",
                RenderDiff::CreateLight { .. } => "create-light",
                RenderDiff::Destroy { .. } => "destroy",
                RenderDiff::Update { .. } => "update",
                RenderDiff::UpdateLight { .. } => "update-light",
                RenderDiff::SetParentJoint { .. } => "joint",
                RenderDiff::SetAnimatedMeshInspection { .. } => "inspection",
                RenderDiff::SetMaterialInstanceParameters {
                    parameters: Some(_),
                    ..
                } => "parameters",
                RenderDiff::SetMaterialInstanceParameters { .. } => "clear-parameters",
                RenderDiff::DefineMaterial { .. } => "define-material",
                RenderDiff::DefineStaticMesh { .. } => "define-mesh",
                RenderDiff::DefineAnimatedMesh { .. } => "define-animated",
                RenderDiff::ReleaseStaticMesh { .. } => "release-mesh",
                _ => "other",
            })
            .collect()
    }

    #[test]
    fn a_batch_touches_only_the_objects_it_changes() {
        let mut projector = projector();
        let everything: Vec<_> = (1..=1_000).map(fact).collect();
        let created = projector.project(&everything).unwrap();
        assert_eq!(
            created
                .ops
                .iter()
                .filter(|op| matches!(op, RenderDiff::Create { .. }))
                .count(),
            1_000
        );
        let handle = projector.object_handle(7).unwrap();

        let frame = projector.apply(&[moved(7, 3.0)], &[]).unwrap();
        assert!(matches!(
            frame.ops.as_slice(),
            [RenderDiff::Update { handle: updated, transform: Some(_), material: None, visible: None, metadata: None }]
                if *updated == handle
        ));
        assert!(projector.apply(&[moved(7, 3.0)], &[]).unwrap().is_empty());

        let hidden = RuntimeAppearanceFact {
            visible: false,
            ..moved(7, 3.0)
        };
        assert!(matches!(
            projector.apply(&[hidden], &[]).unwrap().ops.as_slice(),
            [RenderDiff::Update {
                visible: Some(false),
                transform: None,
                ..
            }]
        ));

        let removed = projector.apply(&[], &[7, 99_999]).unwrap();
        assert!(
            matches!(removed.ops.as_slice(), [RenderDiff::Destroy { handle: destroyed }] if *destroyed == handle)
        );
        assert_eq!(projector.retained_objects(), 999);
    }

    #[test]
    fn a_snapshot_is_the_complete_object_set() {
        let mut projector = projector();
        projector.project(&[fact(1), fact(2), fact(3)]).unwrap();
        let frame = projector
            .project(&[fact(1), moved(3, 2.0), fact(4)])
            .unwrap();
        assert_eq!(kinds(&frame), ["destroy", "update", "create"]);
        assert!(projector.object_handle(2).is_none());
        assert!(projector
            .project(&[fact(1), moved(3, 2.0), fact(4)])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn parents_create_first_and_a_recreated_parent_recreates_its_subtree() {
        let mut projector = projector();
        let initial = projector
            .apply(&[child(3, 2), child(2, 1), fact(1)], &[])
            .unwrap();
        assert_eq!(kinds(&initial), ["create", "create", "create"]);
        let RenderDiff::Create { parent: None, .. } = &initial.ops[0] else {
            panic!("the root is created first");
        };
        projector
            .apply_lights(&[ambient_light(1, Some(2))], &[])
            .unwrap();

        // A geometry change recreates the object, its descendants and their lights.
        let sphere = RuntimeAppearanceFact {
            appearance: SPHERE,
            ..child(2, 1)
        };
        let frame = projector.apply(&[sphere], &[]).unwrap();
        assert_eq!(
            kinds(&frame),
            [
                "destroy",
                "destroy",
                "destroy",
                "create",
                "create",
                "create-light"
            ]
        );

        // Reparenting recreates only the moved branch.
        let frame = projector.apply(&[fact(3)], &[]).unwrap();
        assert_eq!(kinds(&frame), ["destroy", "create"]);
        let RenderDiff::Create { parent: None, .. } = &frame.ops[1] else {
            panic!("object 3 moved to the root");
        };
    }

    #[test]
    fn removing_a_parent_needs_its_children_removed_or_moved() {
        let mut projector = projector();
        projector.apply(&[fact(1), child(2, 1)], &[]).unwrap();
        projector
            .apply_lights(&[ambient_light(5, Some(1))], &[])
            .unwrap();
        assert_eq!(
            projector.apply(&[], &[1]),
            Err(AppearanceProjectionError::MissingParent { id: 2, parent: 1 })
        );
        assert_eq!(
            projector.apply(&[fact(2)], &[1]),
            Err(AppearanceProjectionError::MissingLightParent { id: 5, parent: 1 })
        );
        projector.apply_lights(&[], &[5]).unwrap();
        let frame = projector.apply(&[fact(2)], &[1]).unwrap();
        assert_eq!(kinds(&frame), ["destroy", "destroy", "create"]);
        assert_eq!(projector.retained_objects(), 1);
    }

    #[test]
    fn one_batch_creates_a_parent_with_its_light_and_removes_them_together() {
        let mut projector = projector();
        let frame = projector
            .change_with_lights(&[fact(1)], None, &[ambient_light(5, Some(1))], &[])
            .unwrap();
        assert_eq!(kinds(&frame), ["create", "create-light"]);
        assert_eq!(projector.retained_lights(), 1);
        // The complete object set omits the parent; its light leaves with it.
        let frame = projector.change_with_lights(&[], None, &[], &[5]).unwrap();
        assert_eq!(projector.retained_objects(), 0);
        assert_eq!(projector.retained_lights(), 0);
        assert!(!frame.ops.is_empty());
    }

    #[test]
    fn a_refused_batch_changes_nothing() {
        let mut projector = projector();
        projector.apply(&[fact(1), fact(2)], &[]).unwrap();
        let handle = projector.object_handle(1).unwrap();

        let unknown = RuntimeAppearanceFact {
            appearance: "appearance/missing",
            ..moved(2, 1.0)
        };
        assert!(matches!(
            projector.apply(&[moved(1, 5.0), unknown], &[]),
            Err(AppearanceProjectionError::UnknownAppearance { id: 2, .. })
        ));
        assert!(matches!(
            projector.apply(&[moved(1, 5.0), child(2, 99)], &[]),
            Err(AppearanceProjectionError::MissingParent { id: 2, parent: 99 })
        ));
        assert!(matches!(
            projector.apply(&[child(1, 2), child(2, 1)], &[]),
            Err(AppearanceProjectionError::ParentCycle { .. })
        ));
        assert_eq!(
            projector.apply(&[fact(1), moved(1, 2.0)], &[]),
            Err(AppearanceProjectionError::DuplicateObject { id: 1 })
        );
        assert_eq!(
            projector.apply(&[fact(1)], &[1]),
            Err(AppearanceProjectionError::DuplicateObject { id: 1 })
        );
        assert_eq!(projector.object_handle(1), Some(handle));
        assert!(projector
            .apply(&[fact(1), fact(2)], &[])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn lights_share_the_retained_scene_with_objects() {
        let mut projector = projector();
        projector.apply(&[fact(7)], &[]).unwrap();
        let object = projector.object_handle(7).unwrap();
        let light = projector
            .apply_lights(&[ambient_light(7, Some(7))], &[])
            .unwrap();
        assert!(matches!(
            light.ops.as_slice(),
            [RenderDiff::CreateLight { parent: Some(parent), .. }] if *parent == object
        ));

        let mut brighter = ambient_light(7, Some(7));
        if let LightDescriptor::Ambient { intensity, .. } = &mut brighter.light {
            *intensity = 1.0;
        }
        assert_eq!(
            kinds(&projector.apply_lights(&[brighter], &[]).unwrap()),
            ["update-light"]
        );
        assert_eq!(
            kinds(&projector.apply(&[moved(7, 1.0)], &[]).unwrap()),
            ["update"]
        );
        assert_eq!(
            kinds(&projector.apply_lights(&[], &[7]).unwrap()),
            ["destroy"]
        );
        assert_eq!(projector.retained_lights(), 0);
    }

    #[test]
    fn a_changed_appearance_reaches_the_objects_showing_it() {
        let mut projector = projector();
        let mesh = RuntimeAppearanceFact {
            appearance: MESH,
            ..fact(2)
        };
        projector.apply(&[fact(1), mesh, fact(3)], &[]).unwrap();
        let cube = projector.object_handle(1).unwrap();

        if let Some(Appearance::Primitive { material, .. }) = projector.appearance_mut(CUBE) {
            material.color = [1.0, 0.0, 0.0, 1.0];
        }
        let frame = projector.reconcile().unwrap();
        assert_eq!(kinds(&frame), ["update", "update"]);
        assert!(
            matches!(&frame.ops[0], RenderDiff::Update { handle, material: Some(_), .. } if *handle == cube)
        );

        // Material overrides are fixed at creation.
        if let Some(Appearance::StaticMesh {
            material_overrides, ..
        }) = projector.appearance_mut(MESH)
        {
            material_overrides.push(MeshMaterialSlot {
                slot: 0,
                material: "material/plain".to_owned(),
            });
        }
        assert_eq!(
            kinds(&projector.reconcile().unwrap()),
            ["destroy", "create-mesh"]
        );
        assert!(projector.appearance_in_use(MESH));
        projector.apply(&[], &[2]).unwrap();
        assert!(!projector.appearance_in_use(MESH));
    }

    #[test]
    fn a_redefined_mesh_destroys_its_instances_before_redefining_them() {
        let mut projector = projector();
        let mesh = |id| RuntimeAppearanceFact {
            appearance: MESH,
            ..fact(id)
        };
        projector
            .apply(&[mesh(1), child(2, 1), mesh(4)], &[])
            .unwrap();
        projector
            .apply_lights(&[ambient_light(3, Some(1))], &[])
            .unwrap();

        Arc::make_mut(&mut projector.resources_mut().static_meshes[0])
            .payload
            .bounds
            .max[0] = 2.0;
        let frame = projector.reconcile().unwrap();
        assert_eq!(
            kinds(&frame),
            [
                "destroy",
                "destroy",
                "destroy",
                "destroy",
                "define-mesh",
                "create-mesh",
                "create-mesh",
                "create",
                "create-light"
            ]
        );
    }

    #[test]
    fn inspection_updates_in_place_and_a_released_mesh_is_released_once() {
        let mut projector = projector();
        let body = RuntimeAppearanceFact {
            appearance: BODY,
            ..fact(1)
        };
        projector.apply(&[body], &[]).unwrap();
        let handle = projector.object_handle(1);
        if let Some(Appearance::AnimatedMesh { inspection, .. }) = projector.appearance_mut(BODY) {
            inspection.wireframe = true;
        }
        assert_eq!(kinds(&projector.reconcile().unwrap()), ["inspection"]);
        assert_eq!(projector.object_handle(1), handle);

        projector
            .resources_mut()
            .static_meshes
            .retain(|mesh| mesh.asset != "mesh/triangle");
        assert_eq!(kinds(&projector.reconcile().unwrap()), ["release-mesh"]);
        assert!(projector.reconcile().unwrap().is_empty());
    }

    #[test]
    fn material_parameters_follow_creation_and_update_in_place() {
        let parameters = |red: f32| render_model::MaterialInstanceParameters {
            base_color: Some([red, 0.0, 0.0, 1.0]),
            texture_tint: [1.0; 4],
            emission: None,
        };
        let mut projector = projector();
        if let Some(Appearance::AnimatedMesh {
            material_parameters,
            ..
        }) = projector.appearance_mut(BODY)
        {
            material_parameters.insert(0, parameters(1.0));
        }
        let body = RuntimeAppearanceFact {
            appearance: BODY,
            ..fact(1)
        };
        // A new instance takes the appearance's parameters after it exists.
        let frame = projector.apply(&[body], &[]).unwrap();
        assert_eq!(kinds(&frame), ["create-animated", "parameters"]);
        let handle = projector.object_handle(1);

        let set = |projector: &mut RuntimeAppearanceProjector, value| {
            if let Some(Appearance::AnimatedMesh {
                material_parameters,
                ..
            }) = projector.appearance_mut(BODY)
            {
                *material_parameters = value;
            }
        };
        set(&mut projector, BTreeMap::from([(0, parameters(0.5))]));
        assert_eq!(kinds(&projector.reconcile().unwrap()), ["parameters"]);
        set(&mut projector, BTreeMap::new());
        assert_eq!(kinds(&projector.reconcile().unwrap()), ["clear-parameters"]);
        assert_eq!(projector.object_handle(1), handle, "updated in place");
    }

    #[test]
    fn joint_attachments_follow_the_child_fact() {
        let mut projector = projector();
        let body = RuntimeAppearanceFact {
            appearance: BODY,
            ..fact(1)
        };
        let sword = |joint| RuntimeAppearanceFact {
            joint,
            ..child(2, 1)
        };
        assert!(matches!(
            projector.apply(&[body, sword(Some("Tail"))], &[]),
            Err(AppearanceProjectionError::JointAttachment {
                problem: JointAttachmentProblem::MissingJoint,
                ..
            })
        ));
        let frame = projector.apply(&[body, sword(Some("Hand"))], &[]).unwrap();
        assert_eq!(kinds(&frame), ["create-animated", "create", "joint"]);

        let moved_sword = RuntimeAppearanceFact {
            transform: Transform {
                translation: [0.0, 1.0, 0.0],
                ..Transform::IDENTITY
            },
            ..sword(Some("Hand"))
        };
        assert_eq!(
            kinds(&projector.apply(&[moved_sword], &[]).unwrap()),
            ["update"]
        );
        assert!(matches!(
            projector.apply(&[sword(None)], &[]).unwrap().ops.as_slice(),
            [
                RenderDiff::Update { .. },
                RenderDiff::SetParentJoint { joint: None, .. }
            ]
        ));
        projector.apply(&[sword(Some("Hand"))], &[]).unwrap();

        // The body cannot change to an appearance without the joint.
        let cube_body = RuntimeAppearanceFact {
            appearance: CUBE,
            ..fact(1)
        };
        assert!(matches!(
            projector.apply(&[cube_body], &[]),
            Err(AppearanceProjectionError::JointAttachment {
                id: 2,
                problem: JointAttachmentProblem::ParentNotAnimated,
                ..
            })
        ));
        assert!(matches!(
            projector.apply(
                &[
                    fact(3),
                    RuntimeAppearanceFact {
                        joint: Some("Hand"),
                        ..child(4, 3)
                    }
                ],
                &[]
            ),
            Err(AppearanceProjectionError::JointAttachment {
                problem: JointAttachmentProblem::ParentNotAnimated,
                ..
            })
        ));
    }

    /// A renderer that keeps only what a product can observe, and refuses any
    /// operation on a handle it does not hold.
    #[derive(Default)]
    struct Renderer {
        nodes: BTreeMap<RenderHandle, Realized>,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Realized {
        parent: Option<RenderHandle>,
        object: u64,
        shape: String,
        material: Option<Material>,
        transform: Transform,
        visible: bool,
        joint: Option<String>,
    }

    impl Renderer {
        fn apply(&mut self, frame: &RenderFrameDiff) {
            for operation in &frame.ops {
                match operation {
                    RenderDiff::Create {
                        handle,
                        parent,
                        node,
                    } => self.create(
                        *handle,
                        *parent,
                        &node.metadata,
                        format!("{:?}", node.geometry),
                        Some(node.material),
                        node.transform,
                        node.visible,
                    ),
                    RenderDiff::CreateStaticMeshInstance {
                        handle,
                        parent,
                        instance,
                    } => self.create(
                        *handle,
                        *parent,
                        &instance.metadata,
                        format!("{} {:?}", instance.asset, instance.material_overrides),
                        None,
                        instance.transform,
                        instance.visible,
                    ),
                    RenderDiff::Destroy { handle } => {
                        assert!(
                            self.nodes.contains_key(handle),
                            "destroy of stale {handle:?}"
                        );
                        let mut doomed = vec![*handle];
                        while let Some(next) = doomed.pop() {
                            self.nodes.remove(&next);
                            doomed.extend(
                                self.nodes
                                    .iter()
                                    .filter(|(_, node)| node.parent == Some(next))
                                    .map(|(child, _)| *child),
                            );
                        }
                    }
                    RenderDiff::Update {
                        handle,
                        transform,
                        material,
                        visible,
                        metadata,
                    } => {
                        let node = self.nodes.get_mut(handle).expect("update of a live handle");
                        node.transform = transform.unwrap_or(node.transform);
                        node.material = material.or(node.material);
                        node.visible = visible.unwrap_or(node.visible);
                        if let Some(metadata) = metadata {
                            node.object = metadata.source_entity.unwrap();
                        }
                    }
                    RenderDiff::SetParentJoint { handle, joint } => {
                        self.nodes
                            .get_mut(handle)
                            .expect("joint of a live handle")
                            .joint = joint.clone();
                    }
                    RenderDiff::DefineMaterial { .. }
                    | RenderDiff::DefineStaticMesh { .. }
                    | RenderDiff::DefineAnimatedMesh { .. } => {}
                    other => panic!("unexpected operation {other:?}"),
                }
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn create(
            &mut self,
            handle: RenderHandle,
            parent: Option<RenderHandle>,
            metadata: &RenderMetadata,
            shape: String,
            material: Option<Material>,
            transform: Transform,
            visible: bool,
        ) {
            assert!(
                !self.nodes.contains_key(&handle),
                "{handle:?} created twice"
            );
            if let Some(parent) = parent {
                assert!(
                    self.nodes.contains_key(&parent),
                    "parent {parent:?} is not live"
                );
            }
            self.nodes.insert(
                handle,
                Realized {
                    parent,
                    object: metadata.source_entity.unwrap(),
                    shape,
                    material,
                    transform,
                    visible,
                    joint: None,
                },
            );
        }

        /// The realized scene by product object identity.
        fn by_object(&self) -> BTreeMap<u64, (Option<u64>, Realized)> {
            self.nodes
                .values()
                .map(|node| {
                    let parent = node.parent.map(|parent| self.nodes[&parent].object);
                    let mut node = node.clone();
                    node.parent = None;
                    (node.object, (parent, node))
                })
                .collect()
        }
    }

    struct Rng(u64);

    impl Rng {
        fn below(&mut self, bound: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % bound
        }
    }

    #[derive(Clone, Copy)]
    struct Product {
        parent: Option<u64>,
        appearance: &'static str,
        x: f32,
        visible: bool,
    }

    #[test]
    fn incremental_batches_realize_the_same_scene_as_one_snapshot() {
        const OBJECTS: u64 = 40;
        const APPEARANCES: [&str; 3] = [CUBE, SPHERE, MESH];
        let mut rng = Rng(0x8737_5eed);
        let mut projector = projector();
        let mut renderer = Renderer::default();
        let mut products: BTreeMap<u64, Product> = BTreeMap::new();
        let mut red = false;
        for batch in 1..=300 {
            // Change a few objects. A parent always has a smaller identity,
            // so no batch can make a cycle; a removal takes its subtree.
            let mut next = products.clone();
            for _ in 0..=rng.below(5) {
                let id = 1 + rng.below(OBJECTS);
                let parent = (rng.below(2) == 0 && id > 1)
                    .then(|| 1 + rng.below(id - 1))
                    .filter(|parent| next.contains_key(parent));
                match (next.get(&id).copied(), rng.below(6)) {
                    (None, _) => {
                        next.insert(
                            id,
                            Product {
                                parent,
                                appearance: APPEARANCES[rng.below(3) as usize],
                                x: rng.below(100) as f32,
                                visible: true,
                            },
                        );
                    }
                    (Some(mut product), 0) => {
                        product.parent = parent;
                        next.insert(id, product);
                    }
                    (Some(mut product), 1) => {
                        product.appearance = APPEARANCES[rng.below(3) as usize];
                        next.insert(id, product);
                    }
                    (Some(mut product), 2) => {
                        product.visible = !product.visible;
                        next.insert(id, product);
                    }
                    (Some(_), 3) => {
                        let mut doomed = vec![id];
                        while let Some(gone) = doomed.pop() {
                            next.remove(&gone);
                            doomed.extend(
                                next.iter()
                                    .filter(|(_, product)| product.parent == Some(gone))
                                    .map(|(child, _)| *child),
                            );
                        }
                    }
                    (Some(mut product), _) => {
                        product.x += 1.0;
                        next.insert(id, product);
                    }
                }
            }
            let facts: Vec<_> = next
                .iter()
                .filter(|(id, product)| {
                    products.get(id).is_none_or(|old| {
                        old.parent != product.parent
                            || old.appearance != product.appearance
                            || old.x != product.x
                            || old.visible != product.visible
                    })
                })
                .map(|(id, product)| RuntimeAppearanceFact {
                    object_id: *id,
                    parent_object_id: product.parent,
                    appearance: product.appearance,
                    transform: Transform {
                        translation: [product.x, 0.0, 0.0],
                        ..Transform::IDENTITY
                    },
                    visible: product.visible,
                    layer: RenderLayer::Scene,
                    joint: None,
                })
                .collect();
            let removals: Vec<u64> = products
                .keys()
                .filter(|id| !next.contains_key(id))
                .copied()
                .collect();
            renderer.apply(&projector.apply(&facts, &removals).unwrap());
            products = next;
            if batch % 20 == 0 {
                red = !red;
                if let Some(Appearance::Primitive { material, .. }) = projector.appearance_mut(CUBE)
                {
                    material.color = if red {
                        [1.0, 0.0, 0.0, 1.0]
                    } else {
                        Material::DEFAULT.color
                    };
                }
                renderer.apply(&projector.reconcile().unwrap());
            }

            if batch % 25 == 0 {
                let mut fresh = RuntimeAppearanceProjector::new(projector.catalog.clone());
                let mut expected = Renderer::default();
                expected.apply(&fresh.reconcile().unwrap());
                let snapshot: Vec<_> = products
                    .iter()
                    .map(|(id, product)| RuntimeAppearanceFact {
                        object_id: *id,
                        parent_object_id: product.parent,
                        appearance: product.appearance,
                        transform: Transform {
                            translation: [product.x, 0.0, 0.0],
                            ..Transform::IDENTITY
                        },
                        visible: product.visible,
                        layer: RenderLayer::Scene,
                        joint: None,
                    })
                    .collect();
                expected.apply(&fresh.project(&snapshot).unwrap());
                assert_eq!(renderer.by_object(), expected.by_object(), "batch {batch}");
                assert_eq!(renderer.nodes.len(), products.len());
            }
        }
    }
}
