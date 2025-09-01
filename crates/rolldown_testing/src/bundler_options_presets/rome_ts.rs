use rolldown_common::{BundlerOptions, InputItem};
use rolldown_workspace::root_dir;

pub fn rome_ts() -> BundlerOptions {
  BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("rome-ts".to_string()),
      import: root_dir().join("tmp/bench/rome/src/entry.ts").to_str().unwrap().to_string(),
    }]),
    cwd: Some(root_dir().join("tmp/bench/rome")),

    // --- Required specific options for Rome
    shim_missing_exports: Some(true), // Need this due rome is not written with `isolatedModules: true`
    tsconfig: Some(
      root_dir().join("tmp/bench/rome/src/tsconfig.json").to_str().unwrap().to_string(),
    ),
    ..Default::default()
  }
}
