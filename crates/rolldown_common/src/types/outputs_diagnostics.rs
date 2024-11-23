use std::path::PathBuf;

use rolldown_error::BuildDiagnostic;

pub struct OutputsDiagnostics {
  pub diagnostics: Vec<BuildDiagnostic>,
  pub cwd: PathBuf,
}
