use wax::{Any, BuildError, Glob};

pub fn create_glob(pattern: &str) -> Result<Glob<'static>, BuildError> {
  Glob::new(pattern).map(Glob::into_owned)
}

pub fn create_glob_with_star_prefix(pattern: &str) -> Result<Glob<'static>, BuildError> {
  let pattern =
    if pattern.starts_with("**/") { pattern.to_owned() } else { format!("**/{pattern}") };

  create_glob(&pattern)
}

pub fn create_globset(globs: Vec<Glob>) -> Result<Any, BuildError> {
  wax::any(globs)
}
