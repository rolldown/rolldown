use std::path::PathBuf;

/// Get the absolute path to the root of the workspace/repository.
/// The root is always the directory containing the root `Cargo.toml`, `package.json`, `pnpm-workspace.yaml` etc.
pub fn root_dir() -> PathBuf {
  PathBuf::from(env!("WORKSPACE_DIR"))
}

pub fn crate_dir(crate_name: &str) -> PathBuf {
  let root = root_dir();
  root.join("crates").join(crate_name)
}
