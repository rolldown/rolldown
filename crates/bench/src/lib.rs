use std::path::{Path, PathBuf};
use std::sync::Arc;

use rolldown::{
  BundleFactory, BundleFactoryOptions, BundlerOptions, Platform, ResolveOptions, TsConfig,
};
use rolldown_fs::MemoryFileSystem;
use rolldown_resolver::Resolver;
use rolldown_workspace::root_dir;

pub fn join_by_workspace_root(path: &str) -> PathBuf {
  root_dir().join(path)
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
  walk_and_load(dir, &mut fs);
  fs
}

fn walk_and_load(dir: &Path, fs: &mut MemoryFileSystem) {
  let entries = match std::fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(_) => return,
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      walk_and_load(&path, fs);
    } else if path.is_file()
      && let Ok(content) = std::fs::read(&path)
    {
      fs.add_file_bytes(&path, &content);
    }
  }
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
