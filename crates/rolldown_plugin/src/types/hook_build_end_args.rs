use rolldown_error::BuildError;

#[derive(Debug)]
pub struct HookBuildEndArgs<'a> {
  pub errors: &'a BuildError,
  pub cwd: &'a std::path::PathBuf,
}
