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

#[test]
fn test_root_dir() {
  let root_dir = root_dir();
  assert!(
    root_dir.join("pnpm-workspace.yaml").exists(),
    "Incorrect root directory detected. Expected to find `pnpm-workspace.yaml` in the root directory. But got {root_dir:?}"
  );
}
