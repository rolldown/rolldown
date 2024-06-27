use rolldown_common::{
  ModuleType, NormalizedBundlerOptions, NormalizedInputItem, Platform, SourceMapType,
};
use rustc_hash::FxHashMap;

use super::extract_meaningful_input_name_from_path::try_extract_meaningful_input_name_from_path;

pub struct NormalizeOptionsReturn {
  pub options: NormalizedBundlerOptions,
  pub resolve_options: rolldown_resolver::ResolveOptions,
}

pub fn normalize_options(mut raw_options: crate::BundlerOptions) -> NormalizeOptionsReturn {
  // Take out resolve options

  let raw_resolve = std::mem::take(&mut raw_options.resolve).unwrap_or_default();

  let mut loaders = FxHashMap::from(
    [
      ("js".to_string(), ModuleType::Js),
      ("mjs".to_string(), ModuleType::Js),
      ("cjs".to_string(), ModuleType::Js),
      ("jsx".to_string(), ModuleType::Jsx),
      ("ts".to_string(), ModuleType::Ts),
      ("mts".to_string(), ModuleType::Ts),
      ("cts".to_string(), ModuleType::Ts),
      ("tsx".to_string(), ModuleType::Tsx),
      ("json".to_string(), ModuleType::Json),
      ("txt".to_string(), ModuleType::Text),
    ]
    .into_iter()
    .collect(),
  );

  let user_defined_loaders: FxHashMap<String, ModuleType> = raw_options
    .module_types
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

  let has_only_one_input_item = matches!(&raw_options.input, Some(items) if items.len() == 1);
  let input = raw_options
    .input
    .unwrap_or_default()
    .into_iter()
    .enumerate()
    .map(|(idx, raw)| {
      let name = raw.name.unwrap_or_else(|| {
        // We try to give a meaningful name for unnamed input item.
        let fallback_name =
          || if has_only_one_input_item { "input".to_string() } else { format!("input~{idx}") };

        // If the input is a data URL, no way we can get a meaningful name. Just fallback to the default.
        if raw.import.starts_with("data:") {
          return fallback_name();
        }

        // If it's a file path, use the file name of it.
        try_extract_meaningful_input_name_from_path(&raw.import).unwrap_or_else(fallback_name)
      });
      NormalizedInputItem { name, import: raw.import }
    })
    .collect::<Vec<_>>();

  let normalized = NormalizedBundlerOptions {
    input,
    cwd: raw_options
      .cwd
      .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir")),
    external: raw_options.external,
    treeshake: raw_options.treeshake,
    platform: raw_options.platform.unwrap_or(Platform::Browser),
    entry_filenames: raw_options.entry_filenames.unwrap_or_else(|| "[name].js".to_string()).into(),
    chunk_filenames: raw_options
      .chunk_filenames
      .unwrap_or_else(|| "[name]-[hash].js".to_string())
      .into(),
    asset_filenames: raw_options
      .asset_filenames
      .unwrap_or_else(|| "assets/[name]-[hash][extname]".to_string())
      .into(),
    banner: raw_options.banner,
    footer: raw_options.footer,
    dir: raw_options.dir.unwrap_or_else(|| "dist".to_string()),
    format: raw_options.format.unwrap_or(crate::OutputFormat::Esm),
    sourcemap: raw_options.sourcemap.unwrap_or(SourceMapType::Hidden),
    sourcemap_ignore_list: raw_options.sourcemap_ignore_list,
    sourcemap_path_transform: raw_options.sourcemap_path_transform,
    shim_missing_exports: raw_options.shim_missing_exports.unwrap_or(false),
    module_types: loaders,
  };

  NormalizeOptionsReturn { options: normalized, resolve_options: raw_resolve }
}
