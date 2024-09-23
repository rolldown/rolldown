use oxc::minifier::InjectGlobalVariablesConfig;
use rolldown_common::{InjectImport, ModuleType, NormalizedBundlerOptions, Platform};
use rustc_hash::FxHashMap;

pub struct NormalizeOptionsReturn {
  pub options: NormalizedBundlerOptions,
  pub resolve_options: rolldown_resolver::ResolveOptions,
}

#[allow(clippy::too_many_lines)] // This function is long, but it's mostly just mapping values
pub fn normalize_options(mut raw_options: crate::BundlerOptions) -> NormalizeOptionsReturn {
  // Take out resolve options
  let platform = raw_options.platform.unwrap_or(Platform::Browser);
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
      ("css".to_string(), ModuleType::Css),
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

  let globals: FxHashMap<String, String> =
    raw_options.globals.map(|globals| globals.into_iter().collect()).unwrap_or_default();

  let oxc_inject_global_variables_config = InjectGlobalVariablesConfig::new(
    raw_options
      .inject
      .as_ref()
      .map(|raw_injects| {
        raw_injects
          .iter()
          .map(|raw| match raw {
            InjectImport::Named { imported, alias, from } => {
              oxc::minifier::InjectImport::named_specifier(
                from,
                Some(imported),
                alias.as_deref().unwrap_or(imported),
              )
            }
            InjectImport::Namespace { alias, from } => {
              oxc::minifier::InjectImport::namespace_specifier(from, alias)
            }
          })
          .collect()
      })
      .unwrap_or_default(),
  );

  let normalized = NormalizedBundlerOptions {
    input: raw_options.input.unwrap_or_default(),
    cwd: raw_options
      .cwd
      .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir")),
    external: raw_options.external,
    treeshake: raw_options.treeshake,
    platform,
    name: raw_options.name,
    entry_filenames: raw_options.entry_filenames.unwrap_or_else(|| "[name].js".to_string().into()),
    chunk_filenames: raw_options
      .chunk_filenames
      .unwrap_or_else(|| "[name]-[hash].js".to_string().into()),
    asset_filenames: raw_options
      .asset_filenames
      .unwrap_or_else(|| "assets/[name]-[hash][extname]".to_string())
      .into(),
    css_entry_filenames: raw_options
      .css_entry_filenames
      .unwrap_or_else(|| "[name].css".to_string().into()),
    css_chunk_filenames: raw_options
      .css_chunk_filenames
      .unwrap_or_else(|| "[name]-[hash].css".to_string().into()),
    banner: raw_options.banner,
    footer: raw_options.footer,
    intro: raw_options.intro,
    outro: raw_options.outro,
    es_module: raw_options.es_module.unwrap_or_default(),
    dir: raw_options.dir.unwrap_or_else(|| "dist".to_string()),
    format: raw_options.format.unwrap_or(crate::OutputFormat::Esm),
    exports: raw_options.exports.unwrap_or(crate::OutputExports::Auto),
    globals,
    sourcemap: raw_options.sourcemap,
    sourcemap_ignore_list: raw_options.sourcemap_ignore_list,
    sourcemap_path_transform: raw_options.sourcemap_path_transform,
    shim_missing_exports: raw_options.shim_missing_exports.unwrap_or(false),
    module_types: loaders,
    experimental: raw_options.experimental.unwrap_or_default(),
    minify: raw_options.minify.unwrap_or(false),
    define: raw_options.define.map(|inner| inner.into_iter().collect()).unwrap_or_default(),
    inject: raw_options.inject.unwrap_or_default(),
    oxc_inject_global_variables_config,
    extend: raw_options.extend.unwrap_or(false),
    external_live_bindings: raw_options.external_live_bindings.unwrap_or(true),
    inline_dynamic_imports: raw_options.inline_dynamic_imports.unwrap_or(false),
    advanced_chunks: raw_options.advanced_chunks,
    checks: raw_options.checks.unwrap_or_default(),
  };

  NormalizeOptionsReturn { options: normalized, resolve_options: raw_resolve }
}
