use std::path::PathBuf;

use rolldown_plugin::Pluginable;
use rolldown_plugin_isolated_declaration::IsolatedDeclarationPlugin;
use rolldown_testing::fixture::Fixture;
use testing_macros::fixture;

#[expect(clippy::needless_pass_by_value)]
#[fixture("./tests/**/_config.json")]
fn fixture_with_config(config_path: PathBuf) {
  Fixture::new(config_path.parent().unwrap()).run_integration_test_with_plugins(vec![
    Pluginable::new_shared(IsolatedDeclarationPlugin { strip_internal: true }),
  ]);
}
