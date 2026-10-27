//! What a product's own setup screen asks of the machine it runs on: the
//! platform's folder picker, and runs of the tools the product ships beside
//! it. Each runs on its own thread and never calls its owner; owners read a
//! snapshot whenever they choose.

mod folder;
mod tool;

pub use folder::{FolderChoice, FolderChoiceState};
pub use tool::{ToolRun, ToolRunSnapshot, ToolRunState, OUTPUT_LINES};
