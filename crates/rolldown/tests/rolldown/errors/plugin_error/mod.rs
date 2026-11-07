use std::{borrow::Cow, path::PathBuf, time::Duration};

use rolldown::{Bundler, BundlerOptions, InputItem};
use rolldown_error::{BatchedBuildDiagnostic, EventKind};
use rolldown_plugin::{
  HookLoadArgs, HookLoadReturn, HookUsage, Plugin, PluginContext, SharedLoadPluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct PluginErrorTest;

impl Plugin for PluginErrorTest {
  fn name(&self) -> Cow<'static, str> {
    "plugin-error-test".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    Err(BatchedBuildDiagnostic::new(vec![
      anyhow::anyhow!("A").into(),
      anyhow::anyhow!("B").into(),
    ]))?
  }
}

#[derive(Debug)]
struct PanicOnLoad;

impl Plugin for PanicOnLoad {
  fn name(&self) -> Cow<'static, str> {
    "panic-on-load".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, _args: &HookLoadArgs<'_>) -> HookLoadReturn {
    panic!("intentional module task panic");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn should_emit_multi_plugin_diagnostics() {
  manual_integration_test!()
    .build(TestMeta { expect_error: true, ..Default::default() })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![Plugin::new_shared(PluginErrorTest)],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn module_task_panic_returns_build_error() {
  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      cwd: Some(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/rolldown/errors/plugin_error"
      ))),
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      ..Default::default()
    },
    vec![Plugin::new_shared(PanicOnLoad)],
  )
  .expect("failed to create bundler");

  let result = tokio::time::timeout(Duration::from_secs(5), bundler.generate())
    .await
    .expect("a panicking module task must not hang the build");
  let Err(errors) = result else {
    panic!("a panicking module task must fail the build");
  };
  assert_eq!(errors.len(), 1);
  assert!(matches!(errors[0].kind(), EventKind::UnhandleableError));
  let diagnostic = errors[0].to_string();
  assert!(
    diagnostic.contains("module loader task panicked before reporting completion"),
    "unexpected diagnostic: {diagnostic}"
  );
  assert!(
    !diagnostic.contains("intentional module task panic"),
    "the diagnostic must not expose the panic payload: {diagnostic}"
  );
}
