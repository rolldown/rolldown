use rolldown_error::BuildDiagnostic;

#[derive(Debug)]
pub struct HookCloseBundleArgs<'a> {
  pub errors: &'a Vec<BuildDiagnostic>,
  pub cwd: &'a std::path::PathBuf,
}
