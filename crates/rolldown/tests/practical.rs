use rolldown_testing::{bundler_options_presets, utils::assert_bundled};

#[test]
fn threejs() {
  assert_bundled(bundler_options_presets::threejs());
}

#[test]
fn threejs10x() {
  assert_bundled(bundler_options_presets::threejs10x());
}
