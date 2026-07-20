use std::path::{Path, PathBuf};
use std::sync::Arc;

use criterion::Criterion;
use rolldown::{
  BundleFactory, BundleFactoryOptions, BundlerOptions, InputItem, Platform, ResolveOptions,
  TsConfig,
};
use rolldown_fs::MemoryFileSystem;
use rolldown_resolver::Resolver;
use rolldown_workspace::root_dir;

pub fn bench_preset(name: &str, bench_dir: &str, entry: &str) -> BundlerOptions {
  let dir = root_dir().join(bench_dir);
  BundlerOptions {
    input: Some(vec![InputItem {
      name: Some(name.to_string()),
      import: dir.join(entry).to_str().unwrap().to_string(),
    }]),
    cwd: Some(dir),
    ..Default::default()
  }
}

pub fn rome_ts_preset() -> BundlerOptions {
  let mut opts = bench_preset("rome-ts", "tmp/bench/rome", "src/entry.ts");
  opts.shim_missing_exports = Some(true);
  opts.tsconfig = Some(TsConfig::Manual(root_dir().join("tmp/bench/rome/src/tsconfig.json")));
  opts
}

pub struct BenchItem {
  pub name: String,
  pub options: BundlerOptions,
}

pub struct DeriveOptions {
  pub sourcemap: bool,
  pub minify: bool,
}

pub fn derive_benchmark_items(
  derive_options: &DeriveOptions,
  name: &str,
  options: BundlerOptions,
) -> Vec<BenchItem> {
  let mut ret = vec![BenchItem { name: name.to_string(), options: options.clone() }];

  if derive_options.sourcemap {
    ret.push(BenchItem {
      name: format!("{name}-sourcemap"),
      options: {
        let mut options = options.clone();
        options.sourcemap = Some(rolldown::SourceMapType::File);
        options
      },
    });
  }

  if derive_options.minify {
    ret.push(BenchItem {
      name: format!("{name}-minify"),
      options: {
        let mut options = options.clone();
        options.minify = Some(true.into());
        options
      },
    });
  }

  if derive_options.sourcemap && derive_options.minify {
    ret.push(BenchItem {
      name: format!("{name}-minify-sourcemap"),
      options: {
        let mut options = options;
        options.sourcemap = Some(rolldown::SourceMapType::File);
        options.minify = Some(true.into());
        options
      },
    });
  }

  ret
}

/// Walk a directory recursively and load all files into a `MemoryFileSystem`.
/// This is used in benchmarks to eliminate disk I/O from the timed section.
pub fn preload_into_memory_fs(dir: &Path) -> MemoryFileSystem {
  let mut fs = MemoryFileSystem::default();
  for entry in ignore::WalkBuilder::new(dir)
    .ignore(false)
    .git_ignore(false)
    .git_global(false)
    .git_exclude(false)
    .build()
    .flatten()
  {
    let path = entry.path();
    if path.is_file()
      && let Ok(content) = std::fs::read(path)
    {
      fs.add_file_bytes(path, &content);
    }
  }
  fs
}

/// Precomputed benchmark context: factory, MemoryFileSystem, and resolver config.
/// Created once per benchmark item (outside the timed loop).
pub struct BenchContext {
  pub factory: BundleFactory,
  pub mem_fs: MemoryFileSystem,
  pub cwd: PathBuf,
  pub platform: Platform,
  pub tsconfig: TsConfig,
  pub raw_resolve: ResolveOptions,
}

impl BenchContext {
  /// Create a fresh resolver for each benchmark iteration to avoid cache warming bias.
  pub fn create_resolver(&self) -> Arc<Resolver<MemoryFileSystem>> {
    Arc::new(Resolver::new(
      self.mem_fs.clone(),
      self.cwd.clone(),
      self.platform,
      &self.tsconfig,
      self.raw_resolve.clone(),
    ))
  }
}

/// Create a `BenchContext` for a given set of bundler options.
/// This performs all one-time setup (option normalization, FS preloading, resolver creation)
/// so the timed loop only measures bundling work.
pub fn create_bench_context(options: &BundlerOptions) -> BenchContext {
  let cwd = options
    .cwd
    .clone()
    .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir"));
  let mem_fs = preload_into_memory_fs(&cwd);
  // Mirror the normalization in prepare_build_context: derive platform from format,
  // and add default condition_names for Browser/Node.
  let format = options.format.unwrap_or(rolldown::OutputFormat::Esm);
  let platform = options.platform.unwrap_or(match format {
    rolldown::OutputFormat::Cjs => Platform::Node,
    rolldown::OutputFormat::Esm | rolldown::OutputFormat::Iife | rolldown::OutputFormat::Umd => {
      Platform::Browser
    }
  });
  let tsconfig = options.tsconfig.clone().map(|tc| tc.with_base(&cwd)).unwrap_or_default();
  let mut raw_resolve = options.resolve.clone().unwrap_or_default();
  if raw_resolve.condition_names.is_none() && matches!(platform, Platform::Browser | Platform::Node)
  {
    raw_resolve.condition_names = Some(vec!["module".to_string()]);
  }
  let factory = BundleFactory::new(BundleFactoryOptions {
    bundler_options: options.clone(),
    plugins: vec![],
    session: None,
    disable_tracing_setup: true,
  })
  .expect("Failed to create bundle factory");
  BenchContext { factory, mem_fs, cwd, platform, tsconfig, raw_resolve }
}

#[derive(Clone, Copy)]
pub enum BenchMode {
  Scan,
  Bundle,
}

/// Criterion executor that drives benchmark futures on the production shared
/// async runtime (`rolldown_utils::async_runtime`) — the same scheduler
/// `generate()`'s internal spawns run on, so the outer bench driver and the
/// inner bundling work share one runtime with no cross-runtime wakeup hop.
struct SharedRuntimeExecutor;

impl criterion::async_executor::AsyncExecutor for SharedRuntimeExecutor {
  fn block_on<T>(&self, future: impl std::future::Future<Output = T>) -> T {
    rolldown_utils::async_runtime::block_on(future)
  }
}

// The shared runtime's configuration freezes on the first submission, so pin
// it exactly once before any benchmark iteration runs.
static CONFIGURE_SHARED_RUNTIME: std::sync::Once = std::sync::Once::new();

pub fn run_bench_group(
  c: &mut Criterion,
  group_name: &str,
  mode: BenchMode,
  derive_options: &DeriveOptions,
  items: Vec<(&str, BundlerOptions)>,
) {
  let mut group = c.benchmark_group(group_name);
  // Pin the historical bench configuration (MultiThread, 8 workers / 4
  // blocking tasks) for A/B parity with the previously pinned tokio runtime.
  // Production instead resolves these limits from the environment at addon
  // load. The runtime starts lazily on the first submission — no explicit
  // start is needed.
  CONFIGURE_SHARED_RUNTIME.call_once(|| {
    rolldown_utils::async_runtime::configure(rolldown_utils::async_runtime::RuntimeOptions {
      flavor: rolldown_utils::async_runtime::RuntimeFlavor::MultiThread,
      worker_threads: 8,
      max_blocking_tasks: 4,
      ..Default::default()
    })
    .expect("Failed to configure the shared async runtime");
  });

  for (name, options) in items {
    for item in derive_benchmark_items(derive_options, name, options) {
      let mut ctx = create_bench_context(&item.options);
      group.bench_function(format!("{group_name}@{}", item.name), |b| {
        b.to_async(SharedRuntimeExecutor).iter(|| {
          let bundle = ctx.factory.create_bundle_with_fs(ctx.mem_fs.clone(), ctx.create_resolver());
          async {
            match mode {
              BenchMode::Scan => {
                bundle.scan().await.expect("Failed to scan");
              }
              BenchMode::Bundle => {
                bundle.generate().await.expect("Failed to bundle");
              }
            }
          }
        });
      });
    }
  }
}
