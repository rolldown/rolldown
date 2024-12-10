use rolldown_error::BuildDiagnostic;

#[derive(Debug)]
pub struct HookRenderErrorArgs<'a> {
  pub errors: &'a Vec<BuildDiagnostic>,
  pub cwd: &'a std::path::PathBuf,
}
