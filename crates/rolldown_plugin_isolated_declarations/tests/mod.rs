use rolldown_plugin_isolated_declarations::IsolatedDeclarationPlugin;
use rolldown_testing::fixture::Fixture;
use std::{path::PathBuf, sync::Arc};
use testing_macros::fixture;

#[allow(clippy::needless_pass_by_value)]
#[fixture("./tests/**/_config.json")]
fn fixture_with_config(config_path: PathBuf) {
  Fixture::new(config_path.parent().unwrap())
    .run_integration_test_with_plugins(vec![Arc::new(IsolatedDeclarationPlugin::new(true))]);
}
