use rolldown_testing::{
  bundler_options_presets,
  utils::{assert_bundled, assert_bundled_write},
};

#[test]
fn threejs() {
  assert_bundled(bundler_options_presets::threejs());
}

#[test]
fn threejs10x() {
  assert_bundled(bundler_options_presets::threejs10x());
}

#[test]
fn rome_ts() {
  assert_bundled_write(bundler_options_presets::rome_ts());
  // TODO: verify correctness
  // https://github.com/evanw/esbuild/blob/fc37c2fa9de2ad77476a6d4a8f1516196b90187e/Makefile#L1017-L1023
  // https://github.com/evanw/esbuild/blob/fc37c2fa9de2ad77476a6d4a8f1516196b90187e/Makefile#L1027-L1034
}
