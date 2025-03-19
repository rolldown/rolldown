use rolldown_common::{BundlerOptions, InputItem};

use rolldown_workspace::root_dir;

pub fn threejs() -> BundlerOptions {
  BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("threejs".to_string()),
      import: root_dir().join("tmp/bench/three/entry.js").to_str().unwrap().to_string(),
    }]),
    cwd: root_dir().join("tmp/bench/three").into(),
    ..Default::default()
  }
}

pub fn threejs10x() -> BundlerOptions {
  BundlerOptions {
    input: Some(vec![InputItem {
      name: Some("threejs".to_string()),
      import: root_dir().join("tmp/bench/three10x/entry.js").to_str().unwrap().to_string(),
    }]),
    cwd: root_dir().join("tmp/bench/three10x").into(),
    ..Default::default()
  }
}
