mod utils;

use std::{
  borrow::Cow,
  fmt::Write as _,
  path::Path,
  sync::{
    Arc, RwLock,
    atomic::{AtomicBool, AtomicU32, Ordering},
  },
  time::{Duration, Instant},
};

use cow_utils::CowUtils;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use sugar_path::SugarPath;

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct ReporterPlugin {
  assets_dir: String,
  is_lib: bool,
  is_tty: bool,
  should_log_info: bool,
  chunk_limit: usize,
  chunk_count: AtomicU32,
  compressed_count: AtomicU32,
  report_compressed_size: bool,
  has_rendered_chunk: AtomicBool,
  has_transformed: AtomicBool,
  transformed_count: AtomicU32,
  latest_checkpoint: Arc<RwLock<Instant>>,
}

impl ReporterPlugin {
  #[allow(clippy::fn_params_excessive_bools)]
  pub fn new(
    is_tty: bool,
    should_log_info: bool,
    chunk_limit: usize,
    report_compressed_size: bool,
    assets_dir: String,
    is_lib: bool,
  ) -> Self {
    Self {
      assets_dir,
      is_lib,
      is_tty,
      should_log_info,
      chunk_limit,
      chunk_count: AtomicU32::new(0),
      compressed_count: AtomicU32::new(0),
      report_compressed_size,
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
          Path::new(args.id).relative(ctx.cwd()).to_string_lossy()
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
      "\x1b[32m✓\x1b[39m {} modules transformed.",
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

  #[allow(clippy::too_many_lines)]
  async fn write_bundle(
    &self,
    _ctx: &PluginContext,
    args: &mut rolldown_plugin::HookWriteBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    // TODO(shulaoda): support this warning
    // <https://github.com/vitejs/rolldown-vite/blob/9865a3a/packages/vite/src/node/plugins/reporter.ts#L255-L269>
    if self.should_log_info {
      let mut longest = 0;
      let mut biggest_size = 0;
      let mut biggest_map_size = 0;
      let mut biggest_compress_size = 0;

      let mut log_entries = Vec::with_capacity(args.bundle.len());

      if self.report_compressed_size && self.should_log_info {
        if self.is_tty {
          utils::write_line("computing gzip size (0)...");
        } else {
          utils::log_info("computing gzip size...");
        }
      }
      let pre_compute_size = args
        .bundle
        .par_iter()
        .map(|output| {
          if !self.report_compressed_size {
            return None;
          }
          match output {
            rolldown_common::Output::Chunk(chunk) => {
              utils::compute_gzip_size(chunk.code.as_bytes())
            }
            rolldown_common::Output::Asset(asset) => {
              if asset.filename.ends_with(".map") {
                return None;
              }

              let is_css = asset.filename.ends_with(".css");
              let is_compressible =
                is_css || utils::COMPRESSIBLE_ASSETS.iter().any(|s| asset.filename.ends_with(s));
              if is_compressible { utils::compute_gzip_size(asset.source.as_bytes()) } else { None }
            }
          }
        })
        .collect::<Vec<_>>();

      if self.report_compressed_size && self.should_log_info && self.is_tty {
        utils::write_line(&format!(
          "computing gzip size ({})...",
          itoa::Buffer::new().format(pre_compute_size.iter().filter(|s| s.is_some()).count())
        ));
      }
      for (idx, output) in args.bundle.iter().enumerate() {
        let log_entry = match output {
          rolldown_common::Output::Chunk(chunk) => utils::LogEntry {
            name: &chunk.filename,
            size: chunk.code.len(),
            group: utils::AssetGroup::JS,
            map_size: chunk.map.as_ref().map(|m| m.to_json_string().len()),
            compressed_size: pre_compute_size[idx],
          },
          rolldown_common::Output::Asset(asset) => {
            if asset.filename.ends_with(".map") {
              continue;
            }

            let is_css = asset.filename.ends_with(".css");
            let group = if is_css { utils::AssetGroup::Css } else { utils::AssetGroup::Assets };

            utils::LogEntry {
              name: &asset.filename,
              size: asset.source.as_bytes().len(),
              group,
              map_size: None,
              compressed_size: pre_compute_size[idx],
            }
          }
        };

        if log_entry.name.len() > longest {
          longest = log_entry.name.len();
        }
        if log_entry.size > biggest_size {
          biggest_size = log_entry.size;
        }
        if let Some(size) = log_entry.map_size {
          if size > biggest_map_size {
            biggest_map_size = size;
          }
        }
        if let Some(size) = log_entry.compressed_size {
          if size > biggest_compress_size {
            biggest_compress_size = size;
          }
        }

        log_entries.push(log_entry);
      }

      if self.is_tty {
        let _ = utils::clear_line();
      }

      let size_pad = utils::display_size(biggest_size).len();
      let map_pad = utils::display_size(biggest_map_size).len();
      let compress_pad = utils::display_size(biggest_compress_size).len();

      let out_dir =
        args.options.cwd.join(&args.options.out_dir).normalize().relative(&args.options.cwd);
      let out_dir = out_dir.to_slash_lossy();

      for group in utils::GROUPS {
        let mut filtered = log_entries.iter().filter(|e| e.group == group).collect::<Vec<_>>();
        if filtered.is_empty() {
          continue;
        }
        filtered.sort_by(|a, b| a.size.cmp(&b.size));
        for log_entry in filtered {
          let mut info = String::new();
          let _ = write!(&mut info, "\x1b[2m{out_dir}/\x1b[22m");

          let is_asset = !self.is_lib && Path::new(log_entry.name).starts_with(&self.assets_dir);
          if is_asset {
            let _ = write!(&mut info, "\x1b[2m{}\x1b[22m", &self.assets_dir.cow_replace('\\', "/"));
          }

          let name = if is_asset {
            let dir_len = self.assets_dir.len();
            format!("{:pad$}", &log_entry.name[dir_len..], pad = longest + 2 - dir_len)
          } else {
            format!("{:pad$}", log_entry.name, pad = longest + 2)
          };

          let _ = match group {
            utils::AssetGroup::JS => write!(&mut info, "\x1b[36m{name}\x1b[39m"),
            utils::AssetGroup::Css => write!(&mut info, "\x1b[35m{name}\x1b[39m"),
            utils::AssetGroup::Assets => write!(&mut info, "\x1b[32m{name}\x1b[39m"),
          };

          let size = utils::display_size(log_entry.size);
          if group == utils::AssetGroup::JS && log_entry.size.div_ceil(1000) > self.chunk_limit {
            let _ = write!(&mut info, "\x1b[1m\x1b[33m{size:>size_pad$}\x1b[39m\x1b[22m");
          } else {
            let _ = write!(&mut info, "\x1b[1m\x1b[2m{size:>size_pad$}\x1b[22m\x1b[22m");
          }

          if let Some(compressed_size) = log_entry.compressed_size {
            let size = utils::display_size(compressed_size);
            let _ = write!(&mut info, "\x1b[2m │ gzip: {size:>compress_pad$}\x1b[22m");
          }

          if let Some(map_size) = log_entry.map_size {
            let size = utils::display_size(map_size);
            let _ = write!(&mut info, "\x1b[2m │ map: {size:>map_pad$}\x1b[22m");
          }

          utils::log_info(&info);
        }
      }
    }
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
