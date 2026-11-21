mod utils;

use std::{
  borrow::Cow,
  fmt::Write as _,
  path::{Path, PathBuf},
  pin::Pin,
  sync::{
    Arc, RwLock,
    atomic::{AtomicU32, Ordering},
  },
  time::{Duration, Instant},
};

use cow_utils::CowUtils;
use owo_colors::Style;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rolldown_common::{ModuleInfo, NormalModule, SourceMapType};
use rolldown_plugin::{HookUsage, Plugin, PluginContext, PluginHookMeta};
use rolldown_std_utils::relative_path_to_slash;
use rolldown_utils::dashmap::FxDashSet;
use rustc_hash::FxHashMap;
use sugar_path::SugarPath as _;

pub type LogInfoFn =
  dyn Fn(String) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> + Send + Sync;

/// The key of the mark that `transform` sets in the module meta. The key is the name of the
/// plugin.
const TRANSFORMED_MARK: &str = "builtin:vite-reporter";

#[derive(derive_more::Debug)]
#[expect(clippy::struct_excessive_bools)]
pub struct ViteReporterPlugin {
  pub root: PathBuf,
  pub assets_dir: String,
  pub is_lib: bool,
  pub is_tty: bool,
  pub warn_large_chunks: bool,
  pub chunk_limit: usize,
  pub report_compressed_size: bool,
  pub chunk_count: AtomicU32,
  /// The modules that the progress line has counted in this build. The number that `build_end`
  /// prints does not come from here: see `transformed_module_count`.
  pub transformed_count: AtomicU32,
  /// The modules that `transform` reported in this build.
  pub reported_in_transform: FxDashSet<String>,
  pub latest_checkpoint: Arc<RwLock<Instant>>,
  #[debug(skip)]
  pub log_info: Option<Arc<LogInfoFn>>,
}

impl ViteReporterPlugin {
  /// The number of modules of this build that have the mark of `transform`.
  ///
  /// It does not count the `transform` calls: a module that reuses its saved result from the
  /// persistent cache runs no `transform`, but Rolldown sets its mark again.
  pub fn transformed_module_count(ctx: &PluginContext) -> usize {
    ctx.count_modules_with_meta(TRANSFORMED_MARK)
  }

  fn report_progress(&self, id: &str) {
    let transformed_count = self.transformed_count.fetch_add(1, Ordering::SeqCst);
    if self.is_tty {
      if id.contains('?') {
        return;
      }
      let now = Instant::now();
      let duration = now.duration_since(*self.latest_checkpoint.read().unwrap());

      if duration > Duration::from_millis(100) {
        utils::write_line(&format!(
          "transforming ({}) {}",
          itoa::Buffer::new().format(transformed_count + 1),
          utils::paint(Path::new(id).relative(&self.root).to_string_lossy(), Style::new().dimmed())
        ));

        *self.latest_checkpoint.write().unwrap() = now;
      }
    } else if transformed_count == 0 {
      utils::log_info("transforming...");
    }
  }
}

impl Plugin for ViteReporterPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-reporter")
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    ctx.set_module_meta(args.id, TRANSFORMED_MARK, [1u8]);
    self.reported_in_transform.insert(args.id.to_string());
    self.report_progress(args.id);
    Ok(None)
  }

  // Skipping `transform` loses nothing: the mark is module meta, so Rolldown saves it and sets
  // it again for a module that reuses its saved result.
  fn transform_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: None, cache: Some(true) })
  }

  // `transform` reports progress, because Rolldown runs no `module_parsed` for the runtime
  // module. This hook reports only a module that reused its saved result, since such a module
  // runs no `transform`. The bundler keeps the meta of earlier builds, so a mark from an earlier
  // build must not count.
  async fn module_parsed(
    &self,
    ctx: &PluginContext,
    module_info: Arc<ModuleInfo>,
    _normal_module: &NormalModule,
  ) -> rolldown_plugin::HookNoopReturn {
    if ctx.has_module_meta_in_this_build(&module_info.id, TRANSFORMED_MARK)
      && !self.reported_in_transform.contains(module_info.id.as_str())
    {
      self.report_progress(&module_info.id);
    }
    Ok(())
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    self.chunk_count.store(0, Ordering::SeqCst);
    self.transformed_count.store(0, Ordering::SeqCst);
    self.reported_in_transform.clear();
    Ok(())
  }

  async fn build_end(
    &self,
    ctx: &PluginContext,
    _args: Option<&rolldown_plugin::HookBuildEndArgs<'_>>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.is_tty {
      let _ = utils::clear_line();
    }

    utils::log_info(&format!(
      "{} {} modules transformed.",
      utils::paint("✓", Style::new().green()),
      Self::transformed_module_count(ctx)
    ));

    Ok(())
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    let chunk_count = self.chunk_count.fetch_add(1, Ordering::SeqCst);
    if self.is_tty {
      utils::write_line(&format!(
        "rendering chunks ({})...",
        itoa::Buffer::new().format(chunk_count + 1)
      ));
    } else if chunk_count == 0 {
      utils::log_info("rendering chunks...");
    }
    Ok(None)
  }

  async fn write_bundle(
    &self,
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookWriteBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    let mut has_large_chunks = false;
    if let Some(log_info) = &self.log_info {
      let mut longest = 0;
      let mut biggest_size = 0;
      let mut biggest_map_size = 0;
      let mut biggest_compress_size = 0;

      let mut log_entries = Vec::with_capacity(args.bundle.len());
      let asset_size_by_filename =
        if matches!(args.options.sourcemap, Some(SourceMapType::File | SourceMapType::Hidden)) {
          args
            .bundle
            .iter()
            .filter_map(|output| match output {
              rolldown_common::Output::Asset(asset) => {
                Some((asset.filename.as_str(), asset.source.as_bytes().len()))
              }
              rolldown_common::Output::Chunk(_) => None,
            })
            .collect::<FxHashMap<_, _>>()
        } else {
          FxHashMap::default()
        };

      if self.report_compressed_size {
        utils::log_info("computing gzip size...");
      }

      let pre_compute_size = self.report_compressed_size.then(|| {
        args
          .bundle
          .par_iter()
          .map(|output| match output {
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
          })
          .collect::<Vec<_>>()
      });

      for (idx, output) in args.bundle.iter().enumerate() {
        let log_entry = match output {
          rolldown_common::Output::Chunk(chunk) => utils::LogEntry {
            name: &chunk.filename,
            size: chunk.code.len(),
            group: utils::AssetGroup::JS,
            map_size: chunk.map.as_ref().map(|map| {
              chunk
                .sourcemap_filename
                .as_deref()
                .and_then(|filename| asset_size_by_filename.get(filename).copied())
                .unwrap_or_else(|| map.to_json_string().len())
            }),
            compressed_size: pre_compute_size.as_ref().and_then(|v| v[idx]),
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
              compressed_size: pre_compute_size.as_ref().and_then(|v| v[idx]),
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

      let out_dir = relative_path_to_slash(
        args.options.cwd.join(&args.options.out_dir).normalize(),
        &args.options.cwd,
      );
      let out_dir_prefix = format!("{out_dir}/");

      let mut info = String::new();
      for group in utils::GROUPS {
        let mut filtered = log_entries.iter().filter(|e| e.group == group).collect::<Vec<_>>();
        if filtered.is_empty() {
          continue;
        }
        filtered.sort_by_key(|a| a.size);
        for log_entry in filtered {
          let _ = write!(&mut info, "{}", utils::paint(&out_dir_prefix, Style::new().dimmed()));

          let is_asset = !self.is_lib && Path::new(log_entry.name).starts_with(&self.assets_dir);
          if is_asset {
            let _ = write!(
              &mut info,
              "{}",
              utils::paint(self.assets_dir.cow_replace('\\', "/"), Style::new().dimmed())
            );
          }

          let name = if is_asset {
            let dir_len = self.assets_dir.len();
            format!("{:pad$}", &log_entry.name[dir_len..], pad = longest + 2 - dir_len)
          } else {
            format!("{:pad$}", log_entry.name, pad = longest + 2)
          };

          let _ = match group {
            utils::AssetGroup::JS => {
              write!(&mut info, "{}", utils::paint(&name, Style::new().cyan()))
            }
            utils::AssetGroup::Css => {
              write!(&mut info, "{}", utils::paint(&name, Style::new().magenta()))
            }
            utils::AssetGroup::Assets => {
              write!(&mut info, "{}", utils::paint(&name, Style::new().green()))
            }
          };

          let size = format!("{:>size_pad$}", utils::display_size(log_entry.size));
          if group == utils::AssetGroup::JS && log_entry.size.div_ceil(1000) > self.chunk_limit {
            has_large_chunks = true;
            let _ = write!(&mut info, "{}", utils::paint(&size, Style::new().bold().yellow()));
          } else {
            let _ = write!(&mut info, "{}", utils::paint(&size, Style::new().bold().dimmed()));
          }

          if let Some(compressed_size) = log_entry.compressed_size {
            let size = utils::display_size(compressed_size);
            let _ = write!(
              &mut info,
              "{}",
              utils::paint(format!(" │ gzip: {size:>compress_pad$}"), Style::new().dimmed())
            );
          }

          if let Some(map_size) = log_entry.map_size {
            let size = utils::display_size(map_size);
            let _ = write!(
              &mut info,
              "{}",
              utils::paint(format!(" │ map: {size:>map_pad$}"), Style::new().dimmed())
            );
          }

          let _ = writeln!(&mut info);
        }
      }
      log_info(info).await?;
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
      let message = utils::paint(
        format!(
          "\n(!) Some chunks are larger than {} kB after minification. Consider:\n- Using dynamic import() to code-split the application\n- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting\n- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.",
          itoa::Buffer::new().format(self.chunk_limit)
        ),
        Style::new().bold().yellow(),
      )
      .to_string();
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
    let mut usage = HookUsage::empty();
    if self.log_info.is_some() {
      usage |= HookUsage::ModuleParsed
        | HookUsage::Transform
        | HookUsage::BuildStart
        | HookUsage::BuildEnd
        | HookUsage::RenderChunk
        | HookUsage::WriteBundle;
      if self.is_tty {
        usage |= HookUsage::GenerateBundle;
      }
    } else if self.warn_large_chunks {
      usage |= HookUsage::WriteBundle;
    }
    usage
  }
}
