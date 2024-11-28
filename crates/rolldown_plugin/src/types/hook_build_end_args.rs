use std::sync::Arc;

use rolldown_error::BuildDiagnostic;

#[derive(Debug)]
pub struct HookBuildEndArgs<'a> {
  pub errors: Arc<Vec<BuildDiagnostic>>,
  pub cwd: &'a std::path::PathBuf,
}
