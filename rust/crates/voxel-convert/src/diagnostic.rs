use voxel_asset::{VoxelConversionInputDiagnostic, VoxelConversionInputError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionDiagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionError {
    diagnostics: Vec<ConversionDiagnostic>,
}

impl ConversionError {
    pub fn one(code: &'static str, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![ConversionDiagnostic {
                code,
                path: path.into(),
                message: message.into(),
            }],
        }
    }

    pub fn diagnostics(&self) -> &[ConversionDiagnostic] {
        &self.diagnostics
    }
}

impl From<VoxelConversionInputError> for ConversionError {
    fn from(error: VoxelConversionInputError) -> Self {
        Self {
            diagnostics: error
                .diagnostics()
                .iter()
                .map(from_input_diagnostic)
                .collect(),
        }
    }
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let first = self
            .diagnostics
            .first()
            .expect("conversion error always has a diagnostic");
        write!(
            formatter,
            "{} at {}: {}",
            first.code, first.path, first.message
        )
    }
}

impl std::error::Error for ConversionError {}

fn from_input_diagnostic(input: &VoxelConversionInputDiagnostic) -> ConversionDiagnostic {
    ConversionDiagnostic {
        code: input.code,
        path: input.path.clone(),
        message: input.message.clone(),
    }
}
