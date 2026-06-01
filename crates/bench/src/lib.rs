use std::path::{Path, PathBuf};
use std::sync::Arc;

use criterion::Criterion;
use rolldown::{
  BundleFactory, BundleFactoryOptions, BundlerOptions, InputItem, Platform, ResolveOptions,
  TsConfig,
};
use rolldown::plugin::__inner::SharedPluginable;
use rolldown::plugin::{
  HookTransformArgs, HookTransformOutput, HookTransformOutputMap, HookTransformReturn, HookUsage,
  Plugin, SharedTransformPluginContext,
};
use rolldown_fs::MemoryFileSystem;
use rolldown_resolver::Resolver;
use rolldown_workspace::root_dir;

/// How a transform hook reports "no sourcemap" for its changed code.
///
/// Both map to the code path that commit `f6653cb7b` ("avoid unnecessary
/// intermediate sourcemaps") optimizes: previously each such transform forced
/// a full hires intermediate sourcemap (with a copy of the source content) to
/// be built and kept alive until the render stage.
#[derive(Debug, Clone, Copy)]
pub enum MapMode {
  /// `map` field omitted entirely -> `SourcemapChainElement::Omitted`.
  Omitted,
  /// explicit `map: null` -> `SourcemapChainElement::Null`.
  Null,
}

/// A minimal transform plugin that changes the code of every module but
/// returns no sourcemap, exercising the intermediate-sourcemap path.
#[derive(Debug)]
pub struct OmitMapTransformPlugin {
  pub mode: MapMode,
}

impl Plugin for OmitMapTransformPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("bench:omit-map-transform")
  }

  fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> impl std::future::Future<Output = HookTransformReturn> + Send {
    let map = match self.mode {
      MapMode::Omitted => HookTransformOutputMap::Omitted,
      MapMode::Null => HookTransformOutputMap::Null,
    };
    // The code MUST change, otherwise the driver records nothing for this hook.
    let code = format!("/* bench-transform */\n{}", args.code);
    async move { Ok(Some(HookTransformOutput { code: Some(code), map, ..Default::default() })) }
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}

/// Build the omit-map transform plugin wrapped as a `SharedPluginable`.
pub fn omit_map_plugin(mode: MapMode) -> SharedPluginable {
  Arc::new(OmitMapTransformPlugin { mode })
}

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
  pub plugins: Vec<SharedPluginable>,
}

pub struct DeriveOptions {
  pub sourcemap: bool,
  pub minify: bool,
}

pub fn derive_benchmark_items(
  derive_options: &DeriveOptions,
  name: &str,
  options: BundlerOptions,
  plugins: Vec<SharedPluginable>,
) -> Vec<BenchItem> {
  let mut ret =
    vec![BenchItem { name: name.to_string(), options: options.clone(), plugins: plugins.clone() }];

  if derive_options.sourcemap {
    ret.push(BenchItem {
      name: format!("{name}-sourcemap"),
      options: {
        let mut options = options.clone();
        options.sourcemap = Some(rolldown::SourceMapType::File);
        options
      },
      plugins: plugins.clone(),
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
      plugins: plugins.clone(),
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
      plugins,
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
  create_bench_context_with_plugins(options, vec![])
}

/// Like [`create_bench_context`] but registers `plugins` on the bundle factory,
/// so benchmark items can exercise transform-hook code paths.
pub fn create_bench_context_with_plugins(
  options: &BundlerOptions,
  plugins: Vec<SharedPluginable>,
) -> BenchContext {
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
    plugins,
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

pub fn run_bench_group(
  c: &mut Criterion,
  group_name: &str,
  mode: BenchMode,
  derive_options: &DeriveOptions,
  items: Vec<(&str, BundlerOptions, Vec<SharedPluginable>)>,
) {
  let mut group = c.benchmark_group(group_name);
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(8)
    .enable_all()
    .max_blocking_threads(4)
    .build()
    .expect("Failed to build tokio runtime");

  for (name, options, plugins) in items {
    for item in derive_benchmark_items(derive_options, name, options, plugins) {
      let mut ctx = create_bench_context_with_plugins(&item.options, item.plugins.clone());
      group.bench_function(format!("{group_name}@{}", item.name), |b| {
        b.to_async(&runtime).iter(|| {
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
