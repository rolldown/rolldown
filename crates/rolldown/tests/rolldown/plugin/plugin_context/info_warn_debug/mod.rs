use std::{
  borrow::Cow,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
  },
  time::Duration,
};

use rolldown::{BundlerOptions, InputItem, Log, LogLevel, LogWithoutPlugin, OnLog};
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
  let release = Arc::new(AtomicBool::new(false));
  let release_notify = Arc::new(tokio::sync::Notify::new());
  let (started_tx, started_rx) = mpsc::channel();
  let (done_tx, done_rx) = mpsc::channel();

  let temp = Arc::<std::sync::Mutex<usize>>::clone(&count);
  let callback_release = Arc::clone(&release);
  let callback_release_notify = Arc::clone(&release_notify);
  let on_log = OnLog::new(Arc::new(move |log_level: LogLevel, log: Log| {
    let temp = Arc::<std::sync::Mutex<usize>>::clone(&temp);
    let release = Arc::clone(&callback_release);
    let release_notify = Arc::clone(&callback_release_notify);
    let started_tx = started_tx.clone();
    let done_tx = done_tx.clone();
    Box::pin(async move {
      started_tx.send(()).unwrap();
      while !release.load(Ordering::Acquire) {
        release_notify.notified().await;
      }
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
      done_tx.send(()).unwrap();
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

  for _ in 0..3 {
    started_rx
      .recv_timeout(Duration::from_secs(5))
      .expect("detached native log callback must start after its spawn handle is dropped");
  }
  assert_eq!(*count.lock().unwrap(), 0, "callbacks must remain pending behind the test gate");
  release.store(true, Ordering::Release);
  release_notify.notify_waiters();
  for _ in 0..3 {
    done_rx
      .recv_timeout(Duration::from_secs(5))
      .expect("detached native log callback must finish after the gate opens");
  }
  assert_eq!(*count.lock().unwrap(), 7);
}
