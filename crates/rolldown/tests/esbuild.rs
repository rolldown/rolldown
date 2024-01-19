mod common;

use std::path::PathBuf;

use common::Case;
use testing_macros::fixture;

#[allow(clippy::needless_pass_by_value)]
#[fixture("./tests/esbuild/**/test.config.json")]
fn test(path: PathBuf) {
  Case::new(path.parent().unwrap()).run();
}
