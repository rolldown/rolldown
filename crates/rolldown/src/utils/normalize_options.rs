use std::{borrow::Cow, path::Path};

use oxc::transformer_plugins::InjectGlobalVariablesConfig;
use rolldown_common::{
  AttachDebugInfo, GlobalsOutputOption, InjectImport, LegalComments, MinifyOptions, ModuleType,
  NormalizedBundlerOptions, OutputFormat, Platform, TreeshakeOptions,
  normalize_optimization_option,
};
use rolldown_error::{BuildDiagnostic, InvalidOptionType};
use rustc_hash::{FxHashMap, FxHashSet};

pub struct NormalizeOptionsReturn {
  pub options: NormalizedBundlerOptions,
  pub resolve_options: rolldown_resolver::ResolveOptions,
  pub warnings: Vec<BuildDiagnostic>,
}

fn verify_raw_options(raw_options: &crate::BundlerOptions) -> Vec<BuildDiagnostic> {
  let mut warnings: Vec<BuildDiagnostic> = Vec::new();

  if raw_options.dir.is_some() && raw_options.file.is_some() {
    warnings.push(
      BuildDiagnostic::invalid_option(InvalidOptionType::InvalidOutputDirOption)
        .with_severity_warning(),
    );
  }

  match raw_options.format {
    Some(format @ (OutputFormat::Umd | OutputFormat::Iife)) => {
      if matches!(raw_options.inline_dynamic_imports, Some(false)) {
        warnings.push(
          BuildDiagnostic::invalid_option(InvalidOptionType::UnsupportedInlineDynamicFormat(
            format.to_string(),
          ))
          .with_severity_warning(),
        );
      }
    }
    _ => {}
  }

  if let Some(advanced_chunks) = &raw_options.advanced_chunks {
    let has_groups = advanced_chunks.groups.as_ref().is_some_and(|groups| !groups.is_empty());

    if !has_groups {
      let mut specified_options = Vec::new();
      if advanced_chunks.min_share_count.is_some() {
        specified_options.push("minShareCount".to_string());
      }
      if advanced_chunks.min_size.is_some() {
        specified_options.push("minSize".to_string());
      }
      if advanced_chunks.max_size.is_some() {
        specified_options.push("maxSize".to_string());
      }
      if advanced_chunks.min_module_size.is_some() {
        specified_options.push("minModuleSize".to_string());
      }
      if advanced_chunks.max_module_size.is_some() {
        specified_options.push("maxModuleSize".to_string());
      }
      if advanced_chunks.include_dependencies_recursively.is_some() {
        specified_options.push("includeDependenciesRecursively".to_string());
      }

      if !specified_options.is_empty() {
        warnings.push(
          BuildDiagnostic::invalid_option(InvalidOptionType::AdvancedChunksWithoutGroups(
            specified_options,
          ))
          .with_severity_warning(),
        );
      }
    }
  }

  warnings
}

#[allow(clippy::too_many_lines)] // This function is long, but it's mostly just mapping values
pub fn normalize_options(mut raw_options: crate::BundlerOptions) -> NormalizeOptionsReturn {
  let warnings = verify_raw_options(&raw_options);

  let format = raw_options.format.unwrap_or(crate::OutputFormat::Esm);
  let preserve_entry_signatures = raw_options.preserve_entry_signatures.unwrap_or_default();

  let platform = raw_options.platform.unwrap_or(match format {
    OutputFormat::Cjs => Platform::Node,
    OutputFormat::Esm | OutputFormat::Iife | OutputFormat::Umd => Platform::Browser,
  });

  let minify: MinifyOptions = raw_options.minify.unwrap_or_default().into();

  let mut raw_define = raw_options.define.unwrap_or_default();
  if matches!(platform, Platform::Browser) && !raw_define.contains_key("process.env.NODE_ENV") {
    if minify.is_enabled() {
      raw_define.insert("process.env.NODE_ENV".to_string(), "'production'".to_string());
    } else {
      raw_define.insert("process.env.NODE_ENV".to_string(), "'development'".to_string());
    }
  }

  let define = raw_define.into_iter().collect();

  // Take out resolve options
  let mut raw_resolve = std::mem::take(&mut raw_options.resolve).unwrap_or_default();

  // https://github.com/evanw/esbuild/blob/ea453bf687c8e5cf3c5f11aae372c5ca33be0c98/pkg/api/api_impl.go#L1403-L1405
  // https://github.com/evanw/esbuild/commit/5abe0715f9be662b182989d2f38a44c7c8b28a2d
  if raw_resolve.condition_names.is_none() && matches!(platform, Platform::Browser | Platform::Node)
  {
    raw_resolve.condition_names = Some(vec!["module".to_string()]);
  }

  let mut module_types: FxHashMap<Cow<'static, str>, ModuleType> = FxHashMap::from(
    [
      ("js".into(), ModuleType::Js),
      ("mjs".into(), ModuleType::Js),
      ("cjs".into(), ModuleType::Js),
      ("jsx".into(), ModuleType::Jsx),
      ("ts".into(), ModuleType::Ts),
      ("mts".into(), ModuleType::Ts),
      ("cts".into(), ModuleType::Ts),
      ("tsx".into(), ModuleType::Tsx),
      ("json".into(), ModuleType::Json),
      ("txt".into(), ModuleType::Text),
      ("css".into(), ModuleType::Css),
    ]
    .into_iter()
    .collect(),
  );

  if let Some(user_defined_loaders) = raw_options.module_types {
    user_defined_loaders.into_iter().for_each(|(ext, value)| {
      let stripped = ext.strip_prefix('.').map(ToString::to_string).unwrap_or(ext);
      module_types.insert(Cow::Owned(stripped), value);
    });
  }

  let globals = raw_options.globals.unwrap_or(GlobalsOutputOption::FxHashMap(FxHashMap::default()));

  let oxc_inject_global_variables_config = InjectGlobalVariablesConfig::new(
    raw_options
      .inject
      .as_ref()
      .map(|raw_injects| {
        raw_injects
          .iter()
          .map(|raw| match raw {
            InjectImport::Named { imported, alias, from } => {
              oxc::transformer_plugins::InjectImport::named_specifier(
                from,
                Some(imported),
                alias.as_deref().unwrap_or(imported),
              )
            }
            InjectImport::Namespace { alias, from } => {
              oxc::transformer_plugins::InjectImport::namespace_specifier(from, alias)
            }
          })
          .collect()
      })
      .unwrap_or_default(),
  );

  let mut experimental = raw_options.experimental.unwrap_or_default();
  if experimental.hmr.is_some() {
    experimental.incremental_build = Some(true);
  }

  if experimental.attach_debug_info.is_none() {
    experimental.attach_debug_info = Some(AttachDebugInfo::Simple);
  }

  let inline_dynamic_imports = match format {
    OutputFormat::Umd | OutputFormat::Iife => true,
    _ => raw_options.inline_dynamic_imports.unwrap_or(false),
  };

  // If the `file` is provided, use the parent directory of the file as the `out_dir`.
  // Otherwise, use the `dir` if provided, or default to `dist`.
  let out_dir = raw_options.file.as_ref().map_or_else(
    || raw_options.dir.clone().unwrap_or_else(|| "dist".to_string()),
    |file| {
      Path::new(file.as_str())
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_default()
    },
  );
  let cwd =
    raw_options.cwd.unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir"));

  let mut raw_treeshake = raw_options.treeshake;
  if experimental.hmr.is_some() {
    // HMR requires treeshaking to be disabled
    raw_treeshake = TreeshakeOptions::Boolean(false);
  }

  let normalized = NormalizedBundlerOptions {
    input: raw_options.input.unwrap_or_default(),
    external: raw_options.external.unwrap_or_default(),
    treeshake: raw_treeshake.into_normalized_options(),
    platform,
    name: raw_options.name,
    entry_filenames: raw_options.entry_filenames.unwrap_or_else(|| "[name].js".to_string().into()),
    chunk_filenames: raw_options
      .chunk_filenames
      .unwrap_or_else(|| "[name]-[hash].js".to_string().into()),
    asset_filenames: raw_options
      .asset_filenames
      .unwrap_or_else(|| "assets/[name]-[hash][extname]".to_string().into()),
    css_entry_filenames: raw_options
      .css_entry_filenames
      .unwrap_or_else(|| "[name].css".to_string().into()),
    css_chunk_filenames: raw_options
      .css_chunk_filenames
      .unwrap_or_else(|| "[name]-[hash].css".to_string().into()),
    sanitize_filename: raw_options.sanitize_filename.unwrap_or_default(),
    banner: raw_options.banner,
    footer: raw_options.footer,
    intro: raw_options.intro,
    outro: raw_options.outro,
    es_module: raw_options.es_module.unwrap_or_default(),
    dir: raw_options.dir,
    out_dir,
    file: raw_options.file,
    format,
    exports: raw_options.exports.unwrap_or(crate::OutputExports::Auto),
    hash_characters: raw_options.hash_characters.unwrap_or(crate::HashCharacters::Base64),
    globals,
    sourcemap: raw_options.sourcemap,
    sourcemap_base_url: raw_options.sourcemap_base_url,
    sourcemap_ignore_list: raw_options.sourcemap_ignore_list,
    sourcemap_path_transform: raw_options.sourcemap_path_transform,
    sourcemap_debug_ids: raw_options.sourcemap_debug_ids.unwrap_or(false),
    shim_missing_exports: raw_options.shim_missing_exports.unwrap_or(false),
    module_types,
    experimental,
    // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/bundler/bundler.go#L2767
    profiler_names: raw_options.profiler_names.unwrap_or(!minify.is_enabled()),
    minify,
    define,
    inject: raw_options.inject.unwrap_or_default(),
    oxc_inject_global_variables_config,
    extend: raw_options.extend.unwrap_or(false),
    external_live_bindings: raw_options.external_live_bindings.unwrap_or(true),
    inline_dynamic_imports,
    advanced_chunks: raw_options.advanced_chunks,
    checks: raw_options.checks.unwrap_or_default().into(),
    watch: raw_options.watch.unwrap_or_default(),
    legal_comments: raw_options.legal_comments.unwrap_or(LegalComments::Inline),
    drop_labels: FxHashSet::from_iter(raw_options.drop_labels.unwrap_or_default()),
    keep_names: raw_options.keep_names.unwrap_or_default(),
    polyfill_require: raw_options.polyfill_require.unwrap_or(true),
    defer_sync_scan_data: raw_options.defer_sync_scan_data,
    transform_options: Box::new(raw_options.transform.unwrap_or_default()),
    make_absolute_externals_relative: raw_options
      .make_absolute_externals_relative
      .unwrap_or_default(),
    invalidate_js_side_cache: raw_options.invalidate_js_side_cache,
    mark_module_loaded: raw_options.mark_module_loaded,
    log_level: raw_options.log_level,
    on_log: raw_options.on_log,
    preserve_modules: raw_options.preserve_modules.unwrap_or_default(),
    virtual_dirname: raw_options.virtual_dirname.unwrap_or_else(|| "_virtual".to_string()),
    preserve_modules_root: raw_options.preserve_modules_root.map(|preserve_modules_root| {
      let p = Path::new(&preserve_modules_root);
      if p.is_absolute() {
        preserve_modules_root
      } else {
        cwd.join(p).to_string_lossy().to_string()
      }
    }),
    cwd,
    preserve_entry_signatures,
    debug: raw_options.debug.is_some(),
    optimization: normalize_optimization_option(raw_options.optimization, platform),
    top_level_var: raw_options.top_level_var.unwrap_or(false),
    minify_internal_exports: raw_options.minify_internal_exports.unwrap_or(false),
    context: raw_options.context,
  };

  NormalizeOptionsReturn { options: normalized, resolve_options: raw_resolve, warnings }
}
