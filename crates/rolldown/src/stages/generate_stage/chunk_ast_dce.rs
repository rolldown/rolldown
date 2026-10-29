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
use rolldown_common::{ChunkKind, MinifyOptions, NormalizedBundlerOptions, OutputFormat, WrapKind};
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

static ENABLED: LazyLock<bool> = LazyLock::new(|| {
  std::env::var_os("ROLLDOWN_CHUNK_AST_DCE")
    .is_some_and(|v| v == "1" || v == "nobarrier" || v == "nobarrier-unsafe")
});

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
}

/// Returns the DCE options when the fast path may run for this build at all.
pub fn build_eligible<'o>(
  options: &'o NormalizedBundlerOptions,
  plugin_driver: &SharedPluginDriver,
  inline_state: &InlineCommonChunksState,
) -> Option<&'o OxcMinifierOptions> {
  if !*ENABLED {
    return None;
  }
  // Full minify can mangle and join statements across modules: not covered.
  let MinifyOptions::DeadCodeEliminationOnly(minify_options) = &options.minify else {
    return None;
  };
  let eligible = minify_options.mangle.is_none()
    && minify_options.mangle_properties.is_none()
    && minify_options.compress.is_some()
    && matches!(options.format, OutputFormat::Esm)
    && !options.is_dev_mode_enabled()
    // `renderChunk` plugins operate on (and may change) the chunk text before minify.
    && user_render_chunk_plugins(plugin_driver) == 0
    // intro/outro are code that the re-parse would also DCE.
    && options.intro.is_none()
    && options.outro.is_none()
    && plugin_driver.order_by_intro_meta.is_empty()
    && plugin_driver.order_by_outro_meta.is_empty()
    && !inline_state.has_records();
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

/// `renderChunk` plugins other than the builtin asset/copy plugins, which only replace emitted-file
/// placeholders inside string literals (DCE is indifferent to that text, so they can run on the
/// DCE'd text as they do on the minified text today), and the test harness's region-marker plugin,
/// which only rewrites comment lines. PoC-only: a real version needs a plugin-declared contract
/// instead of names.
fn user_render_chunk_plugins(plugin_driver: &SharedPluginDriver) -> usize {
  plugin_driver
    .order_by_render_chunk_meta
    .iter()
    .filter(|idx| {
      !matches!(
        &*plugin_driver.plugins()[**idx].call_name(),
        "builtin:asset-module" | "builtin:copy-module" | "rolldown-testing:hidden-module-markers"
      )
    })
    .count()
}

/// Per-chunk checks: the chunk text must consist only of module code, generated imports and
/// exports, and passthrough addons.
pub fn chunk_eligible(ctx: &GenerateContext<'_>, asts: &[&mut EcmaAst]) -> bool {
  let chunk = ctx.chunk;
  let filename = chunk.preliminary_filename.as_deref().map(|f| f.as_str()).unwrap_or_default();
  if filename.ends_with(".d.ts") || filename.ends_with(".d.cts") || filename.ends_with(".d.mts") {
    return false;
  }
  // Concatenated wrapped modules emit hoisted vars/fns and `__esm` wrappers as text.
  if !chunk.module_groups.is_empty() {
    return false;
  }
  // A CJS-wrapped ESM entry renders `export default require_x();`, which parses into the tail.
  // Other wrapped-entry shapes (init calls of order-wrapped ESM) are not covered.
  if render_wrapped_entry_chunk(ctx, None).is_some()
    && !matches!(
      ctx.chunk.kind,
      ChunkKind::EntryPoint { module, .. }
        if matches!(ctx.link_output.metas[module].wrap_kind(), WrapKind::Cjs)
    )
  {
    return false;
  }
  let module_table = &ctx.link_output.module_table;
  let all_plain = chunk
    .modules
    .iter()
    .all(|idx| module_table[*idx].as_normal().is_none_or(|m| m.ecma_view.mutations.is_empty()));
  if !all_plain {
    return false;
  }
  // Module bodies are expected to have no module declarations after finalization: they would be
  // confused with the generated head/tail.
  asts.iter().all(|ast| !ast.program().body.iter().any(Statement::is_module_declaration))
}

/// Runs DCE on the chunk assembled from `asts` (in `chunk.modules` order) and writes the DCE'd
/// statements back into each module's program.
///
/// # Lifetimes
///
/// Statements are moved between programs whose arenas differ, so their lifetimes are erased with
/// `transmute`. `allocator` receives every node the minifier creates, and those nodes end up in
/// the module programs, so `allocator` must outlive every use of `asts` (the caller drops it
/// after the module ASTs are printed and dropped).
pub fn dce_chunk(
  ctx: &GenerateContext<'_>,
  allocator: &Allocator,
  asts: &mut [&mut EcmaAst],
  minify_options: &OxcMinifierOptions,
) -> ChunkAstDceOutput {
  let t = Timer::start();
  let builder = AstBuilder::new(allocator);
  let source_type = ctx.options.format.source_type().with_jsx(true);
  let imports_text = render_esm_chunk_imports(ctx).map(|s| &*allocator.alloc_str(&s));
  // `render_esm` emits the wrapped entry's `export default require_x();` right before the exports.
  let exports_text = match (render_wrapped_entry_chunk(ctx, None), render_chunk_exports(ctx, None))
  {
    (None, None) => None,
    (entry, exports) => Some(
      &*allocator
        .alloc_str(&concat_string!(entry.unwrap_or_default(), exports.unwrap_or_default())),
    ),
  };
  // Without sourcemaps, `ROLLDOWN_CHUNK_AST_DCE=nobarrier` drops the markers, so oxc may move
  // code across module boundaries like it does on the re-parsed text; see `split_by_address`.
  let use_markers = !(*NO_BARRIER && (ctx.options.sourcemap.is_none() || *NO_BARRIER_WITH_MAPS));

  let mut body: ArenaVec<'_, Statement<'_>> = ArenaVec::with_capacity_in(
    asts.iter().map(|ast| ast.program().body.len() + 1).sum::<usize>() + 2,
    &allocator,
  );
  if let Some(text) = imports_text {
    parse_generated(allocator, text, source_type, &mut body);
  }
  let mut owner_by_address = FxHashMap::default();
  for (module_pos, ast) in asts.iter_mut().enumerate() {
    ast.program.with_mut(|fields| {
      let stmts = fields.program.body.take_in(&fields.allocator);
      for stmt in stmts {
        // SAFETY: see the function docs; the module arena outlives the chunk program.
        let stmt: Statement<'_> = unsafe { std::mem::transmute(stmt) };
        if !use_markers {
          owner_by_address.insert(stmt.address(), module_pos);
        }
        body.push(stmt);
      }
    });
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

  let t = Timer::start();
  let stmts = program.body.take_in(&allocator);
  let Split { head, segments, tail } = if use_markers {
    split_at_markers(allocator, stmts, asts.len())
  } else {
    split_by_address(allocator, stmts, asts.len(), &owner_by_address)
  };
  for (ast, segment) in asts.iter_mut().zip(segments) {
    ast.program.with_mut(|fields| {
      // SAFETY: see the function docs.
      let segment: ArenaVec<'_, Statement<'_>> = unsafe { std::mem::transmute(segment) };
      fields.program.body = segment;
    });
  }
  let imports = print_generated(allocator, ctx.options, source_type, head);
  let exports = print_generated(allocator, ctx.options, source_type, tail);
  t.stop(Phase::AstSplit);
  ChunkAstDceOutput { imports, exports }
}

struct Split<'a> {
  /// Generated imports (module declarations before any module code).
  head: ArenaVec<'a, Statement<'a>>,
  /// One body per module, in `chunk.modules` order.
  segments: Vec<ArenaVec<'a, Statement<'a>>>,
  /// Generated exports (module declarations after the module code).
  tail: ArenaVec<'a, Statement<'a>>,
}

/// Module bodies contain no module declarations, so leading ones are the generated imports and
/// trailing ones the generated exports. oxc keeps the order of top-level statements.
fn split_head_tail<'a>(
  allocator: &'a Allocator,
  stmts: ArenaVec<'a, Statement<'a>>,
) -> (ArenaVec<'a, Statement<'a>>, std::vec::Vec<Statement<'a>>, ArenaVec<'a, Statement<'a>>) {
  let mut head = ArenaVec::new_in(&allocator);
  let mut middle = vec![];
  let mut tail = ArenaVec::new_in(&allocator);
  for stmt in stmts {
    if stmt.is_module_declaration() && !is_marker(&stmt) {
      if middle.is_empty() { head.push(stmt) } else { tail.push(stmt) }
    } else {
      middle.push(stmt);
    }
  }
  (head, middle, tail)
}

fn split_at_markers<'a>(
  allocator: &'a Allocator,
  stmts: ArenaVec<'a, Statement<'a>>,
  module_count: usize,
) -> Split<'a> {
  let (head, middle, tail) = split_head_tail(allocator, stmts);
  let mut segments = Vec::with_capacity(module_count);
  let mut current = ArenaVec::new_in(&allocator);
  for stmt in middle {
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
  // Statements after the last marker (e.g. `var`s re-emitted by dead-code removal) belong to the
  // last module.
  if let Some(last) = segments.last_mut() {
    last.extend(current);
  }
  Split { head, segments, tail }
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
  let (head, middle, tail) = split_head_tail(allocator, stmts);
  let mut segments: Vec<_> = (0..module_count).map(|_| ArenaVec::new_in(&allocator)).collect();
  let mut current = 0;
  for stmt in middle {
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

fn parse_generated<'a>(
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

fn print_generated<'a>(
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
