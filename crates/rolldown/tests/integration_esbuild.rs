use std::path::PathBuf;

use rolldown_testing::fixture::Fixture;
use testing_macros::fixture;

#[allow(clippy::needless_pass_by_value)]
#[fixture("./tests/esbuild/**/_config.json")]
fn test(path: PathBuf) {
  Fixture::new(path.parent().unwrap()).run_integration_test();
}
