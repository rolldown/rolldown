//! PoC (`poc/chunk-ast-minify`, see `POC_REPORT.md`): chunk-level DCE on an AST assembled from
//! the finalized module ASTs, instead of printing every module, joining the text and re-parsing
//! the whole chunk in `minify_chunks`.
//!
//! Enabled with `ROLLDOWN_CHUNK_AST_DCE=1`. For an eligible chunk, before modules are printed:
//!
//! 1. Every module's top-level statements are moved (not copied) into one chunk `Program` that
//!    lives in a fresh allocator. The nodes stay in their module arenas. An `export {}` marker
//!    with a sentinel span follows each module, so the body can be split again afterwards.
//!    `export {}` is declarative (does not run code), has no `from`, and has no specifiers, so
//!    oxc's DCE neither removes it nor merges anything into it; it also keeps adjacent-statement
//!    rewrites from crossing a module boundary.
//! 2. The chunk's generated imports and exports are parsed from the text rolldown would emit,
//!    and put at the head and tail of the program, so the chunk exports are roots for DCE and
//!    imported bindings resolve.
//! 3. `Minifier::dce` runs on the chunk program (one `SemanticBuilder` + compressor pass).
//! 4. The body is split at the markers and each segment goes back into its module's `Program`.
//!    The head (imports) and tail (exports) are printed by oxc codegen and replace the text
//!    `render_esm` would have generated.
//!
//! Then each module is printed as today, with its own source text, comments and sourcemap, and
//! joined by `SourceJoiner`. `minify_chunks` skips the chunk.

use std::sync::LazyLock;

use oxc::{
  allocator::{Address, Allocator, GetAddress, TakeIn, Vec as ArenaVec},
  ast::{
    ast::{ImportOrExportKind, Program, Statement},
    builder::AstBuilder,
  },
  codegen::{Codegen, CodegenOptions, CommentOptions},
  minifier::{Minifier, MinifierOptions as OxcMinifierOptions},
  parser::{ParseOptions, Parser},
  span::{SPAN, SourceType, Span},
};
use rolldown_common::{
  AddonOutputOption, ChunkKind, MinifyOptions, NormalizedBundlerOptions, OutputFormat, WrapKind,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_plugin::SharedPluginDriver;
use rolldown_utils::concat_string;
use rolldown_utils::poc_prof::{Phase, Timer};
use rustc_hash::FxHashMap;

use crate::{
  ecmascript::format_esm::render_esm_chunk_imports,
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::{render_chunk_exports, render_wrapped_entry_chunk},
};

use super::inline_common_chunks::InlineCommonChunksState;

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
pub(super) use owned::{ChunkAst, PreparedChunk};

static ENABLED: LazyLock<bool> = LazyLock::new(|| {
  std::env::var_os("ROLLDOWN_CHUNK_AST_DCE")
    .is_some_and(|v| v == "1" || v == "nobarrier" || v == "nobarrier-unsafe" || v == "global")
});

/// `global`: no barrier, spans rebased into one per-chunk source space; see `global`.
static GLOBAL: LazyLock<bool> =
  LazyLock::new(|| std::env::var_os("ROLLDOWN_CHUNK_AST_DCE").is_some_and(|v| v == "global"));

static NO_BARRIER: LazyLock<bool> = LazyLock::new(|| {
  std::env::var_os("ROLLDOWN_CHUNK_AST_DCE")
    .is_some_and(|v| v == "nobarrier" || v == "nobarrier-unsafe")
});

/// `nobarrier-unsafe` drops the barrier even with sourcemaps on, to measure the damage.
static NO_BARRIER_WITH_MAPS: LazyLock<bool> = LazyLock::new(|| {
  std::env::var_os("ROLLDOWN_CHUNK_AST_DCE").is_some_and(|v| v == "nobarrier-unsafe")
});

/// Span start of the segment markers. No real node starts at `u32::MAX`.
const MARKER_POS: u32 = u32::MAX;

/// The generated head/tail of a chunk whose DCE ran on its AST, printed after DCE.
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
}

/// The chunk's generated import and export text, as `render_esm` would emit it.
pub struct GeneratedText<'a> {
  pub imports: Option<&'a str>,
  pub exports: Option<&'a str>,
}

/// Returns the DCE options when the fast path may run for this build at all.
#[expect(clippy::print_stderr, reason = "PoC debug output behind an env var")]
pub fn build_eligible<'o>(
  options: &'o NormalizedBundlerOptions,
  plugin_driver: &SharedPluginDriver,
  inline_state: &InlineCommonChunksState,
) -> Option<&'o OxcMinifierOptions> {
  reject!(!*ENABLED, options, "build", "disabled", None, None);
  let minify_options = match &options.minify {
    MinifyOptions::DeadCodeEliminationOnly(minify_options) => minify_options,
    MinifyOptions::Enabled(minify) => {
      reject!(
        minify.options.mangle_properties.is_some(),
        options,
        "build",
        "property_mangle",
        None,
        None
      );
      &minify.options
    }
    MinifyOptions::Disabled => {
      #[cfg(feature = "testing")]
      coverage::record(options, "build", "minify_disabled", None);
      return None;
    }
  };
  reject!(
    minify_options.compress.is_none() && minify_options.mangle.is_none(),
    options,
    "build",
    "no_compress_or_mangle",
    None,
    None
  );
  reject!(!matches!(options.format, OutputFormat::Esm), options, "build", "format", None, None);
  reject!(options.is_dev_mode_enabled(), options, "build", "dev_mode", None, None);
  // `renderChunk` plugins operate on chunk text before the original minifier.
  reject!(
    user_render_chunk_plugins(plugin_driver) != 0,
    options,
    "build",
    "render_chunk_plugin",
    None,
    None
  );
  // Intro/outro are executable code that the original reparse also optimizes.
  reject!(options.intro.is_some(), options, "build", "intro", None, None);
  reject!(options.outro.is_some(), options, "build", "outro", None, None);
  reject!(
    !plugin_driver.order_by_intro_meta.is_empty(),
    options,
    "build",
    "intro_hook",
    None,
    None
  );
  reject!(
    !plugin_driver.order_by_outro_meta.is_empty(),
    options,
    "build",
    "outro_hook",
    None,
    None
  );
  reject!(inline_state.has_records(), options, "build", "inline_records", None, None);
  // Only comment-only static banners/footers are known safe before rendering.
  reject!(!addon_is_comment_only(options.banner.as_ref()), options, "build", "banner", None, None);
  reject!(!addon_is_comment_only(options.footer.as_ref()), options, "build", "footer", None, None);
  reject!(
    !plugin_driver.order_by_banner_meta.is_empty(),
    options,
    "build",
    "banner_hook",
    None,
    None
  );
  reject!(
    !plugin_driver.order_by_footer_meta.is_empty(),
    options,
    "build",
    "footer_hook",
    None,
    None
  );
  let eligible = true;
  #[cfg(feature = "testing")]
  coverage::record(options, "build", "eligible", None);
  if std::env::var_os("ROLLDOWN_CHUNK_AST_DCE_DEBUG").is_some() {
    eprintln!(
      "chunk_ast_dce build_eligible={eligible} mangle={} mangle_props={} compress={} format={:?} \
       render_chunk_plugins={} intro={} outro={} intro_plugins={} outro_plugins={} records={}",
      minify_options.mangle.is_some(),
      minify_options.mangle_properties.is_some(),
      minify_options.compress.is_some(),
      options.format,
      user_render_chunk_plugins(plugin_driver),
      options.intro.is_some(),
      options.outro.is_some(),
      plugin_driver.order_by_intro_meta.len(),
      plugin_driver.order_by_outro_meta.len(),
      inline_state.has_records(),
    );
  }
  eligible.then_some(minify_options)
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

/// Per-chunk checks: the chunk text must consist only of module code, generated imports and
/// exports, and passthrough addons.
pub fn chunk_eligible(ctx: &GenerateContext<'_>, asts: &[&EcmaAst]) -> bool {
  let chunk = ctx.chunk;
  let filename =
    chunk.preliminary_filename.as_deref().map(arcstr::ArcStr::as_str).unwrap_or_default();
  reject!(
    filename.ends_with(".d.ts") || filename.ends_with(".d.cts") || filename.ends_with(".d.mts"),
    ctx.options,
    "chunk",
    "declaration",
    Some(filename),
    false
  );
  // Concatenated wrapped modules emit hoisted vars/fns and `__esm` wrappers as text.
  reject!(
    !chunk.module_groups.is_empty(),
    ctx.options,
    "chunk",
    "module_groups",
    Some(filename),
    false
  );
  // A CJS-wrapped ESM entry renders `export default require_x();`, which parses into the tail.
  // Other wrapped-entry shapes (init calls of order-wrapped ESM) are not covered.
  reject!(
    render_wrapped_entry_chunk(ctx, None).is_some()
      && !matches!(
        ctx.chunk.kind,
        ChunkKind::EntryPoint { module, .. }
          if matches!(ctx.link_output.metas[module].wrap_kind(), WrapKind::Cjs)
      ),
    ctx.options,
    "chunk",
    "wrapped_entry",
    Some(filename),
    false
  );
  let module_table = &ctx.link_output.module_table;
  let all_plain = chunk
    .modules
    .iter()
    .all(|idx| module_table[*idx].as_normal().is_none_or(|m| m.ecma_view.mutations.is_empty()));
  reject!(!all_plain, ctx.options, "chunk", "mutations", Some(filename), false);
  // Module bodies are expected to have no module declarations after finalization: they would be
  // confused with the generated head/tail.
  reject!(
    asts.iter().any(|ast| ast.program().body.iter().any(Statement::is_module_declaration)),
    ctx.options,
    "chunk",
    "module_declaration",
    Some(filename),
    false
  );
  // Builtin renderChunk hooks resolve these strings before the normal minify pass. Running the
  // minifier first can fold operations on the placeholder or print it with backticks, which the
  // emitted-file rewriter does not recognize. Copy-module references also occur in generated
  // imports/exports, so checking module source alone is insufficient.
  // See internal-docs/plugin-asset-module/implementation.md.
  reject!(
    asts.iter().any(|ast| has_emitted_file_placeholder(ast.program().source_text))
      || render_esm_chunk_imports(ctx).as_deref().is_some_and(has_emitted_file_placeholder)
      || render_wrapped_entry_chunk(ctx, None).as_deref().is_some_and(has_emitted_file_placeholder)
      || render_chunk_exports(ctx, None).as_deref().is_some_and(has_emitted_file_placeholder),
    ctx.options,
    "chunk",
    "emitted_file_placeholder",
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
#[expect(clippy::print_stderr, reason = "PoC debug output behind an env var")]
fn dce_chunk(
  ctx: &GenerateContext<'_>,
  allocator: &Allocator,
  asts: &mut [owned::ChunkProgram],
  source_text: &mut Option<std::sync::Arc<str>>,
  minify_options: &OxcMinifierOptions,
) -> ChunkAstDceOutput {
  let t = Timer::start();
  let builder = AstBuilder::new(allocator);
  let source_type = ctx.options.format.source_type().with_jsx(true);
  let imports_text = render_esm_chunk_imports(ctx).map(|s| allocator.alloc_str(&s));
  // `render_esm` emits the wrapped entry's `export default require_x();` right before the exports.
  let exports_text = match (render_wrapped_entry_chunk(ctx, None), render_chunk_exports(ctx, None))
  {
    (None, None) => None,
    (entry, exports) => Some(
      allocator.alloc_str(&concat_string!(entry.unwrap_or_default(), exports.unwrap_or_default())),
    ),
  };
  // Without sourcemaps, `ROLLDOWN_CHUNK_AST_DCE=nobarrier` drops the markers, so oxc may move
  // code across module boundaries like it does on the re-parsed text; see `split_by_address`.
  if *GLOBAL {
    #[cfg(feature = "testing")]
    coverage::record(
      ctx.options,
      "execute",
      "global",
      ctx.chunk.preliminary_filename.as_deref().map(arcstr::ArcStr::as_str),
    );
    let modules: Vec<_> = ctx
      .chunk
      .modules
      .iter()
      .filter_map(|idx| ctx.link_output.module_table[*idx].as_normal())
      .collect();
    let generated = GeneratedText { imports: imports_text, exports: exports_text };
    return global::dce_chunk_global(
      ctx,
      allocator,
      asts,
      &modules,
      &generated,
      source_text,
      minify_options,
    );
  }
  let use_markers = !(*NO_BARRIER && (ctx.options.sourcemap.is_none() || *NO_BARRIER_WITH_MAPS));
  #[cfg(feature = "testing")]
  coverage::record(
    ctx.options,
    "execute",
    if use_markers { "legacy_barrier" } else { "legacy_nobarrier" },
    ctx.chunk.preliminary_filename.as_deref().map(arcstr::ArcStr::as_str),
  );

  let mut body: ArenaVec<'_, Statement<'_>> = ArenaVec::with_capacity_in(
    asts.iter().map(|ast| ast.body.len() + 1).sum::<usize>() + 2,
    &allocator,
  );
  if let Some(text) = imports_text {
    parse_generated(allocator, text, source_type, &mut body);
  }
  let mut owner_by_address = FxHashMap::default();
  for (module_pos, ast) in asts.iter_mut().enumerate() {
    for stmt in ast.take_body(allocator) {
      if !use_markers {
        owner_by_address.insert(stmt.address(), module_pos);
      }
      body.push(stmt);
    }
    if use_markers {
      body.push(Statement::new_export_named_declaration(
        Span::new(MARKER_POS, MARKER_POS),
        [],
        ImportOrExportKind::Value,
        &builder,
      ));
    }
  }
  if let Some(text) = exports_text {
    parse_generated(allocator, text, source_type, &mut body);
  }
  let mut program = Program::new(SPAN, source_type, "", [], None, [], body, &builder);
  t.stop(Phase::AstAssemble);
  if let Some(dir) = std::env::var_os("ROLLDOWN_CHUNK_AST_DCE_DUMP") {
    let code = Codegen::new().build(&program).code;
    let name = ctx.chunk.preliminary_filename.as_deref().map(|f| f.as_str().replace('/', "_"));
    std::fs::write(
      std::path::Path::new(&dir).join(format!("{}.pre-dce.js", name.unwrap_or_default())),
      code,
    )
    .unwrap();
  }

  let t = Timer::start();
  let ret = Minifier::new(minify_options.clone()).dce(allocator, &mut program);
  debug_assert!(ret.scoping.is_none(), "DCE-only mode does not mangle");
  if std::env::var_os("ROLLDOWN_CHUNK_AST_DCE_DEBUG").is_some() {
    eprintln!("chunk_ast_dce dce iterations={}", ret.iterations);
  }
  t.stop(Phase::AstDce);

  if let Some(dir) = std::env::var_os("ROLLDOWN_CHUNK_AST_DCE_DUMP") {
    let code = Codegen::new().build(&program).code;
    let name = ctx.chunk.preliminary_filename.as_deref().map(|f| f.as_str().replace('/', "_"));
    std::fs::write(
      std::path::Path::new(&dir).join(format!("{}.post-dce.js", name.unwrap_or_default())),
      code,
    )
    .unwrap();
  }
  let t = Timer::start();
  let stmts = program.body.take_in(&allocator);
  let Split { head, segments, tail } = if use_markers {
    split_at_markers(allocator, stmts, asts.len())
  } else {
    split_by_address(allocator, stmts, asts.len(), &owner_by_address)
  };
  for (ast, segment) in asts.iter_mut().zip(segments) {
    // SAFETY: the private ChunkAst owner retains every backing arena through printing/drop.
    ast.body = unsafe {
      std::mem::transmute::<ArenaVec<'_, Statement<'_>>, ArenaVec<'static, Statement<'static>>>(
        segment,
      )
    };
  }
  let imports = print_generated(allocator, ctx.options, source_type, head);
  let exports = print_generated(allocator, ctx.options, source_type, tail);
  t.stop(Phase::AstSplit);
  ChunkAstDceOutput { imports, exports, imports_map: None, exports_map: None, space: None }
}

struct Split<'a> {
  /// Generated imports (module declarations before any module code).
  head: ArenaVec<'a, Statement<'a>>,
  /// One body per module, in `chunk.modules` order.
  segments: Vec<ArenaVec<'a, Statement<'a>>>,
  /// Generated exports (module declarations after the module code).
  tail: ArenaVec<'a, Statement<'a>>,
}

/// `stmts` without the leading generated imports: module declarations before any module code
/// (module bodies contain none, and oxc keeps the order of top-level statements).
fn split_head<'a>(
  allocator: &'a Allocator,
  stmts: ArenaVec<'a, Statement<'a>>,
) -> (ArenaVec<'a, Statement<'a>>, std::vec::Vec<Statement<'a>>) {
  let mut head = ArenaVec::new_in(&allocator);
  let mut rest = vec![];
  for stmt in stmts {
    if rest.is_empty() && stmt.is_module_declaration() && !is_marker(&stmt) {
      head.push(stmt);
    } else {
      rest.push(stmt);
    }
  }
  (head, rest)
}

/// Everything after the last marker came from the generated tail (wrapped entry + exports), which
/// is not only module declarations (e.g. `var sub = import_sub.sub;` before `export { sub }`).
fn split_at_markers<'a>(
  allocator: &'a Allocator,
  stmts: ArenaVec<'a, Statement<'a>>,
  module_count: usize,
) -> Split<'a> {
  let (head, rest) = split_head(allocator, stmts);
  let mut segments = Vec::with_capacity(module_count);
  let mut current = ArenaVec::new_in(&allocator);
  for stmt in rest {
    if is_marker(&stmt) {
      segments.push(std::mem::replace(&mut current, ArenaVec::new_in(&allocator)));
    } else {
      current.push(stmt);
    }
  }
  assert_eq!(
    segments.len(),
    module_count,
    "chunk AST DCE: a segment marker was removed; the split would be wrong"
  );
  // Rarely, a module's statements land after its marker (`var`s that dead-code removal re-emits
  // at the end of the body); they are printed with the tail. Correct, but loses their mappings.
  Split { head, segments, tail: current }
}

/// Assigns each surviving top-level statement to the module whose original top-level statement
/// had the same address; statements created by the minifier go to the module of the statement
/// before them. Nodes moved across modules (single-use inlining) keep their original module's
/// spans, so they would get wrong sourcemap positions and could pick up the wrong comments: only
/// used without sourcemaps, to measure what the barrier costs.
fn split_by_address<'a>(
  allocator: &'a Allocator,
  stmts: ArenaVec<'a, Statement<'a>>,
  module_count: usize,
  owner_by_address: &FxHashMap<Address, usize>,
) -> Split<'a> {
  let (head, mut rest) = split_head(allocator, stmts);
  // The generated tail: the trailing run of statements that no module owned.
  let tail_len =
    rest.iter().rev().take_while(|s| !owner_by_address.contains_key(&s.address())).count();
  let tail = ArenaVec::from_iter_in(rest.drain(rest.len() - tail_len..), &allocator);
  let mut segments: Vec<_> = (0..module_count).map(|_| ArenaVec::new_in(&allocator)).collect();
  let mut current = 0;
  for stmt in rest {
    if let Some(&owner) = owner_by_address.get(&stmt.address()) {
      current = current.max(owner);
    }
    segments[current].push(stmt);
  }
  Split { head, segments, tail }
}

fn is_marker(stmt: &Statement<'_>) -> bool {
  matches!(stmt, Statement::ExportNamedDeclaration(decl)
    if decl.span.start == MARKER_POS && decl.specifiers.is_empty())
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

pub(super) fn print_generated<'a>(
  allocator: &'a Allocator,
  options: &NormalizedBundlerOptions,
  source_type: SourceType,
  stmts: ArenaVec<'a, Statement<'a>>,
) -> Option<String> {
  if stmts.is_empty() {
    return None;
  }
  let builder = AstBuilder::new(allocator);
  let program = Program::new(SPAN, source_type, "", [], None, [], stmts, &builder);
  let code = Codegen::new()
    .with_options(CodegenOptions {
      ascii_only: options.charset.is_ascii(),
      comments: CommentOptions::disabled(),
      ..CodegenOptions::default()
    })
    .build(&program)
    .code;
  Some(code)
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
