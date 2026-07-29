use std::{
  borrow::Cow,
  sync::{Arc, Mutex},
};

use rolldown::{Bundler, BundlerOptions, InputItem, Log, LogLevel, LogWithoutPlugin, OnLog};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct TestPlugin;

impl Plugin for TestPlugin {
  fn name(&self) -> Cow<'static, str> {
    "TestPlugin".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    ctx.info(LogWithoutPlugin { message: "info".to_owned(), ..Default::default() });
    ctx.warn(LogWithoutPlugin { message: "warn".to_owned(), ..Default::default() });
    ctx.debug(LogWithoutPlugin { message: "debug".to_owned(), ..Default::default() });
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn allow_pass_custom_arg() {
  let count = Arc::new(Mutex::new(0_usize));

  let temp = Arc::<std::sync::Mutex<usize>>::clone(&count);
  let on_log = OnLog::new(Arc::new(move |log_level: LogLevel, log: Log| {
    let temp = Arc::<std::sync::Mutex<usize>>::clone(&temp);
    Box::pin(async move {
      let mut guard = temp.lock().unwrap();
      if log.plugin.is_none_or(|p| p != "TestPlugin") {
        return Ok(());
      }
      match log_level {
        LogLevel::Info if log.message == "info" => *guard ^= 1 << 0,
        LogLevel::Warn if log.message == "warn" => *guard ^= 1 << 1,
        LogLevel::Debug if log.message == "debug" => *guard ^= 1 << 2,
        _ => unreachable!(),
      }
      Ok(())
    })
  }));

  manual_integration_test!()
    .build(TestMeta {
      snapshot: false,
      write_to_disk: false,
      expect_executed: false,
      ..Default::default()
    })
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem {
          name: Some("entry".to_string()),
          import: "./entry.js".to_string(),
        }]),
        on_log: Some(on_log),
        ..Default::default()
      },
      vec![Arc::new(TestPlugin)],
    )
    .await;

  assert_eq!(*count.lock().unwrap(), 7);
}

// A throwing `onLog` callback should fail the build (the callback runs as a
// detached task that the build awaits at a barrier)
#[tokio::test(flavor = "multi_thread")]
async fn on_log_error_fails_build() {
  let on_log = OnLog::new(Arc::new(move |_log_level: LogLevel, log: Log| {
    Box::pin(async move {
      if log.plugin.as_deref() == Some("TestPlugin") {
        anyhow::bail!("boom from onLog");
      }
      Ok(())
    })
  }));

  let cwd = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests/rolldown/plugin/plugin_context/info_warn_debug");

  let mut bundler = Bundler::with_plugins(
    BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      cwd: Some(cwd),
      on_log: Some(on_log),
      ..Default::default()
    },
    vec![Arc::new(TestPlugin)],
  )
  .expect("failed to create bundler");

  let Err(err) = bundler.generate().await else {
    panic!("expected the build to fail when onLog errors");
  };
  assert!(err.to_string().contains("boom from onLog"), "unexpected error: {err}");
}
