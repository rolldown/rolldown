use rolldown::ModuleType;
use rolldown_common::{BundlerOptions, InputItem};
use rustc_hash::FxHashMap;

use rolldown_workspace::root_dir;

pub fn multi_duplicated_symbol() -> BundlerOptions {
  BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("multi_duplicated_symbol".to_string()),
      import: root_dir()
        .join("tmp/bench/rolldown-benchcases/packages/multi-duplicated-symbols/index.jsx")
        .to_str()
        .unwrap()
        .to_string(),
    }]),
    cwd: Some(root_dir().join("tmp/bench/rolldown-benchcases/packages/multi-duplicated-symbols")),

    module_types: Some(FxHashMap::from_iter([("css".to_string(), ModuleType::Empty)])),
    ..Default::default()
  }
}
