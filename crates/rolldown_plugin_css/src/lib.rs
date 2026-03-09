use std::{borrow::Cow, sync::Arc};

use arcstr::ArcStr;
use rolldown_common::{EmittedAsset, ModuleType, StrOrBytes, side_effects::HookSideEffects};
use rolldown_plugin::{HookTransformOutput, HookUsage, Plugin, PluginContext};
use rolldown_plugin_utils::css::is_css_module;
use rolldown_sourcemap::SourceMap;
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};

pub mod at_rule_hoisting;
pub mod css_modules;
pub mod generate_bundle;
pub mod import_inlining;
pub mod minification;
pub mod preprocessors;
pub mod sourcemap;
pub mod url_rewriting;

pub use css_modules::CssModulesExportsCache;
pub use url_rewriting::UrlPlaceholders;

// ---------------------------------------------------------------------------
// Shared state types stored in `PluginContextMeta`
// ---------------------------------------------------------------------------

/// Per-module CSS content, keyed by module ID.
#[derive(Debug, Default)]
pub struct CssStylesCache {
  pub inner: FxDashMap<String, String>,
}

/// Set of chunk filenames that are "pure CSS" (contain only CSS modules, no JS exports).
#[derive(Debug, Default)]
pub struct PureCssChunks {
  pub inner: FxDashSet<ArcStr>,
}

/// Mapping from chunk filename to the emitted CSS asset reference ID.
#[derive(Debug, Default)]
pub struct ChunkCssMap {
  pub inner: FxDashMap<ArcStr, ArcStr>,
}

/// Ordered collection of (chunk_filename, css_content) for non-code-split mode.
#[derive(Debug, Default)]
pub struct AccumulatedCss {
  pub inner: parking_lot::Mutex<Vec<(ArcStr, String)>>,
}

// ---------------------------------------------------------------------------
// Plugin options
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct CssPluginOptions {
  /// Emit per-chunk `.css` files (true) or accumulate into a single bundle (false).
  pub code_split: bool,
  /// Minify the emitted CSS.
  pub minify: bool,
  /// Generate source maps for CSS output.
  pub sourcemap: bool,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

#[derive(derive_more::Debug)]
pub struct CssPlugin {
  pub options: CssPluginOptions,
}

impl CssPlugin {
  pub fn new(options: CssPluginOptions) -> Self {
    Self { options }
  }

  /// Finalize CSS content before emission.
  ///
  /// Pipeline: url() placeholder replacement → @charset/@import hoisting → minification (if enabled).
  ///
  /// When `with_sourcemap` is true, the minification step produces a source map that maps
  /// minified output back to the pre-minified CSS. Callers should chain this with any
  /// per-module source maps using `collapse_sourcemaps`.
  fn finalize_css(
    &self,
    css: &str,
    ctx: &PluginContext,
    url_deps: &[url_rewriting::ResolvedUrlDep],
    placeholders: &UrlPlaceholders,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
    with_sourcemap: bool,
  ) -> anyhow::Result<(String, Option<SourceMap>)> {
    // Step 1: Replace url() placeholders with output-relative paths
    let mut result = if url_deps.is_empty() {
      css.to_owned()
    } else {
      let css_filename =
        format!("{}.css", args.chunk.filename.strip_suffix(".js").unwrap_or(&args.chunk.filename));
      let css_output_dir = std::path::Path::new(&css_filename)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

      url_rewriting::replace_url_placeholders(css, placeholders, url_deps, ctx, &css_output_dir)?
    };

    // Step 2: Hoist @charset and @import rules to the top
    result = at_rule_hoisting::hoist_at_rules(&result);

    // Step 3: Minify if enabled
    if self.options.minify {
      if with_sourcemap {
        let css_filename = format!(
          "{}.css",
          args.chunk.filename.strip_suffix(".js").unwrap_or(&args.chunk.filename)
        );
        let (minified, min_map) = sourcemap::minify_css_with_sourcemap(&result, &css_filename)?;
        return Ok((minified, Some(min_map)));
      }
      result = minification::minify_css(&result)?;
    }

    Ok((result, None))
  }
}

impl Plugin for CssPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:css")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
      | HookUsage::Transform
      | HookUsage::RenderChunk
      | HookUsage::AugmentChunkHash
      | HookUsage::GenerateBundle
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    // Initialize shared state caches
    ctx.meta().insert(Arc::new(CssStylesCache::default()));
    ctx.meta().insert(Arc::new(PureCssChunks::default()));
    ctx.meta().insert(Arc::new(ChunkCssMap::default()));
    ctx.meta().insert(Arc::new(AccumulatedCss::default()));
    url_rewriting::init_url_placeholders(ctx);
    Ok(())
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    // Detect preprocessor files by extension (.scss, .sass, .less, .styl, .stylus)
    // These may not have ModuleType::Css, so check extension first.
    let preprocessor_lang = preprocessors::PreprocessorLang::from_path(args.id);

    if *args.module_type != ModuleType::Css && preprocessor_lang.is_none() {
      return Ok(None);
    }

    // If this is a preprocessor file, compile to CSS first
    let css_source = if let Some(lang) = preprocessor_lang {
      let result = preprocessors::compile(lang, args.code.as_str(), args.id)?;

      // Register preprocessor dependencies for watch mode
      for dep in &result.dependencies {
        ctx.add_watch_file(dep);
      }

      result.css
    } else {
      args.code.clone()
    };

    // Inline @import statements before caching
    let inline_result = import_inlining::inline_imports_from_code(args.id, &css_source)?;
    let css_code = inline_result.code;

    // Register imported files for watch mode
    for dep_path in &inline_result.dependencies {
      if let Some(path_str) = dep_path.to_str() {
        ctx.add_watch_file(path_str);
      }
    }

    // Handle CSS modules (.module.css, .module.scss, .module.less, etc.)
    let is_module = is_css_module(args.id) || preprocessors::is_preprocessor_css_module(args.id);
    if is_module {
      let module_result = css_modules::transform_css_module(args.id, &css_code)?;

      // Store the transformed CSS (with hashed class names) in the cache
      let cache = ctx
        .meta()
        .get::<CssStylesCache>()
        .ok_or_else(|| anyhow::anyhow!("CssStylesCache missing — was build_start called?"))?;
      cache.inner.insert(args.id.to_owned(), module_result.code);

      return Ok(Some(HookTransformOutput {
        code: Some(module_result.js_proxy),
        side_effects: Some(HookSideEffects::False),
        module_type: Some(ModuleType::Js),
        ..Default::default()
      }));
    }

    // Store the processed CSS (with imports inlined) in the cache
    let cache = ctx
      .meta()
      .get::<CssStylesCache>()
      .ok_or_else(|| anyhow::anyhow!("CssStylesCache missing — was build_start called?"))?;
    cache.inner.insert(args.id.to_owned(), css_code);

    // Plain CSS has side effects (no tree-shaking)
    let side_effects = HookSideEffects::NoTreeshake;

    // Return a JS proxy module so the bundler can track the dependency
    let js_proxy =
      format!("// CSS proxy module for {id}\nexport default undefined;\n", id = args.id);

    Ok(Some(HookTransformOutput {
      code: Some(js_proxy),
      side_effects: Some(side_effects),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }

  async fn render_chunk(
    &self,
    ctx: &PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    let cache = ctx
      .meta()
      .get::<CssStylesCache>()
      .ok_or_else(|| anyhow::anyhow!("CssStylesCache missing"))?;

    // Collect CSS for modules in this chunk, preserving module order
    let mut chunk_css_parts: Vec<(String, String)> = Vec::new(); // (module_id, css)
    let mut css_module_count: usize = 0;

    for module_id in &args.chunk.module_ids {
      let id_str = module_id.as_str();
      if let Some(css) = cache.inner.get(id_str) {
        chunk_css_parts.push((id_str.to_owned(), css.clone()));
        css_module_count += 1;
      }
    }

    if chunk_css_parts.is_empty() {
      return Ok(None);
    }

    // Extract url() dependencies and replace with placeholders per module,
    // then emit the referenced assets.
    let placeholders = ctx
      .meta()
      .get::<UrlPlaceholders>()
      .ok_or_else(|| anyhow::anyhow!("UrlPlaceholders missing"))?;

    let mut all_url_deps = Vec::new();
    let mut processed_parts: Vec<String> = Vec::new();

    for (module_id, css) in &chunk_css_parts {
      let (processed_css, url_deps) = url_rewriting::extract_url_dependencies(css, module_id)?;
      url_rewriting::emit_url_assets(ctx, &url_deps, &placeholders)?;
      all_url_deps.extend(url_deps);
      processed_parts.push(processed_css);
    }

    // When source maps are enabled, generate per-module source maps and join them.
    // Otherwise, simply concatenate the CSS strings.
    let (combined_css, concat_map) = if self.options.sourcemap {
      let mut parts_with_maps = Vec::with_capacity(processed_parts.len());
      for (i, css_code) in processed_parts.into_iter().enumerate() {
        let module_id = &chunk_css_parts[i].0;
        let (_reparsed_code, sm) = sourcemap::parse_css_with_sourcemap(&css_code, module_id)?;
        // Use the original css_code (not reparsed) to preserve url() placeholders,
        // since parse_css_with_sourcemap may re-serialize and lose them.
        parts_with_maps.push((css_code, sm));
      }
      let (joined_css, joined_map) = sourcemap::join_css_sourcemaps(parts_with_maps);
      (joined_css, joined_map)
    } else {
      (processed_parts.join("\n"), None)
    };

    let (finalized_css, minify_map) = self.finalize_css(
      &combined_css,
      ctx,
      &all_url_deps,
      &placeholders,
      args,
      self.options.sourcemap,
    )?;

    // Build the final source map by chaining concat_map and minify_map (if present).
    let final_sourcemap = if self.options.sourcemap {
      match (concat_map, minify_map) {
        (Some(concat), Some(minify)) => {
          Some(rolldown_sourcemap::collapse_sourcemaps(&[&concat, &minify]))
        }
        (Some(map), None) | (None, Some(map)) => Some(map),
        (None, None) => None,
      }
    } else {
      None
    };

    // Detect pure CSS chunks: all modules are CSS, no JS exports
    let is_pure_css =
      css_module_count == args.chunk.module_ids.len() && args.chunk.exports.is_empty();

    if is_pure_css {
      let pure_css_chunks = ctx
        .meta()
        .get::<PureCssChunks>()
        .ok_or_else(|| anyhow::anyhow!("PureCssChunks missing"))?;
      pure_css_chunks.inner.insert(args.chunk.filename.clone());
    }

    if self.options.code_split {
      // Emit a per-chunk .css asset file
      let css_filename =
        format!("{}.css", args.chunk.filename.strip_suffix(".js").unwrap_or(&args.chunk.filename));

      // Append sourceMappingURL comment and emit .css.map if sourcemap is enabled
      let css_output = if let Some(map) = &final_sourcemap {
        let map_filename = format!("{css_filename}.map");
        let map_json = map.to_json_string();

        ctx.emit_file(
          EmittedAsset {
            name: Some(map_filename.clone()),
            original_file_name: None,
            file_name: Some(ArcStr::from(&map_filename)),
            source: StrOrBytes::Str(map_json),
          },
          None,
          None,
        )?;

        let map_basename =
          std::path::Path::new(&map_filename).file_name().unwrap().to_string_lossy();
        sourcemap::append_sourcemap_url(&finalized_css, &map_basename)
      } else {
        finalized_css
      };

      let reference_id = ctx.emit_file(
        EmittedAsset {
          name: Some(css_filename.clone()),
          original_file_name: None,
          file_name: Some(ArcStr::from(&css_filename)),
          source: StrOrBytes::Str(css_output),
        },
        None,
        None,
      )?;

      let chunk_css_map =
        ctx.meta().get::<ChunkCssMap>().ok_or_else(|| anyhow::anyhow!("ChunkCssMap missing"))?;
      chunk_css_map.inner.insert(args.chunk.filename.clone(), reference_id);
    } else {
      // Accumulate CSS for single-bundle mode
      let accumulated = ctx
        .meta()
        .get::<AccumulatedCss>()
        .ok_or_else(|| anyhow::anyhow!("AccumulatedCss missing"))?;
      accumulated.inner.lock().push((args.chunk.filename.clone(), finalized_css));
    }

    Ok(None)
  }

  async fn augment_chunk_hash(
    &self,
    ctx: &PluginContext,
    chunk: Arc<rolldown_common::RollupRenderedChunk>,
  ) -> rolldown_plugin::HookAugmentChunkHashReturn {
    let cache = ctx
      .meta()
      .get::<CssStylesCache>()
      .ok_or_else(|| anyhow::anyhow!("CssStylesCache missing"))?;

    // Include CSS content in the chunk hash so CSS changes invalidate the chunk
    let mut hasher_input = String::new();
    for module_id in &chunk.module_ids {
      if let Some(css) = cache.inner.get(module_id.as_str()) {
        hasher_input.push_str(&css);
      }
    }

    if hasher_input.is_empty() { Ok(None) } else { Ok(Some(hasher_input)) }
  }

  async fn generate_bundle(
    &self,
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    generate_bundle::prune_pure_css_chunks(ctx, args);

    // In single-bundle mode, collect all CSS in chunk order and emit style.css
    if !self.options.code_split {
      generate_bundle::emit_single_css_bundle(ctx, args)?;
    }

    Ok(())
  }
}
