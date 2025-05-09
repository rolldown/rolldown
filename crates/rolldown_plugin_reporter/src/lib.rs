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
  #[allow(dead_code)]
  chunk_limit: bool,
  chunk_count: AtomicU32,
  compressed_count: AtomicU32,
  #[allow(dead_code)]
  has_compress_chunk: AtomicBool,
  has_rendered_chunk: AtomicBool,
  has_transformed: AtomicBool,
  transformed_count: AtomicU32,
  latest_checkpoint: Arc<RwLock<Instant>>,
}

impl ReporterPlugin {
  pub fn new(is_tty: bool, should_log_info: bool, chunk_limit: bool) -> Self {
    Self {
      is_tty,
      should_log_info,
      chunk_limit,
      chunk_count: AtomicU32::new(0),
      compressed_count: AtomicU32::new(0),
      has_compress_chunk: AtomicBool::new(false),
      has_rendered_chunk: AtomicBool::new(false),
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

    self.has_transformed.store(true, Ordering::Release);

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

  async fn render_start(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookRenderStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    self.chunk_count.store(0, Ordering::SeqCst);
    self.compressed_count.store(0, Ordering::SeqCst);
    Ok(())
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    // TODO(shulaoda): dynamic importer warning
    // <https://github.com/vitejs/rolldown-vite/blob/9865a3a/packages/vite/src/node/plugins/reporter.ts#L300-L328>
    let chunk_count = self.chunk_count.fetch_add(1, Ordering::SeqCst);
    if self.should_log_info {
      if self.is_tty {
        utils::write_line(&format!(
          "rendering chunks ({})...",
          itoa::Buffer::new().format(chunk_count)
        ));
      } else if !self.has_rendered_chunk.load(Ordering::Relaxed) {
        utils::log_info("rendering chunks...");
      }
      self.has_rendered_chunk.store(true, Ordering::Release);
    }
    Ok(None)
  }

  async fn write_bundle(
    &self,
    _ctx: &PluginContext,
    _args: &mut rolldown_plugin::HookWriteBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    // TODO(shulaoda): support this warning
    // <https://github.com/vitejs/rolldown-vite/blob/9865a3a/packages/vite/src/node/plugins/reporter.ts#L255-L269>
    Ok(())
  }

  async fn generate_bundle(
    &self,
    _ctx: &PluginContext,
    _args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    let _ = utils::clear_line();
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    let hook_usage = HookUsage::RenderStart | HookUsage::RenderChunk | HookUsage::WriteBundle;
    if self.should_log_info {
      let usage = hook_usage | HookUsage::Transform | HookUsage::BuildStart | HookUsage::BuildEnd;
      if self.is_tty { usage | HookUsage::GenerateBundle } else { usage }
    } else {
      hook_usage
    }
  }
}
