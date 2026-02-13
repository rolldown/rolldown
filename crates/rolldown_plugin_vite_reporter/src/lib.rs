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
use owo_colors::{OwoColorize, Stream};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use sugar_path::SugarPath as _;

pub type LogInfoFn =
  dyn Fn(String) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> + Send + Sync;

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
  pub transformed_count: AtomicU32,
  pub latest_checkpoint: Arc<RwLock<Instant>>,
  #[debug(skip)]
  pub log_info: Option<Arc<LogInfoFn>>,
}

impl Plugin for ViteReporterPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-reporter")
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
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
          itoa::Buffer::new().format(transformed_count + 1),
          Path::new(args.id)
            .relative(&self.root)
            .to_string_lossy()
            .if_supports_color(Stream::Stdout, |text| { text.dimmed() })
        ));

        *self.latest_checkpoint.write().unwrap() = now;
      }
    } else if transformed_count == 0 {
      utils::write_line("transforming...");
    }
    Ok(None)
  }

  async fn build_start(
    &self,
    _ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    self.chunk_count.store(0, Ordering::SeqCst);
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
      "✓".if_supports_color(Stream::Stdout, |text| text.green()),
      self.transformed_count.load(Ordering::SeqCst)
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
            map_size: chunk.map.as_ref().map(|m| m.to_json_string().len()),
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

      let out_dir =
        args.options.cwd.join(&args.options.out_dir).normalize().relative(&args.options.cwd);
      let out_dir = out_dir.to_slash_lossy();

      let mut info = String::new();
      for group in utils::GROUPS {
        let mut filtered = log_entries.iter().filter(|e| e.group == group).collect::<Vec<_>>();
        if filtered.is_empty() {
          continue;
        }
        filtered.sort_by(|a, b| a.size.cmp(&b.size));
        for log_entry in filtered {
          let _ = write!(
            &mut info,
            "{}",
            format!("{out_dir}/").if_supports_color(Stream::Stdout, |text| text.dimmed())
          );

          let is_asset = !self.is_lib && Path::new(log_entry.name).starts_with(&self.assets_dir);
          if is_asset {
            let _ = write!(
              &mut info,
              "{}",
              self
                .assets_dir
                .cow_replace('\\', "/")
                .if_supports_color(Stream::Stdout, |text| text.dimmed())
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
              write!(&mut info, "{}", name.if_supports_color(Stream::Stdout, |text| text.cyan()))
            }
            utils::AssetGroup::Css => {
              write!(&mut info, "{}", name.if_supports_color(Stream::Stdout, |text| text.magenta()))
            }
            utils::AssetGroup::Assets => {
              write!(&mut info, "{}", name.if_supports_color(Stream::Stdout, |text| text.green()))
            }
          };

          let size = format!("{:>size_pad$}", utils::display_size(log_entry.size));
          if group == utils::AssetGroup::JS && log_entry.size.div_ceil(1000) > self.chunk_limit {
            has_large_chunks = true;
            let _ = write!(
              &mut info,
              "{}",
              size.if_supports_color(Stream::Stdout, |text| text.bold().yellow().to_string())
            );
          } else {
            let _ = write!(
              &mut info,
              "{}",
              size.if_supports_color(Stream::Stdout, |text| text.bold().dimmed().to_string())
            );
          }

          if let Some(compressed_size) = log_entry.compressed_size {
            let size = utils::display_size(compressed_size);
            let _ = write!(
              &mut info,
              "{}",
              format!(" │ gzip: {size:>compress_pad$}")
                .if_supports_color(Stream::Stdout, |text| text.dimmed())
            );
          }

          if let Some(map_size) = log_entry.map_size {
            let size = utils::display_size(map_size);
            let _ = write!(
              &mut info,
              "{}",
              format!(" │ map: {size:>map_pad$}")
                .if_supports_color(Stream::Stdout, |text| text.dimmed())
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
      let message = format!(
        "\n(!) Some chunks are larger than {} kB after minification. Consider:\n- Using dynamic import() to code-split the application\n- Use build.rolldownOptions.output.codeSplitting to improve chunking: https://rolldown.rs/reference/OutputOptions.codeSplitting\n- Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.",
        itoa::Buffer::new().format(self.chunk_limit)
      ).if_supports_color(Stream::Stdout, |text| { text.bold().yellow().to_string() }).to_string();
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
      usage |= HookUsage::Transform
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
