use rolldown_error::BuildError;

#[derive(Debug)]
pub struct HookRenderErrorArgs<'a> {
  pub errors: &'a BuildError,
  pub cwd: &'a std::path::PathBuf,
}
