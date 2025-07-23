use std::{
  borrow::Cow,
  sync::{Arc, Mutex},
};

use rolldown::{BundlerOptions, InputItem, Log, LogLevel, OnLog};
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
    ctx.info(Log { message: "info".to_owned(), code: None, id: None, exporter: None });
    ctx.warn(Log { message: "warn".to_owned(), code: None, id: None, exporter: None });
    ctx.debug(Log { message: "debug".to_owned(), code: None, id: None, exporter: None });
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
