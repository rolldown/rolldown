use rolldown_common::{NormalizedBundlerOptions, Platform, SourceMapType};

#[allow(clippy::struct_field_names)]
pub struct NormalizeOptionsReturn {
  pub options: NormalizedBundlerOptions,
  pub resolve_options: rolldown_resolver::ResolveOptions,
}

pub fn normalize_options(mut raw_options: crate::BundlerOptions) -> NormalizeOptionsReturn {
  // Take out resolve options

  let raw_resolve = std::mem::take(&mut raw_options.resolve).unwrap_or_default();

  let normalized = NormalizedBundlerOptions {
    input: raw_options.input.unwrap_or_default(),
    cwd: raw_options
      .cwd
      .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir")),
    external: raw_options.external.unwrap_or_default(),
    treeshake: raw_options.treeshake.unwrap_or(true),
    platform: raw_options.platform.unwrap_or(Platform::Browser),
    entry_file_names: raw_options
      .entry_file_names
      .unwrap_or_else(|| "[name].js".to_string())
      .into(),
    chunk_file_names: raw_options
      .chunk_file_names
      .unwrap_or_else(|| "[name]-[hash].js".to_string())
      .into(),
    banner: raw_options.banner,
    footer: raw_options.footer,
    dir: raw_options.dir.unwrap_or_else(|| "dist".to_string()),
    format: raw_options.format.unwrap_or(crate::OutputFormat::Esm),
    sourcemap: raw_options.sourcemap.unwrap_or(SourceMapType::Hidden),
    shim_missing_exports: raw_options.shim_missing_exports.unwrap_or(false),
  };

  NormalizeOptionsReturn { options: normalized, resolve_options: raw_resolve }
}
