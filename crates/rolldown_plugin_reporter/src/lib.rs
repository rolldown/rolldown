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
use owo_colors::OwoColorize;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_plugin_utils::is_in_node_modules;
use sugar_path::SugarPath as _;

#[derive(Debug)]
#[expect(clippy::struct_excessive_bools)]
pub struct ReporterPlugin {
  assets_dir: String,
  is_lib: bool,
  is_tty: bool,
  warn_large_chunks: bool,
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
  #[expect(clippy::fn_params_excessive_bools)]
  pub fn new(
    is_tty: bool,
    should_log_info: bool,
    chunk_limit: usize,
    report_compressed_size: bool,
    assets_dir: String,
    is_lib: bool,
    warn_large_chunks: bool,
  ) -> Self {
    Self {
      assets_dir,
      is_lib,
      is_tty,
      should_log_info,
      warn_large_chunks,
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
          "transforming ({}) {}",
          itoa::Buffer::new().format(transformed_count),
          Path::new(args.id).relative(ctx.cwd()).to_string_lossy().dimmed()
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
      "{} {} modules transformed.",
      "✓".green(),
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
    ctx: &PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    // TODO(shulaoda): Consider moving the following logic into core
    if !args.options.inline_dynamic_imports {
      for id in &args.chunk.module_ids {
        let Some(module) = ctx.get_module_info(id) else {
          continue;
        };
        // When a dynamic importer shares a chunk with the imported module,
        // warn that the dynamic imported module will not be moved to another chunk (#12850).
        if !module.importers.is_empty() && !module.dynamic_importers.is_empty() {
          // Filter out the intersection of dynamic importers and sibling modules in
          // the same chunk. The intersecting dynamic importers' dynamic import is not
          // expected to work. Note we're only detecting the direct ineffective dynamic import here.
          let detected_ineffective_dynamic_import = module
            .dynamic_importers
            .iter()
            .any(|id| !is_in_node_modules(id.as_path()) && args.chunk.module_ids.contains(id));
          if detected_ineffective_dynamic_import {
            let message = format!(
              "\n(!) {} is dynamically imported by {} but also statically imported by {}, dynamic import will not move module into another chunk.\n",
              module.id.as_ref(),
              module
                .dynamic_importers
                .iter()
                .map(std::convert::AsRef::as_ref)
                .collect::<Vec<_>>()
                .join(", "),
              module
                .importers
                .iter()
                .map(std::convert::AsRef::as_ref)
                .collect::<Vec<_>>()
                .join(", "),
            );
            ctx.warn(rolldown_common::LogWithoutPlugin { message, ..Default::default() });
          }
        }
      }
    }

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
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookWriteBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    let mut has_large_chunks = false;
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
          let _ = write!(&mut info, "{}/", out_dir.dimmed());

          let is_asset = !self.is_lib && Path::new(log_entry.name).starts_with(&self.assets_dir);
          if is_asset {
            let _ = write!(&mut info, "{}", self.assets_dir.cow_replace('\\', "/").dimmed());
          }

          let name = if is_asset {
            let dir_len = self.assets_dir.len();
            format!("{:pad$}", &log_entry.name[dir_len..], pad = longest + 2 - dir_len)
          } else {
            format!("{:pad$}", log_entry.name, pad = longest + 2)
          };

          let _ = match group {
            utils::AssetGroup::JS => write!(&mut info, "{}", name.cyan()),
            utils::AssetGroup::Css => write!(&mut info, "{}", name.magenta()),
            utils::AssetGroup::Assets => write!(&mut info, "{}", name.green()),
          };

          let size = utils::display_size(log_entry.size);
          if group == utils::AssetGroup::JS && log_entry.size.div_ceil(1000) > self.chunk_limit {
            has_large_chunks = true;
            let _ = write!(&mut info, "{:>size_pad$}", size.bold().yellow());
          } else {
            let _ = write!(&mut info, "{:>size_pad$}", size.bold().dimmed());
          }

          if let Some(compressed_size) = log_entry.compressed_size {
            let size = utils::display_size(compressed_size);
            let _ = write!(&mut info, " │ gzip: {:>compress_pad$}", size.dimmed());
          }

          if let Some(map_size) = log_entry.map_size {
            let size = utils::display_size(map_size);
            let _ = write!(&mut info, " │ map: {:>map_pad$}", size.dimmed());
          }

          utils::log_info(&info);
        }
      }
    } else if self.warn_large_chunks {
      has_large_chunks = args.bundle.iter().any(|output| {
        if let rolldown_common::Output::Chunk(chunk) = output {
          chunk.code.len().div_ceil(1000) > self.chunk_limit
        } else {
          false
        }
      });
    }
    if self.warn_large_chunks && has_large_chunks {
      let message = format!(
        "\n(!) Some chunks are larger than {} kB after minification. Consider:\n- Using dynamic import() to code-split the application\n- Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks\n- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.",
        itoa::Buffer::new().format(self.chunk_limit).bold().yellow(),
      );
      ctx.warn(rolldown_common::LogWithoutPlugin { message, ..Default::default() });
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
