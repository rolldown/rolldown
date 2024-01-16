mod common;

use std::path::PathBuf;

use common::Case;
use testing_macros::fixture;

#[allow(clippy::needless_pass_by_value)]
#[fixture("./tests/fixtures/**/test.config.json")]
fn fixture_with_config(config_path: PathBuf) {
  Case::new(config_path.parent().unwrap()).run();
}
