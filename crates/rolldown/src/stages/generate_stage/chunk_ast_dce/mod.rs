//! Chunk-level optimization on an AST assembled from finalized module ASTs and generated glue.
//!
//! Flat chunks optimize before their preliminary render. Wrapped, inline, dev, and complete-text
//! chunks retain a complete program until hooks finish. `minify_chunks` reparses only actual text
//! changed by `renderChunk` or arbitrary executable addons; it never falls back to the original
//! module-printing pipeline. See internal-docs/chunk-ast-minify/implementation.md.

use oxc::{
  allocator::{Allocator, Vec as ArenaVec},
  ast::ast::{Program, Statement},
  minifier::MinifierOptions as OxcMinifierOptions,
  parser::{ParseOptions, Parser},
  span::SourceType,
};
use rolldown_common::{AddonOutputOption, MinifyOptions, NormalizedBundlerOptions, OutputFormat};
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::concat_string;

use crate::{
  ecmascript::{
    format_cjs::{render_cjs_chunk_imports, render_cjs_namespace_markers},
    format_esm::{render_esm_chunk_imports, render_esm_external_star_exports},
  },
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::{render_chunk_exports, render_wrapped_entry_chunk},
};

#[cfg(feature = "testing")]
mod coverage;
mod global;
mod owned;

#[cfg(feature = "testing")]
pub(super) use coverage::record as record_coverage;

macro_rules! reject {
  ($condition:expr, $options:expr, $stage:literal, $reason:literal, $chunk:expr, $result:expr) => {
    if $condition {
      #[cfg(feature = "testing")]
      coverage::record($options, $stage, $reason, $chunk);
      return $result;
    }
  };
}

pub use global::dedupe_sources;
pub use owned::PendingChunk;
pub(super) use owned::{ChunkAst, PreparedChunk};

#[derive(Default)]
pub(super) struct DeferredChunks {
  pub pending: rustc_hash::FxHashMap<rolldown_common::ChunkIdx, PendingChunk>,
  pub replaced: rustc_hash::FxHashSet<rolldown_common::ChunkIdx>,
  pub rendered_text: rustc_hash::FxHashSet<rolldown_common::ChunkIdx>,
}

/// Retain finalized inline copies for the direct AST input path.
pub(super) fn retain_inline_asts(
  options: &NormalizedBundlerOptions,
  plugin_driver: &SharedPluginDriver,
) -> bool {
  !complete_program(options, plugin_driver) && !matches!(options.minify, MinifyOptions::Disabled)
}

pub fn complete_program(
  options: &NormalizedBundlerOptions,
  plugin_driver: &SharedPluginDriver,
) -> bool {
  options.intro.is_some()
    || options.outro.is_some()
    || !addon_is_comment_only(options.banner.as_ref())
    || !addon_is_comment_only(options.footer.as_ref())
    || !plugin_driver.order_by_intro_meta.is_empty()
    || !plugin_driver.order_by_outro_meta.is_empty()
    || !plugin_driver.order_by_banner_meta.is_empty()
    || !plugin_driver.order_by_footer_meta.is_empty()
}

pub(super) fn defer_for_render_hooks(ctx: &GenerateContext<'_>) -> bool {
  complete_program(ctx.options, ctx.plugin_driver)
    || user_render_chunk_plugins(ctx.plugin_driver) != 0
    || ctx.chunk.modules.iter().any(|idx| {
      ctx.link_output.module_table[*idx]
        .as_normal()
        .is_some_and(|module| has_emitted_file_placeholder(&module.source))
    })
    || has_emitted_file_placeholder(&render_cjs_or_esm_imports(ctx))
    || render_esm_external_star_exports(ctx).as_deref().is_some_and(has_emitted_file_placeholder)
    || render_wrapped_entry_chunk(
      ctx,
      matches!(ctx.options.format, OutputFormat::Cjs).then_some(&ctx.chunk.output_exports),
    )
    .as_deref()
    .is_some_and(has_emitted_file_placeholder)
    || render_chunk_exports(
      ctx,
      matches!(ctx.options.format, OutputFormat::Cjs).then_some(&ctx.chunk.output_exports),
    )
    .as_deref()
    .is_some_and(has_emitted_file_placeholder)
}

fn render_cjs_or_esm_imports(ctx: &GenerateContext<'_>) -> String {
  if matches!(ctx.options.format, OutputFormat::Cjs) {
    render_cjs_chunk_imports(ctx)
  } else {
    render_esm_chunk_imports(ctx).unwrap_or_default()
  }
}

/// The generated head/tail of an optimized chunk.
#[derive(Debug, Default)]
pub struct ChunkAstDceOutput {
  /// Replaces `render_esm_chunk_imports`.
  pub imports: Option<String>,
  /// Replaces `render_wrapped_entry_chunk` + `render_chunk_exports`.
  pub exports: Option<String>,
  /// `global` mode: maps of `imports`/`exports`, which can hold code moved out of modules.
  pub imports_map: Option<rolldown_sourcemap::SourceMap>,
  pub exports_map: Option<rolldown_sourcemap::SourceMap>,
  /// `global` mode: the chunk's shared source space; modules are printed against it.
  pub space: Option<std::sync::Arc<global::ChunkSourceSpace>>,
  pub property_mangle_cache: Option<oxc::minifier::ManglePropertyCache>,
}

/// The chunk's generated import and export text, as `render_esm` would emit it.
pub struct GeneratedText<'a> {
  /// Statements before runtime initialization, such as CJS namespace markers.
  pub prefix: Option<&'a str>,
  pub imports: Option<&'a str>,
  pub exports: Option<&'a str>,
  /// CJS requires run after the runtime helper module, before user modules.
  pub imports_after_runtime: bool,
}

/// Returns the optimizer options unless this build explicitly disables minification.
pub fn build_minify_options(options: &NormalizedBundlerOptions) -> Option<&OxcMinifierOptions> {
  let minify_options = match &options.minify {
    MinifyOptions::DeadCodeEliminationOnly(minify_options) => minify_options,
    MinifyOptions::Enabled(minify) => &minify.options,
    MinifyOptions::Disabled => {
      #[cfg(feature = "testing")]
      coverage::record(options, "build", "minify_disabled", None);
      return None;
    }
  };
  #[cfg(feature = "testing")]
  coverage::record(options, "build", "eligible", None);
  Some(minify_options)
}

fn addon_is_comment_only(addon: Option<&AddonOutputOption>) -> bool {
  match addon {
    None | Some(AddonOutputOption::String(None)) => true,
    Some(AddonOutputOption::String(Some(text))) => {
      let allocator = Allocator::default();
      let ret = Parser::new(&allocator, text, SourceType::mjs()).parse();
      !ret.fatal_error
        && ret.diagnostics.is_empty()
        && ret.program.body.is_empty()
        && ret.program.directives.is_empty()
        && ret.program.hashbang.is_none()
    }
    Some(AddonOutputOption::Fn(_)) => false,
  }
}

/// `renderChunk` plugins other than the builtin asset/copy plugins (chunks with emitted-file
/// placeholders are excluded by `chunk_eligible`) and the test harness's region-marker plugin,
/// which only rewrites comment lines. Concrete identity prevents custom plugins from bypassing
/// the gate by using one of their names.
fn user_render_chunk_plugins(plugin_driver: &SharedPluginDriver) -> usize {
  plugin_driver
    .order_by_render_chunk_meta
    .iter()
    .filter(|idx| !render_chunk_plugin_is_safe(&plugin_driver.plugins()[**idx]))
    .count()
}

fn render_chunk_plugin_is_safe(plugin: &rolldown_plugin::Pluginable) -> bool {
  if plugin.is::<rolldown_plugin_asset_module::AssetModulePlugin>()
    || plugin.is::<rolldown_plugin_copy_module::CopyModulePlugin>()
  {
    return true;
  }
  #[cfg(feature = "testing")]
  if plugin.is::<rolldown_plugin_utils::hidden_module_markers::HiddenModuleMarkersPlugin>() {
    return true;
  }
  false
}

/// Declaration chunks have no executable program to retain.
pub fn chunk_eligible(ctx: &GenerateContext<'_>) -> bool {
  let filename =
    ctx.chunk.preliminary_filename.as_deref().map(arcstr::ArcStr::as_str).unwrap_or_default();
  reject!(
    filename.ends_with(".d.ts") || filename.ends_with(".d.cts") || filename.ends_with(".d.mts"),
    ctx.options,
    "chunk",
    "declaration",
    Some(filename),
    false
  );
  #[cfg(feature = "testing")]
  coverage::record(ctx.options, "chunk", "eligible", Some(filename));
  true
}

fn has_emitted_file_placeholder(text: &str) -> bool {
  text.contains("__ROLLDOWN_ASSET__#") || text.contains("__ROLLDOWN_COPY_MODULE__#")
}

/// Prepares roots exclusively owned by `ChunkAst`. Its intact donor cells and stable scratch
/// arena outlive all roots, including unwinding. No mixed program is put back into `EcmaAst`.
fn generated_text<'a>(ctx: &GenerateContext<'_>, allocator: &'a Allocator) -> GeneratedText<'a> {
  let is_cjs = matches!(ctx.options.format, OutputFormat::Cjs);
  let imports = if is_cjs {
    Some(render_cjs_chunk_imports(ctx))
  } else {
    match (render_esm_chunk_imports(ctx), render_esm_external_star_exports(ctx)) {
      (None, None) => None,
      (imports, stars) => {
        Some(concat_string!(imports.unwrap_or_default(), stars.unwrap_or_default()))
      }
    }
  }
  .map(|code| allocator.alloc_str(&code) as &str);
  let export_mode = is_cjs.then_some(&ctx.chunk.output_exports);
  let exports =
    match (render_wrapped_entry_chunk(ctx, export_mode), render_chunk_exports(ctx, export_mode)) {
      (None, None) => None,
      (entry, exports) => Some(
        allocator.alloc_str(&concat_string!(entry.unwrap_or_default(), exports.unwrap_or_default()))
          as &str,
      ),
    };
  GeneratedText {
    prefix: if is_cjs { render_cjs_namespace_markers(ctx) } else { None },
    imports,
    exports,
    imports_after_runtime: is_cjs,
  }
}

fn dce_chunk(
  ctx: &GenerateContext<'_>,
  allocator: &Allocator,
  asts: &mut [owned::ChunkProgram],
  assembled: &mut Option<owned::ChunkProgram>,
  source_text: &mut Option<std::sync::Arc<str>>,
  minify_options: &OxcMinifierOptions,
) -> ChunkAstDceOutput {
  #[cfg(feature = "testing")]
  coverage::record(
    ctx.options,
    "execute",
    "global",
    ctx.chunk.preliminary_filename.as_deref().map(arcstr::ArcStr::as_str),
  );
  let generated = generated_text(ctx, allocator);
  let modules: Vec<_> = ctx
    .chunk
    .modules
    .iter()
    .filter_map(|idx| ctx.link_output.module_table[*idx].as_normal())
    .collect();
  let (program, layout) =
    global::assemble_chunk(ctx, allocator, asts, &modules, &generated, source_text)
      .expect("flat generated glue has no parsed wrapper");
  // SAFETY: the complete program is private to ChunkAst, whose stable donors, scratch allocator
  // and source text outlive both assembled and split roots, including unwinding.
  *assembled = Some(owned::ChunkProgram(unsafe {
    std::mem::transmute::<Program<'_>, Program<'static>>(program)
  }));
  let output = global::optimize_chunk(
    &global::OptimizeContext {
      options: ctx.options,
      module_table: &ctx.link_output.module_table,
      #[cfg(feature = "testing")]
      filename: ctx.chunk.preliminary_filename.as_deref().map(arcstr::ArcStr::as_str),
    },
    allocator,
    asts,
    // SAFETY: shorten the private root's erased lifetime to the stable scratch borrow.
    // The optimizer cannot return AST references; ChunkAst retains every backing owner.
    unsafe {
      std::mem::transmute::<&mut Program<'static>, &mut Program<'_>>(
        &mut assembled.as_mut().expect("assembled chunk program").0,
      )
    },
    layout,
    minify_options,
  );
  *assembled = None;
  output
}

pub(super) fn parse_generated<'a>(
  allocator: &'a Allocator,
  text: &'a str,
  source_type: SourceType,
  body: &mut ArenaVec<'a, Statement<'a>>,
) {
  let ret = Parser::new(allocator, text, source_type)
    .with_options(ParseOptions { preserve_parens: false, ..ParseOptions::default() })
    .parse();
  assert!(
    !ret.fatal_error && ret.diagnostics.is_empty(),
    "chunk AST DCE: generated import/export text failed to parse: {text}"
  );
  body.extend(ret.program.body);
}

#[cfg(test)]
mod tests {
  use rolldown_plugin::Plugin;

  use super::render_chunk_plugin_is_safe;

  #[test]
  fn builtin_render_chunk_plugins_are_safe_by_concrete_identity() {
    let module_types = rustc_hash::FxHashMap::default();
    let asset =
      Plugin::new_shared(rolldown_plugin_asset_module::AssetModulePlugin::new(&module_types));
    let copy =
      Plugin::new_shared(rolldown_plugin_copy_module::CopyModulePlugin::new(&module_types));
    assert!(render_chunk_plugin_is_safe(&asset));
    assert!(render_chunk_plugin_is_safe(&copy));
  }

  #[cfg(feature = "testing")]
  #[test]
  fn harness_render_chunk_plugin_is_safe_by_concrete_identity() {
    let marker =
      Plugin::new_shared(rolldown_plugin_utils::hidden_module_markers::HiddenModuleMarkersPlugin {
        hide_runtime_module: true,
      });
    assert!(render_chunk_plugin_is_safe(&marker));
  }
}
