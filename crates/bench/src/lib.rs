use std::path::PathBuf;

use rolldown::BundlerOptions;
use rolldown_workspace::root_dir;

pub fn join_by_workspace_root(path: &str) -> PathBuf {
  root_dir().join(path)
}

pub struct BenchItem {
  pub name: String,
  pub options: Box<dyn Fn() -> BundlerOptions + 'static>,
}

pub struct DeriveOptions {
  pub sourcemap: bool,
  pub minify: bool,
}

pub fn derive_benchmark_items(
  derive_options: &DeriveOptions,
  name: String,
  create_bundler_options: impl Fn() -> BundlerOptions + 'static + Clone,
) -> Vec<BenchItem> {
  let mut ret =
    vec![BenchItem { name: name.clone(), options: Box::new(create_bundler_options.clone()) }];

  if derive_options.sourcemap {
    let create_bundler_options = create_bundler_options.clone();
    ret.push(BenchItem {
      name: format!("{}-sourcemap", name),
      options: Box::new(move || {
        let mut options = create_bundler_options();
        options.sourcemap = Some(rolldown::SourceMapType::File);
        options
      }),
    });
  }

  if derive_options.minify {
    let create_bundler_options = create_bundler_options.clone();
    ret.push(BenchItem {
      name: format!("{}-minify", name),
      options: Box::new(move || {
        let mut options = create_bundler_options();
        options.minify = Some(true.into());
        options
      }),
    });
  }

  if derive_options.sourcemap && derive_options.minify {
    ret.push(BenchItem {
      name: format!("{}-minify-sourcemap", name),
      options: Box::new(move || {
        let mut options = create_bundler_options();
        options.sourcemap = Some(rolldown::SourceMapType::File);
        options.minify = Some(true.into());
        options
      }),
    });
  }

  ret
}
