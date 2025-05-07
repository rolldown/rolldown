mod utils;

use std::{
  borrow::Cow,
  io::Write,
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
  latest_checkpoint: Arc<RwLock<Instant>>,
  count: AtomicU32,
  has_transformed: AtomicBool,
  is_tty: bool,
}

impl ReporterPlugin {
  pub fn new(is_tty: bool) -> Self {
    Self {
      latest_checkpoint: Arc::new(RwLock::new(Instant::now())),
      count: AtomicU32::new(0),
      has_transformed: AtomicBool::new(false),
      is_tty,
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
    let count = self.count.fetch_add(1, Ordering::SeqCst);
    let now = Instant::now();
    let duration = {
      let latest_checkpoint = self.latest_checkpoint.read().unwrap();
      now.duration_since(*latest_checkpoint)
    };

    if duration > Duration::from_millis(100) {
      if self.is_tty {
        if args.id.contains('?') {
          return Ok(None);
        }
        let relative_path = Path::new(args.id).relative(ctx.inner.cwd());
        // fetch_add return previous value
        utils::write_line(&format!(
          "transforming ({}) {}",
          count + 1,
          relative_path.to_string_lossy()
        ));
      } else if !self.has_transformed.load(Ordering::Relaxed) {
        utils::write_line("transforming...");
      }
      *self.latest_checkpoint.write().unwrap() = now;
    }
    Ok(None)
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    self.count.store(0, Ordering::SeqCst);
    Ok(())
  }

  #[allow(clippy::print_stdout)]
  #[allow(clippy::write_literal)]
  async fn build_end(
    &self,
    _ctx: &PluginContext,
    _args: Option<&rolldown_plugin::HookBuildEndArgs<'_>>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.is_tty {
      utils::write_line("");
    }
    let count = self.count.load(Ordering::SeqCst);

    let mut lock = utils::clear_current_line();
    // print text with green fg color
    write!(&mut lock, "\x1b[32m{}\x1b[0m", "✓").unwrap();
    writeln!(&mut lock, " {count} modules transformed.",).unwrap();
    lock.flush().unwrap();
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::BuildStart | HookUsage::BuildEnd
  }
}
