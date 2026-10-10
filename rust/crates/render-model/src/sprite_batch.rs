//! Sprite batches: one retained node draws many camera-facing sprites from one
//! atlas.
//!
//! A batch is static scatter such as vegetation billboards or a crowd far off:
//! each instance has its own position in the batch node's space, scale and
//! atlas frame, and the batch is created, shown and released as one node. The
//! instances are rows of the node, not nodes: they are never picked, moved or
//! animated one by one.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{SpriteError, SpriteInstanceDescriptor, SpriteSizeMode};

/// The most instances one batch may hold.
pub const MAX_SPRITE_BATCH_INSTANCES: usize = 65_536;

/// One sprite of a batch, in the batch node's space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteBatchInstance {
    pub position: [f32; 3],
    /// Multiplies the frame's size; positive.
    pub scale: f32,
    pub frame: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteBatchDescriptor {
    /// What every instance shares: atlas, pivot, the size of a frame with no
    /// size of its own, billboard, tint, render order, depth, layer, shading
    /// and material, and the batch node's transform, visibility and
    /// metadata. Its `frame` is not used: each instance names its own. Sizes
    /// are in world units, and a batch is not placed in the viewport or
    /// attached.
    pub sprite: SpriteInstanceDescriptor,
    /// Shared, so copies of the batch do not copy its instances.
    pub instances: Arc<[SpriteBatchInstance]>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpriteBatchError {
    Sprite(SpriteError),
    /// Pixel sizes, viewport placement and attachment are per sprite.
    UnsupportedSprite,
    TooManyInstances {
        instances: usize,
        limit: usize,
    },
    InvalidInstance {
        index: usize,
    },
}

impl SpriteBatchDescriptor {
    pub fn validate(&self) -> Result<(), SpriteBatchError> {
        self.sprite.validate().map_err(SpriteBatchError::Sprite)?;
        if self.sprite.size_mode != SpriteSizeMode::World
            || self.sprite.viewport_placement.is_some()
            || self.sprite.attachment != Default::default()
        {
            return Err(SpriteBatchError::UnsupportedSprite);
        }
        if self.instances.len() > MAX_SPRITE_BATCH_INSTANCES {
            return Err(SpriteBatchError::TooManyInstances {
                instances: self.instances.len(),
                limit: MAX_SPRITE_BATCH_INSTANCES,
            });
        }
        if let Some(index) = self.instances.iter().position(|instance| {
            !(instance.position.iter().all(|value| value.is_finite())
                && instance.scale.is_finite()
                && instance.scale > 0.0)
        }) {
            return Err(SpriteBatchError::InvalidInstance { index });
        }
        Ok(())
    }
}
