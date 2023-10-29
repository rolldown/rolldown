mod common;

use std::path::PathBuf;

use common::Case;
use testing_macros::fixture;

#[fixture("./tests/fixtures/**/test.config.json")]
fn fixture_with_config(config_path: PathBuf) {
  Case::new(config_path.parent().unwrap()).run();
}
