use std::path::PathBuf;

use sugar_path::SugarPath;

/// Get the absolute path to the root of the workspace/repository.
/// The root is always the directory containing the root `Cargo.toml`, `package.json`, `pnpm-workspace.yaml` etc.
pub fn root_dir() -> PathBuf {
  let root_dir;

  if let Ok(cargo_manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
    // cargo_manifest_dir = ${WORKSPACE_ROOT}/crates/rolldown_testing
    root_dir = cargo_manifest_dir.as_path().parent().unwrap().parent().unwrap().to_path_buf();
  } else {
    // We will hit this branch if we are not running rolldown in a cargo environment. For examples:
    // - Executing the compiled binary directly not via cargo
    //
    // In this case, we could only take a guess and ensure the correctness by checking for the existence of the `pnpm-workspace.yaml` file.
    let this_file = file!();
    // this_file = crates/rolldown_testing/src/workspace.rs
    root_dir = this_file
      .absolutize()
      .parent()
      .unwrap() // crates/rolldown_testing/src
      .parent()
      .unwrap() // crates/rolldown_testing
      .parent()
      .unwrap() // crates
      .parent()
      .unwrap() // ${WORKSPACE_ROOT}
      .to_path_buf();

    // Check if the root directory contains the `pnpm-workspace.yaml` file
    assert!(root_dir.join("pnpm-workspace.yaml").exists(), "Incorrect root directory detected. Expected to find `pnpm-workspace.yaml` in the root directory. But got {root_dir:?}");
  };
  root_dir
}

pub fn crate_dir(crate_name: &str) -> PathBuf {
  let root = root_dir();
  root.join("crates").join(crate_name)
}
