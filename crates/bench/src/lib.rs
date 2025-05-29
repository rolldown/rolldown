use std::path::PathBuf;

use rolldown::BundlerOptions;
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
      name: format!("{}-sourcemap", name),
      options: {
        let mut options = options.clone();
        options.sourcemap = Some(rolldown::SourceMapType::File);
        options
      },
    });
  }

  if derive_options.minify {
    ret.push(BenchItem {
      name: format!("{}-minify", name),
      options: {
        let mut options = options.clone();
        options.minify = Some(true.into());
        options
      },
    });
  }

  if derive_options.sourcemap && derive_options.minify {
    ret.push(BenchItem {
      name: format!("{}-minify-sourcemap", name),
      options: {
        let mut options = options.clone();
        options.sourcemap = Some(rolldown::SourceMapType::File);
        options.minify = Some(true.into());
        options
      },
    });
  }

  ret
}
