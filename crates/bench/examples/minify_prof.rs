use std::borrow::Cow;

use bench::{bench_preset, create_bench_context_with_plugins, rome_ts_preset};
use rolldown::{
  AddonOutputOption, BundlerOptions, CodeSplittingMode, DevModeOptions, ExperimentalOptions,
  InlineCommonChunksOptions, InputItem, ManualCodeSplittingOptions, OutputFormat,
  PreserveEntrySignatures, RawMinifyOptions, SourceMapType,
  plugin::{
    HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookUsage, Plugin,
    PluginContext,
  },
};

#[global_allocator]
static GLOBAL: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

#[derive(Debug)]
struct RenderChunkBenchmark {
  replace: bool,
}

impl Plugin for RenderChunkBenchmark {
  fn name(&self) -> Cow<'static, str> {
    "deferred-render-chunk-benchmark".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    Ok(self.replace.then(|| HookRenderChunkOutput {
      code: format!("{}\nglobalThis.__deferredBenchmark = 6 * 7;", args.code),
      map: Default::default(),
    }))
  }
}

#[cfg(unix)]
fn process_cpu_ns() -> u64 {
  let mut time = libc::timespec { tv_sec: 0, tv_nsec: 0 };
  // SAFETY: `time` is a valid writable `timespec` for the duration of the call.
  if unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &raw mut time) } != 0 {
    return 0;
  }
  (time.tv_sec as u64) * 1_000_000_000 + time.tv_nsec as u64
}

#[cfg(not(unix))]
fn process_cpu_ns() -> u64 {
  0
}

/// Peak resident set size of this process in MiB (`getrusage(RUSAGE_SELF).ru_maxrss`).
#[cfg(unix)]
fn max_rss_mib() -> f64 {
  let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
  // SAFETY: `usage` points to writable storage for `rusage` and is only read after success.
  if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
    return 0.0;
  }
  // SAFETY: `getrusage` initialized `usage` when it returned success.
  let max_rss = unsafe { usage.assume_init() }.ru_maxrss as f64;
  // macOS reports bytes; Linux and the other supported Unix targets report KiB.
  let bytes_per_unit = if cfg!(target_vendor = "apple") { 1.0 } else { 1024.0 };
  max_rss * bytes_per_unit / (1024.0 * 1024.0)
}

#[cfg(not(unix))]
fn max_rss_mib() -> f64 {
  0.0
}

fn main() {
  let args: Vec<String> = std::env::args().collect();
  let mut opts: BundlerOptions = match args[1].as_str() {
    "three10x" => bench_preset("three10x", "tmp/bench/three10x", "entry.js"),
    "rome" => rome_ts_preset(),
    other if other.starts_with("rome:") => {
      let mut opts = rome_ts_preset();
      opts.input = Some(vec![rolldown::InputItem {
        name: Some("rome-ts".to_string()),
        import: std::path::Path::new("tmp/bench/rome")
          .join(&other["rome:".len()..])
          .canonicalize()
          .unwrap()
          .to_string_lossy()
          .into_owned(),
      }]);
      opts
    }
    other => {
      let (d, e) = other.split_once(':').unwrap_or((other, "entry.js"));
      bench_preset("x", d, e)
    }
  };
  if let Ok(entries) = std::env::var("BENCH_ENTRIES") {
    opts.input = Some(
      entries.split(',').map(|entry| InputItem { name: None, import: entry.to_string() }).collect(),
    );
  }
  let scenario = std::env::var("DEFER").unwrap_or_else(|_| "none".to_string());
  let mut plugins: Vec<rolldown::plugin::__inner::SharedPluginable> = Vec::new();
  match scenario.as_str() {
    "none" => {}
    "unchanged" => plugins.push(Plugin::new_shared(RenderChunkBenchmark { replace: false })),
    "replaced" => plugins.push(Plugin::new_shared(RenderChunkBenchmark { replace: true })),
    "addon" => {
      opts.intro = Some(AddonOutputOption::String(Some(
        "globalThis.__deferredBenchmark = 6 * 7;".to_string(),
      )));
    }
    "inline" | "dev" | "dev-inline" => {
      if scenario != "inline" {
        opts.experimental = Some(ExperimentalOptions {
          dev_mode: Some(DevModeOptions::default()),
          ..Default::default()
        });
      }
      if scenario != "dev" {
        opts.code_splitting = Some(CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
          experimental_inline_common_chunks: Some(InlineCommonChunksOptions {
            max_size: Some(f64::INFINITY),
            exclude: None,
          }),
          ..Default::default()
        }));
      }
      opts.preserve_entry_signatures = Some(PreserveEntrySignatures::False);
    }
    other => panic!("unknown DEFER scenario: {other}"),
  }
  let format = std::env::var("FORMAT").unwrap_or_else(|_| "esm".to_string());
  match format.as_str() {
    "esm" => {}
    "iife" | "umd" => {
      opts.format = Some(if format == "iife" { OutputFormat::Iife } else { OutputFormat::Umd });
      opts.name = Some("DeferredBenchmark".to_string());
    }
    other => panic!("unknown FORMAT: {other}"),
  }
  let iters: usize = args[2].parse().unwrap();
  if std::env::var_os("NO_MAP").is_none() {
    opts.sourcemap = Some(SourceMapType::File);
  }
  match args[3].as_str() {
    "off" => opts.minify = Some(false.into()),
    "dce" => opts.minify = Some(RawMinifyOptions::DeadCodeEliminationOnly),
    "full" => opts.minify = Some(true.into()),
    _ => {}
  }
  if std::env::var("PLATFORM").is_ok_and(|p| p == "node") {
    opts.platform = Some(rolldown::Platform::Node);
  }
  let out = args.get(4).cloned();
  let mut ctx = create_bench_context_with_plugins(&opts, plugins);
  let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
  let p0 = process_cpu_ns();
  let w0 = std::time::Instant::now();
  for i in 0..iters {
    let bundle = ctx.factory.create_bundle_with_fs(ctx.mem_fs.clone(), ctx.create_resolver());
    let output = rt.block_on(bundle.generate()).unwrap();
    if i == 0
      && let Some(dir) = &out
    {
      std::fs::create_dir_all(dir).unwrap();
      for asset in &output.assets {
        let path = std::path::Path::new(dir).join(asset.filename());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, asset.content_as_bytes()).unwrap();
      }
    }
  }
  let wall = w0.elapsed().as_secs_f64() * 1e3;
  let proc_cpu = (process_cpu_ns() - p0) as f64 / 1e6;
  println!(
    "{} {} scenario={scenario} format={format} map={} iters={iters} wall={wall:.3}ms proc_cpu={proc_cpu:.3}ms max_rss={:.3}MiB",
    args[1],
    args[3],
    opts.sourcemap.is_some(),
    max_rss_mib(),
  );
}
