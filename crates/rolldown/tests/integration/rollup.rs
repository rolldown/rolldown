use std::path::PathBuf;

use rolldown_testing::fixture::Fixture;
use testing_macros::fixture;

#[expect(clippy::needless_pass_by_value)]
#[fixture("./tests/rollup/**/_config.json")]
fn test(path: PathBuf) {
  Fixture::new(path.parent().unwrap()).run_integration_test();
}
