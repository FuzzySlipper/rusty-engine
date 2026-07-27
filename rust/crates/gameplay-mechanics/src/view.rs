use core_ids::EntityId;
use entity_state::{ComponentRevision, EntityState};

use crate::{
    CatalogVersion, IntrinsicSourceBinding, IntrinsicSourcesComponent, MechanicsError, StatValue,
    StatsComponent, TrackValue, TracksComponent,
};

#[derive(Debug, Clone)]
pub struct StatsView<'a> {
    revision: ComponentRevision,
    catalog_version: &'a CatalogVersion,
    values: &'a [StatValue],
}

impl<'a> StatsView<'a> {
    pub fn revision(&self) -> &ComponentRevision {
        &self.revision
    }

    pub const fn catalog_version(&self) -> &'a CatalogVersion {
        self.catalog_version
    }

    pub const fn values(&self) -> &'a [StatValue] {
        self.values
    }
}

#[derive(Debug, Clone)]
pub struct TracksView<'a> {
    revision: ComponentRevision,
    catalog_version: &'a CatalogVersion,
    values: &'a [TrackValue],
}

impl<'a> TracksView<'a> {
    pub fn revision(&self) -> &ComponentRevision {
        &self.revision
    }

    pub const fn catalog_version(&self) -> &'a CatalogVersion {
        self.catalog_version
    }

    pub const fn values(&self) -> &'a [TrackValue] {
        self.values
    }
}

#[derive(Debug, Clone)]
pub struct IntrinsicSourcesView<'a> {
    revision: ComponentRevision,
    catalog_version: &'a CatalogVersion,
    bindings: &'a [IntrinsicSourceBinding],
}

impl<'a> IntrinsicSourcesView<'a> {
    pub fn revision(&self) -> &ComponentRevision {
        &self.revision
    }

    pub const fn catalog_version(&self) -> &'a CatalogVersion {
        self.catalog_version
    }

    pub const fn bindings(&self) -> &'a [IntrinsicSourceBinding] {
        self.bindings
    }
}

#[derive(Debug, Clone)]
pub struct MechanicsEntityView<'a> {
    entity: EntityId,
    stats: Option<StatsView<'a>>,
    tracks: Option<TracksView<'a>>,
    intrinsic_sources: Option<IntrinsicSourcesView<'a>>,
}

impl<'a> MechanicsEntityView<'a> {
    pub fn read(state: &'a EntityState, entity: EntityId) -> Result<Self, MechanicsError> {
        if !state.is_alive(entity) {
            return Err(MechanicsError::MissingEntity { entity });
        }
        let stats = state
            .component::<StatsComponent>(entity)?
            .map(|component| -> Result<_, MechanicsError> {
                Ok(StatsView {
                    revision: state.component_revision::<StatsComponent>(entity)?,
                    catalog_version: component.catalog_version(),
                    values: component.values(),
                })
            })
            .transpose()?;
        let tracks = state
            .component::<TracksComponent>(entity)?
            .map(|component| -> Result<_, MechanicsError> {
                Ok(TracksView {
                    revision: state.component_revision::<TracksComponent>(entity)?,
                    catalog_version: component.catalog_version(),
                    values: component.values(),
                })
            })
            .transpose()?;
        let intrinsic_sources = state
            .component::<IntrinsicSourcesComponent>(entity)?
            .map(|component| -> Result<_, MechanicsError> {
                Ok(IntrinsicSourcesView {
                    revision: state.component_revision::<IntrinsicSourcesComponent>(entity)?,
                    catalog_version: component.catalog_version(),
                    bindings: component.bindings(),
                })
            })
            .transpose()?;
        Ok(Self {
            entity,
            stats,
            tracks,
            intrinsic_sources,
        })
    }

    pub const fn entity(&self) -> EntityId {
        self.entity
    }

    pub const fn stats(&self) -> Option<&StatsView<'a>> {
        self.stats.as_ref()
    }

    pub const fn tracks(&self) -> Option<&TracksView<'a>> {
        self.tracks.as_ref()
    }

    pub const fn intrinsic_sources(&self) -> Option<&IntrinsicSourcesView<'a>> {
        self.intrinsic_sources.as_ref()
    }
}
