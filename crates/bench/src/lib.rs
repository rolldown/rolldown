use std::path::{Path, PathBuf};
use std::sync::Arc;

use rolldown::{BundleFactory, BundleFactoryOptions, BundlerOptions};
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
    } else if path.is_file() {
      // MemoryFileSystem only supports UTF-8 content; binary files are skipped.
      if let Ok(content) = std::fs::read_to_string(&path) {
        fs.add_file(&path, &content);
      }
    }
  }
}

/// Precomputed benchmark context: factory, MemoryFileSystem, and resolver.
/// Created once per benchmark item (outside the timed loop).
pub struct BenchContext {
  pub factory: BundleFactory,
  pub mem_fs: MemoryFileSystem,
  pub resolver: Arc<Resolver<MemoryFileSystem>>,
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
  let platform = options.platform.unwrap_or(rolldown::Platform::Browser);
  let raw_resolve = options.resolve.clone().unwrap_or_default();
  let resolver = Arc::new(Resolver::new(
    mem_fs.clone(),
    cwd,
    platform,
    &Default::default(),
    raw_resolve,
  ));
  let factory = BundleFactory::new(BundleFactoryOptions {
    bundler_options: options.clone(),
    plugins: vec![],
    session: None,
    disable_tracing_setup: true,
  })
  .expect("Failed to create bundle factory");
  BenchContext { factory, mem_fs, resolver }
}
