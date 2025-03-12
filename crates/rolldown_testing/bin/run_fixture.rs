use std::path::PathBuf;

use rolldown_testing::fixture::Fixture;
use rolldown_workspace::root_dir;
fn main() {
  let args = std::env::args().skip(1).collect::<Vec<_>>();
  let Some(arg) = args.first() else {
    panic!(
      "Please provide the _config.json path to a test case. \nusage: cargo run --bin run-fixture <path> \n cargo run-fixture <path>"
    );
  };
  let mut path = PathBuf::from(arg);
  if !path.is_absolute() {
    let workspace_dir = root_dir();
    path = workspace_dir.join(path);
  }
  Fixture::new(path.parent().unwrap()).run_integration_test();
}
