use rolldown_common::{Loader, NormalizedBundlerOptions, Platform, SourceMapType};
use rustc_hash::FxHashMap;

#[allow(clippy::struct_field_names)]
pub struct NormalizeOptionsReturn {
  pub options: NormalizedBundlerOptions,
  pub resolve_options: rolldown_resolver::ResolveOptions,
}

pub fn normalize_options(mut raw_options: crate::BundlerOptions) -> NormalizeOptionsReturn {
  // Take out resolve options

  let raw_resolve = std::mem::take(&mut raw_options.resolve).unwrap_or_default();

  let mut loaders = FxHashMap::from(
    [
      ("json".to_string(), Loader::Json),
      ("js".to_string(), Loader::Js),
      ("mjs".to_string(), Loader::Js),
      ("cjs".to_string(), Loader::Js),
    ]
    .into_iter()
    .collect(),
  );

  let user_defined_loaders: FxHashMap<String, Loader> = raw_options
    .loaders
    .map(|loaders| {
      loaders
        .into_iter()
        .map(|(ext, value)| {
          let stripped = ext.strip_prefix('.').map(ToString::to_string).unwrap_or(ext);

          (stripped, value)
        })
        .collect()
    })
    .unwrap_or_default();

  loaders.extend(user_defined_loaders);

  let normalized = NormalizedBundlerOptions {
    input: raw_options.input.unwrap_or_default(),
    cwd: raw_options
      .cwd
      .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir")),
    external: raw_options.external,
    treeshake: raw_options.treeshake.unwrap_or(true),
    platform: raw_options.platform.unwrap_or(Platform::Browser),
    entry_filenames: raw_options.entry_filenames.unwrap_or_else(|| "[name].js".to_string()).into(),
    chunk_filenames: raw_options
      .chunk_filenames
      .unwrap_or_else(|| "[name]-[hash].js".to_string())
      .into(),
    banner: raw_options.banner,
    footer: raw_options.footer,
    dir: raw_options.dir.unwrap_or_else(|| "dist".to_string()),
    format: raw_options.format.unwrap_or(crate::OutputFormat::Esm),
    sourcemap: raw_options.sourcemap.unwrap_or(SourceMapType::Hidden),
    sourcemap_ignore_list: raw_options.sourcemap_ignore_list,
    sourcemap_path_transform: raw_options.sourcemap_path_transform,
    shim_missing_exports: raw_options.shim_missing_exports.unwrap_or(false),
    loaders,
  };

  NormalizeOptionsReturn { options: normalized, resolve_options: raw_resolve }
}
