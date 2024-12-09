use rolldown_error::BuildDiagnostic;

#[derive(Debug)]
pub struct HookBuildEndArgs<'a> {
  pub errors: &'a Vec<BuildDiagnostic>,
  pub cwd: &'a std::path::PathBuf,
}
