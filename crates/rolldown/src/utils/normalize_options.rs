use rolldown_resolver::EnforceExtension;

use crate::options::{
  normalized_input_options::NormalizedInputOptions,
  normalized_output_options::NormalizedOutputOptions, output_options::SourceMapType,
};

#[allow(clippy::struct_field_names)]
pub struct NormalizeOptionsReturn {
  pub input_options: NormalizedInputOptions,
  pub output_options: NormalizedOutputOptions,
  pub resolve_options: rolldown_resolver::ResolveOptions,
}

pub fn normalize_options(
  mut raw_input: crate::InputOptions,
  raw_output: crate::OutputOptions,
) -> NormalizeOptionsReturn {
  let raw_resolve = std::mem::take(&mut raw_input.resolve).unwrap_or_default();

  // So far we align the default behavior with esbuild that target browser platform;
  let resolve_options = rolldown_resolver::ResolveOptions {
    tsconfig: None,
    alias: raw_resolve
      .alias
      .map(|alias| {
        alias
          .into_iter()
          .map(|(key, value)| {
            (key, value.into_iter().map(rolldown_resolver::AliasValue::Path).collect::<Vec<_>>())
          })
          .collect::<Vec<_>>()
      })
      .unwrap_or_default(),
    alias_fields: raw_resolve.alias_fields.unwrap_or_default(),
    condition_names: raw_resolve.condition_names.unwrap_or_else(|| {
      ["import", "default", "require"].into_iter().map(str::to_string).collect()
    }),
    description_files: vec!["package.json".to_string()],
    enforce_extension: EnforceExtension::Auto,
    exports_fields: raw_resolve.exports_fields.unwrap_or_else(|| vec![vec!["exports".to_string()]]),
    extension_alias: vec![],
    extensions: raw_resolve.extensions.unwrap_or_else(|| {
      [".tsx", ".ts", ".jsx", ".js", ".json"].into_iter().map(str::to_string).collect()
    }),
    fallback: vec![],
    fully_specified: false,
    main_fields: raw_resolve
      .main_fields
      .unwrap_or_else(|| vec!["browser".to_string(), "module".to_string(), "main".to_string()]),
    main_files: raw_resolve.main_files.unwrap_or_else(|| vec!["index".to_string()]),
    modules: raw_resolve.modules.unwrap_or_else(|| vec!["node_modules".to_string()]),
    resolve_to_context: false,
    prefer_relative: false,
    prefer_absolute: false,
    restrictions: vec![],
    roots: vec![],
    symlinks: raw_resolve.symlinks.unwrap_or(true),
    builtin_modules: false,
  };

  // Normalize input options

  let input_options = NormalizedInputOptions {
    input: raw_input.input,
    cwd: raw_input
      .cwd
      .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir")),
    external: raw_input.external.unwrap_or_default(),
    treeshake: raw_input.treeshake.unwrap_or(true),
  };

  // Normalize output options

  let output_options: NormalizedOutputOptions = NormalizedOutputOptions {
    entry_file_names: raw_output.entry_file_names.unwrap_or_else(|| "[name].js".to_string()).into(),
    chunk_file_names: raw_output
      .chunk_file_names
      .unwrap_or_else(|| "[name]-[hash].js".to_string())
      .into(),
    banner: raw_output.banner.unwrap_or_default(),
    footer: raw_output.footer.unwrap_or_default(),
    dir: raw_output.dir.unwrap_or_else(|| "dist".to_string()),
    format: raw_output.format.unwrap_or(crate::OutputFormat::Esm),
    sourcemap: raw_output.sourcemap.unwrap_or(SourceMapType::Hidden),
  };

  NormalizeOptionsReturn { input_options, output_options, resolve_options }
}
