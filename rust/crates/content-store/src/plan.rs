use crate::{
    ArtifactRole, ContentDelete, ContentManifest, ContentMove, ContentWrite, ContentWriteCandidate,
    ManifestError, CONTENT_MANIFEST_PATH,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentLoadStage {
    AssetAuthority,
    AssetData,
    Annotations,
    Prefabs,
    Scenes,
    EntityState,
    Resources,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentLoadStep {
    pub stage: ContentLoadStage,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentLoadPlan {
    pub steps: Vec<ContentLoadStep>,
}

impl ContentLoadPlan {
    pub fn build(manifest: &ContentManifest) -> Result<Self, ManifestError> {
        manifest.validate()?;
        let mut steps: Vec<_> = manifest
            .load_required()
            .map(|artifact| ContentLoadStep {
                stage: stage_for(&artifact.role),
                path: artifact.path.clone(),
            })
            .collect();
        steps.sort_by(|left, right| {
            left.stage
                .cmp(&right.stage)
                .then_with(|| left.path.cmp(&right.path))
        });
        Ok(Self { steps })
    }

    pub fn verify_order(&self) -> bool {
        self.steps.windows(2).all(|pair| {
            (pair[0].stage, pair[0].path.as_str()) <= (pair[1].stage, pair[1].path.as_str())
        })
    }
}

fn stage_for(role: &ArtifactRole) -> ContentLoadStage {
    match role {
        ArtifactRole::AssetCatalog | ArtifactRole::AssetLock => ContentLoadStage::AssetAuthority,
        ArtifactRole::VoxelAsset | ArtifactRole::ImportedAsset => ContentLoadStage::AssetData,
        ArtifactRole::VoxelAnnotation => ContentLoadStage::Annotations,
        ArtifactRole::PrefabRegistry => ContentLoadStage::Prefabs,
        ArtifactRole::SceneDocument => ContentLoadStage::Scenes,
        ArtifactRole::EntityStateSnapshot => ContentLoadStage::EntityState,
        _ => ContentLoadStage::Resources,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSavePlan {
    pub writes: Vec<ContentWrite>,
    pub moves: Vec<ContentMove>,
    pub deletes: Vec<ContentDelete>,
    pub manifest_path: String,
    pub manifest_bytes: Vec<u8>,
}

impl ContentSavePlan {
    pub fn new(
        mut writes: Vec<ContentWrite>,
        mut moves: Vec<ContentMove>,
        mut deletes: Vec<ContentDelete>,
        manifest_path: impl Into<String>,
        manifest_bytes: impl Into<Vec<u8>>,
    ) -> Self {
        writes.sort_by(|left, right| left.path().cmp(right.path()));
        moves.sort_by(|left, right| {
            left.from
                .cmp(&right.from)
                .then_with(|| left.to.cmp(&right.to))
        });
        deletes.sort_by(|left, right| left.path.cmp(&right.path));
        Self {
            writes,
            moves,
            deletes,
            manifest_path: manifest_path.into(),
            manifest_bytes: manifest_bytes.into(),
        }
    }

    pub fn from_candidate(candidate: &ContentWriteCandidate) -> Self {
        Self::new(
            candidate.writes().to_vec(),
            candidate.moves().to_vec(),
            candidate.deletes().to_vec(),
            CONTENT_MANIFEST_PATH,
            candidate.manifest_json().as_bytes(),
        )
    }
}
