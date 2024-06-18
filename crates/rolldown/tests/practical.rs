use rolldown_testing::{bundler_options_presets, utils::assert_bundled};

#[test]
#[ignore]
fn threejs() {
  assert_bundled(bundler_options_presets::threejs());
}

#[test]
#[ignore]
fn threejs10x() {
  assert_bundled(bundler_options_presets::threejs10x());
}
