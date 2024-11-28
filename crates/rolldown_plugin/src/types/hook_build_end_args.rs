use rolldown_error::BuildDiagnostic;
use rolldown_utils::unique_arc::WeakRef;

#[derive(Debug)]
pub struct HookBuildEndArgs<'a> {
  pub errors: WeakRef<Vec<BuildDiagnostic>>,
  pub cwd: &'a std::path::PathBuf,
}
