mod utils;

use std::{
  borrow::Cow,
  path::Path,
  sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicU32, Ordering},
  },
  time::{Duration, Instant},
};

use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use sugar_path::SugarPath;

#[derive(Debug)]
pub struct ReporterPlugin {
  is_tty: bool,
  should_log_info: bool,
  has_transformed: AtomicBool,
  transformed_count: AtomicU32,
  latest_checkpoint: Arc<RwLock<Instant>>,
}

impl ReporterPlugin {
  pub fn new(is_tty: bool, should_log_info: bool) -> Self {
    Self {
      is_tty,
      should_log_info,
      has_transformed: AtomicBool::new(false),
      transformed_count: AtomicU32::new(0),
      latest_checkpoint: Arc::new(RwLock::new(Instant::now())),
    }
  }
}

impl Plugin for ReporterPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:reporter")
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let transformed_count = self.transformed_count.fetch_add(1, Ordering::SeqCst);

    if self.is_tty {
      if args.id.contains('?') {
        return Ok(None);
      }
      let now = Instant::now();
      let duration = now.duration_since(*self.latest_checkpoint.read().unwrap());

      if duration > Duration::from_millis(100) {
        utils::write_line(&format!(
          "transforming ({}) \x1b[2m{}\x1b[22m",
          itoa::Buffer::new().format(transformed_count),
          Path::new(args.id).relative(ctx.inner.cwd()).to_string_lossy()
        ));

        *self.latest_checkpoint.write().unwrap() = now;
      }
    } else if !self.has_transformed.load(Ordering::Relaxed) {
      utils::write_line("transforming...");
    }

    Ok(None)
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    self.transformed_count.store(0, Ordering::SeqCst);
    Ok(())
  }

  async fn build_end(
    &self,
    _ctx: &PluginContext,
    _args: Option<&rolldown_plugin::HookBuildEndArgs<'_>>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.is_tty {
      let _ = utils::clear_line();
    }

    utils::log_info(&format!(
      "\x1b[32m✓\x1b[39m {} modules transformed.\n",
      self.transformed_count.load(Ordering::SeqCst)
    ));

    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    if self.should_log_info {
      HookUsage::Transform | HookUsage::BuildStart | HookUsage::BuildEnd
    } else {
      HookUsage::empty()
    }
  }
}
