use std::fmt::Write;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
    Fatal,
}

impl DiagnosticSeverity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Fatal => "fatal",
        }
    }

    pub const fn rank(self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Warning => 1,
            Self::Error => 2,
            Self::Fatal => 3,
        }
    }

    pub const fn blocks_load(self) -> bool {
        matches!(self, Self::Fatal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticDomain {
    AssetCatalog,
    EntityState,
    Scene,
    VoxelState,
    Persistence,
    Import,
}

impl DiagnosticDomain {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AssetCatalog => "assetCatalog",
            Self::EntityState => "entityState",
            Self::Scene => "scene",
            Self::VoxelState => "voxelState",
            Self::Persistence => "persistence",
            Self::Import => "import",
        }
    }
}

/// A local authority location, never a cross-runtime source trace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticLocation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_node_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk: Option<[i64; 3]>,
}

impl DiagnosticLocation {
    pub fn path(path: impl Into<String>) -> Self {
        Self {
            path: Some(path.into()),
            ..Self::default()
        }
    }

    pub fn with_asset(mut self, asset_id: impl Into<String>) -> Self {
        self.asset_id = Some(asset_id.into());
        self
    }

    pub const fn with_entity(mut self, entity_id: u64) -> Self {
        self.entity_id = Some(entity_id);
        self
    }

    pub const fn with_scene_node(mut self, scene_node_id: u64) -> Self {
        self.scene_node_id = Some(scene_node_id);
        self
    }

    pub const fn with_chunk(mut self, chunk: [i64; 3]) -> Self {
        self.chunk = Some(chunk);
        self
    }

    fn text(&self) -> String {
        let mut fields = Vec::new();
        if let Some(path) = &self.path {
            fields.push(format!("path={path}"));
        }
        if let Some(asset_id) = &self.asset_id {
            fields.push(format!("asset={asset_id}"));
        }
        if let Some(entity_id) = self.entity_id {
            fields.push(format!("entity={entity_id}"));
        }
        if let Some(scene_node_id) = self.scene_node_id {
            fields.push(format!("sceneNode={scene_node_id}"));
        }
        if let Some(chunk) = self.chunk {
            fields.push(format!("chunk={},{},{}", chunk[0], chunk[1], chunk[2]));
        }
        fields.join(" ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RemedyAction {
    Inspect,
    ProvideAsset,
    FixReference,
    BreakCycle,
    Regenerate,
    RestoreArtifact,
    RefreshCache,
}

impl RemedyAction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Inspect => "inspect",
            Self::ProvideAsset => "provideAsset",
            Self::FixReference => "fixReference",
            Self::BreakCycle => "breakCycle",
            Self::Regenerate => "regenerate",
            Self::RestoreArtifact => "restoreArtifact",
            Self::RefreshCache => "refreshCache",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Remedy {
    pub action: RemedyAction,
    pub detail: String,
}

impl Remedy {
    pub fn new(action: RemedyAction, detail: impl Into<String>) -> Self {
        Self {
            action,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub domain: DiagnosticDomain,
    pub severity: DiagnosticSeverity,
    /// Owner-local stable code. There is intentionally no engine-wide code enum.
    pub code: String,
    pub location: DiagnosticLocation,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remedy: Option<Remedy>,
}

impl Diagnostic {
    pub fn new(
        domain: DiagnosticDomain,
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        location: DiagnosticLocation,
        message: impl Into<String>,
    ) -> Self {
        Self {
            domain,
            severity,
            code: code.into(),
            location,
            message: message.into(),
            remedy: None,
        }
    }

    pub fn with_remedy(mut self, action: RemedyAction, detail: impl Into<String>) -> Self {
        self.remedy = Some(Remedy::new(action, detail));
        self
    }

    pub fn to_text(&self) -> String {
        let mut output = format!(
            "[{}] {} {}",
            self.severity.label(),
            self.domain.label(),
            self.code
        );
        let location = self.location.text();
        if !location.is_empty() {
            output.push(' ');
            output.push_str(&location);
        }
        output.push_str(": ");
        output.push_str(&self.message);
        if let Some(remedy) = &self.remedy {
            let _ = write!(
                output,
                " (remedy={} {})",
                remedy.action.label(),
                remedy.detail
            );
        }
        output
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSet {
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn one(diagnostic: Diagnostic) -> Self {
        Self {
            diagnostics: vec![diagnostic],
        }
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        self.diagnostics.extend(diagnostics);
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn max_severity(&self) -> Option<DiagnosticSeverity> {
        self.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.severity)
            .max()
    }

    pub fn count_at(&self, severity: DiagnosticSeverity) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == severity)
            .count()
    }

    pub fn blocks_load(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity.blocks_load())
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.severity,
                DiagnosticSeverity::Error | DiagnosticSeverity::Fatal
            )
        })
    }

    pub fn to_text(&self) -> String {
        let maximum = self
            .max_severity()
            .map_or("none", DiagnosticSeverity::label);
        let mut output = format!(
            "diagnostics count={} max={} blocksLoad={}\n",
            self.diagnostics.len(),
            maximum,
            self.blocks_load()
        );
        for diagnostic in &self.diagnostics {
            output.push_str(&diagnostic.to_text());
            output.push('\n');
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_and_text_are_stable() {
        let mut diagnostics = DiagnosticSet::new();
        diagnostics.push(
            Diagnostic::new(
                DiagnosticDomain::Scene,
                DiagnosticSeverity::Warning,
                "missing-asset",
                DiagnosticLocation::path("nodes[7]")
                    .with_scene_node(7)
                    .with_asset("mesh/missing"),
                "asset is absent",
            )
            .with_remedy(RemedyAction::ProvideAsset, "add it to the catalog"),
        );
        diagnostics.push(Diagnostic::new(
            DiagnosticDomain::Persistence,
            DiagnosticSeverity::Fatal,
            "manifest.decode",
            DiagnosticLocation::path("$"),
            "malformed JSON",
        ));

        assert_eq!(diagnostics.max_severity(), Some(DiagnosticSeverity::Fatal));
        assert_eq!(diagnostics.count_at(DiagnosticSeverity::Warning), 1);
        assert!(diagnostics.blocks_load());
        assert!(diagnostics.has_errors());
        assert!(diagnostics.to_text().contains(
            "[warning] scene missing-asset path=nodes[7] asset=mesh/missing sceneNode=7"
        ));
    }
}
