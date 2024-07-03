use std::collections::HashMap;

use rolldown::ModuleType;
use rolldown_common::{BundlerOptions, InputItem, ResolveOptions};

use crate::workspace::root_dir;

pub fn multi_duplicated_symbol() -> BundlerOptions {
  BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("multi_duplicated_symbol".to_string()),
      import: root_dir()
        .join("tmp/bench/multi-duplicated-symbol/src/index.jsx")
        .to_str()
        .unwrap()
        .to_string(),
    }]),
    cwd: Some(root_dir().join("tmp/bench/multi-duplicated-symbol")),

    module_types: Some(HashMap::from_iter([("css".to_string(), ModuleType::Empty)])),
    ..Default::default()
  }
}
