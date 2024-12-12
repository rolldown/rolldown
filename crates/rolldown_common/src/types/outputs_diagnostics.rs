use std::path::PathBuf;

use rolldown_error::BuildDiagnostic;

#[derive(Debug)]
pub struct OutputsDiagnostics {
  pub diagnostics: Vec<BuildDiagnostic>,
  pub cwd: PathBuf,
}
