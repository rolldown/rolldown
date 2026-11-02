//! Chunk AST optimization without module barriers, with correct sourcemaps.
//!
//! Oxc moves code across module boundaries (single-use inlining), and a moved node keeps spans
//! into its *original* module's source while it is printed with the *target* module's source text
//! and line tables. A shared virtual source space removes that ambiguity:
//!
//! 1. One text `V` per chunk holds every module's source (each starting on its own line), then
//!    the generated head and tail text. Every module's spans and comments are rebased by the
//!    module's offset in `V` (in parallel), and `program.source_text` becomes `V`. A span now
//!    identifies its module.
//! 2. The modules' statements are assembled without markers and `Minifier::dce` runs, free to
//!    move code across modules as it does on the re-parsed text.
//! 3. Each top-level statement goes back to the module whose range of `V` holds its span.
//! 4. Each module is printed (in parallel, as today) against `V`, with `V`'s line tables built
//!    once and shared (patched `oxc_codegen`, `vendor/oxc_codegen`). Moved nodes get correct
//!    positions in `V`.
//! 5. Each module's map is remapped from `V` to `(module, line, column)`: a module's map can now
//!    name other modules as sources. The joined chunk map is then de-duplicated by source.

#![cfg_attr(
  test,
  expect(clippy::self_named_module_files, reason = "only test helpers use the subdirectory")
)]

#[cfg(test)]
mod tests;

use std::{borrow::Cow, sync::Arc};

use oxc::{
  allocator::{Allocator, TakeIn, Vec as ArenaVec},
  ast::{
    ast::{Class, Program, Statement},
    builder::AstBuilder,
  },
  ast_visit::{Visit, VisitMut, walk},
  codegen::{Codegen, CodegenOptions, CommentOptions, LegalComment, LineOffsetTables},
  minifier::MinifierOptions as OxcMinifierOptions,
  semantic::Scoping,
  span::{SPAN, Span},
  syntax::class::ClassId,
};
use oxc_index::IndexVec;
use oxc_sourcemap::{SourceMap, SourceMapParts, Token};
use oxc_str::CompactStr;
use rolldown_common::{
  MinifyOptions, ModuleIdx, ModuleRenderOutput, ModuleTable, NormalModule, NormalizedBundlerOptions,
};
use rolldown_ecmascript::EcmaCompiler;
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  poc_prof::{Phase, Timer},
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};
use rustc_hash::FxHashMap;

use super::{ChunkAstDceOutput, GeneratedText, parse_generated};
use crate::{
  types::generator::GenerateContext, utils::render_ecma_module::collapse_module_sourcemap,
};

type PrivateMemberMappings = IndexVec<ClassId, FxHashMap<String, CompactStr>>;

/// The shared source space of one chunk (see the module docs).
pub struct ChunkSourceSpace {
  /// `V`: module sources, then head and tail text, each followed by `\n`.
  text: Arc<str>,
  /// Line tables of `text`; empty when sourcemaps are off.
  tables: Arc<LineOffsetTables>,
  /// Per module, in `chunk.modules` order (normal modules only).
  modules: Vec<SpaceModule>,
  /// Chunk-wide mangled names, shared by every module print.
  scoping: Option<Arc<Scoping>>,
  private_member_mappings: Option<Arc<PrivateMemberMappings>>,
  /// First chunk-wide class ID of each module, for private-member lookup.
  class_offsets: Vec<usize>,
}

struct SpaceModule {
  /// Byte offset of the module's source in `V`.
  base: u32,
  len: u32,
  /// Lines of `base` and of the module's last byte in `V`.
  line: u32,
  end_line: u32,
  /// Source name in sourcemaps (`module.id`), `None` for virtual modules (no mappings).
  source_name: Option<String>,
  idx: ModuleIdx,
  /// The module's text came out of plugin transforms: tokens into it must be composed with its
  /// `sourcemap_chain`.
  has_chain: bool,
}

impl ChunkSourceSpace {
  pub(super) fn module_idx(&self, pos: usize) -> ModuleIdx {
    self.modules[pos].idx
  }
}

impl std::fmt::Debug for ChunkSourceSpace {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ChunkSourceSpace").field("len", &self.text.len()).finish_non_exhaustive()
  }
}

struct Rebase(u32);

impl VisitMut<'_> for Rebase {
  fn visit_span(&mut self, span: &mut Span) {
    span.start += self.0;
    span.end += self.0;
  }
}

#[derive(Default)]
struct ClassCounter(usize);

impl<'a> Visit<'a> for ClassCounter {
  fn visit_class(&mut self, class: &Class<'a>) {
    self.0 += 1;
    walk::walk_class(self, class);
  }
}

fn count_classes(statements: &[Statement<'_>]) -> usize {
  let mut counter = ClassCounter::default();
  for statement in statements {
    counter.visit_statement(statement);
  }
  counter.0
}

/// Layout retained with the complete unoptimized program. It contains no borrowed AST or
/// generation context, so the exclusive owner can survive asynchronous rendering hooks.
pub(super) struct ChunkLayout {
  space: ChunkSourceSpace,
  head_base: u32,
  tail_base: u32,
  runtime_before_imports: bool,
}

fn parse_user_text<'a>(
  allocator: &'a Allocator,
  text: &'a str,
  source_type: oxc::span::SourceType,
  base: u32,
  filename: &str,
) -> rolldown_error::BuildResult<Program<'a>> {
  let ret = oxc::parser::Parser::new(allocator, text, source_type).parse();
  if ret.fatal_error || !ret.diagnostics.is_empty() {
    return Err(
      rolldown_error::BuildDiagnostic::from_oxc_diagnostics(
        ret.diagnostics,
        &arcstr::ArcStr::from(text),
        filename,
        rolldown_error::Severity::Error,
        rolldown_error::EventKind::ParseError,
      )
      .into(),
    );
  }
  let mut program = ret.program;
  Rebase(base).visit_program(&mut program);
  for comment in &mut program.comments {
    comment.span.start += base;
    comment.span.end += base;
    comment.attached_to += base;
  }
  Ok(program)
}

// Finalization removes module directives; CJS strictness comes from the emitted prologue.
fn chunk_directives<'a>(
  ctx: &GenerateContext<'_>,
  allocator: &'a Allocator,
  is_cjs: bool,
) -> ArenaVec<'a, oxc::ast::ast::Directive<'a>> {
  let builder = AstBuilder::new(allocator);
  let mut directives = ArenaVec::new_in(&allocator);
  if is_cjs
    && crate::ecmascript::chunk_directives(ctx).iter().any(|directive| {
      directive.trim_start_matches(['\'', '"']).trim_end_matches(['\'', '"', ';']) == "use strict"
    })
  {
    directives.push(oxc::ast::ast::Directive::new(
      SPAN,
      oxc::ast::ast::StringLiteral::new(SPAN, "use strict", None, &builder),
      "use strict",
      &builder,
    ));
  }
  directives
}

fn retain_module_comment(comment: &oxc::ast::Comment, options: &NormalizedBundlerOptions) -> bool {
  (comment.is_legal() && options.comments.legal)
    || (comment.is_jsdoc() && options.comments.jsdoc)
    || (comment.is_annotation() && options.comments.annotation)
}

/// Assemble without optimizing or splitting. `modules[i]` is the module of `asts[i]`.
pub(super) fn assemble_chunk<'a>(
  ctx: &GenerateContext<'_>,
  allocator: &'a Allocator,
  asts: &mut [super::owned::ChunkProgram],
  modules: &[&NormalModule],
  generated: &GeneratedText<'a>,
  source_text: &mut Option<Arc<str>>,
) -> rolldown_error::BuildResult<(Program<'a>, ChunkLayout)> {
  assemble_chunk_with_inline(ctx, allocator, asts, modules, generated, source_text, &[])
}

pub(super) fn assemble_chunk_with_inline<'a>(
  ctx: &GenerateContext<'_>,
  allocator: &'a Allocator,
  asts: &mut [super::owned::ChunkProgram],
  modules: &[&NormalModule],
  generated: &GeneratedText<'a>,
  source_text: &mut Option<Arc<str>>,
  inline_records: &[(rolldown_common::ChunkIdx, std::ops::Range<usize>)],
) -> rolldown_error::BuildResult<(Program<'a>, ChunkLayout)> {
  assemble_chunk_with_wrapper(
    ctx,
    allocator,
    asts,
    modules,
    generated,
    source_text,
    inline_records,
    None,
  )
}

#[expect(
  clippy::too_many_arguments,
  reason = "exclusive assembly inputs plus optional retained inline and wrapper plans"
)]
pub(super) fn assemble_chunk_with_wrapper<'a>(
  ctx: &GenerateContext<'_>,
  allocator: &'a Allocator,
  asts: &mut [super::owned::ChunkProgram],
  modules: &[&NormalModule],
  generated: &GeneratedText<'a>,
  source_text: &mut Option<Arc<str>>,
  inline_records: &[(rolldown_common::ChunkIdx, std::ops::Range<usize>)],
  wrapper_ast: Option<&crate::ecmascript::WrapperAstPlan>,
) -> rolldown_error::BuildResult<(Program<'a>, ChunkLayout)> {
  let t = Timer::start();
  let builder = AstBuilder::new(allocator);
  let source_type = ctx.options.format.source_type().with_jsx(true);
  let enable_sourcemap = ctx.options.sourcemap.is_some();

  // 1. `V` and the module offsets.
  let total = asts.iter().map(|ast| ast.source_text.len() + 1).sum::<usize>()
    + generated.prefix.map_or(0, str::len)
    + generated.imports.map_or(0, str::len)
    + generated.exports.map_or(0, str::len)
    + wrapper_ast.map_or(0, |plan| plan.outer_suffix.len())
    + 4;
  let mut text = String::with_capacity(total);
  // Keep every donor, including the first, away from SPAN. Finalizer-synthesized nodes are
  // rebased with their donor; nodes created later by the minifier retain SPAN and stay unowned.
  text.push('\n');
  let mut bases = Vec::with_capacity(asts.len());
  let mut lens = Vec::with_capacity(asts.len());
  for ast in asts.iter() {
    bases.push(u32::try_from(text.len()).expect("chunk sources fit in u32"));
    lens.push(u32::try_from(ast.source_text.len()).expect("chunk sources fit in u32"));
    text.push_str(ast.source_text);
    text.push('\n');
  }
  let head_base = u32::try_from(text.len()).expect("chunk sources fit in u32");
  text.push_str(generated.prefix.unwrap_or_default());
  text.push('\n');
  let imports_base = u32::try_from(text.len()).expect("chunk sources fit in u32");
  text.push_str(generated.imports.unwrap_or_default());
  text.push('\n');
  let tail_base = u32::try_from(text.len()).expect("chunk sources fit in u32");
  text.push_str(generated.exports.unwrap_or_default());
  text.push('\n');
  let footer_base = u32::try_from(text.len()).expect("chunk sources fit in u32");
  if let Some(plan) = wrapper_ast {
    text.push_str(&plan.outer_suffix);
    text.push('\n');
  }
  let text: Arc<str> = Arc::from(text);
  // Retain V in the owning guard BEFORE installing references, including on unwind.
  *source_text = Some(Arc::clone(&text));
  // SAFETY: ChunkAst retains this allocation until all private roots are retired.
  let text_ref: &'static str = unsafe { std::mem::transmute::<&str, &'static str>(&text) };

  // Rebase every module's spans and comments into `V`.
  t.stop(Phase::AstSpace);
  // The module's line tables are built in the same parallel job; `concat`ed into `V`'s below.
  let module_tables: Vec<LineOffsetTables> = asts
    .par_iter_mut()
    .zip(bases.par_iter())
    .map(|(ast, base)| {
      let t = Timer::start();
      let base = *base;
      let tables = if enable_sourcemap {
        LineOffsetTables::new(ast.source_text)
      } else {
        LineOffsetTables::default()
      };
      Rebase(base).visit_program(&mut ast.0);
      for comment in &mut ast.comments {
        comment.span.start += base;
        comment.span.end += base;
        if comment.attached_to == 0 && wrapper_ast.is_some() {
          comment.attached_to = head_base;
        } else if comment.attached_to != 0 {
          comment.attached_to += base;
        }
      }
      ast.source_text = text_ref;
      t.stop(Phase::AstRebase);
      tables
    })
    .collect();
  let t = Timer::start();

  // 2. Assemble without markers: imports, module bodies, tail.
  let mut body: ArenaVec<'_, Statement<'_>> = ArenaVec::with_capacity_in(
    asts.iter().map(|ast| ast.body.len()).sum::<usize>() + 2,
    &allocator,
  );
  if wrapper_ast.is_none()
    && let Some(prefix) = generated.prefix
  {
    parse_generated(allocator, prefix, source_type, &mut body);
    for stmt in &mut body {
      Rebase(head_base).visit_statement(stmt);
    }
  }
  // CJS external interop can call runtime helpers from its generated require prelude.
  // Move those helper declarations first, matching the ordinary CJS renderer.
  if generated.imports_after_runtime
    && modules.first().is_some_and(|module| module.idx == ctx.link_output.runtime.id())
  {
    body.extend(asts[0].take_body(allocator));
  }
  if let Some(imports) = generated.imports {
    let start = body.len();
    parse_generated(allocator, imports, source_type, &mut body);
    for stmt in &mut body[start..] {
      Rebase(imports_base).visit_statement(stmt);
    }
  }
  append_module_bodies(ctx, allocator, asts, inline_records, head_base, &mut body);
  if let Some(exports) = generated.exports {
    let start = body.len();
    parse_generated(allocator, exports, source_type, &mut body);
    for stmt in &mut body[start..] {
      Rebase(tail_base).visit_statement(stmt);
    }
  }
  // The compressor uses comment anchors to retain consumer attribution when moving a literal
  // would otherwise strand its comments. Module codegen still owns and prints the originals.
  let mut comments = ArenaVec::new_in(&allocator);
  if enable_sourcemap || wrapper_ast.is_some() {
    for ast in asts.iter() {
      comments.extend(
        ast
          .comments
          .iter()
          .filter(|comment| wrapper_ast.is_none() || retain_module_comment(comment, ctx.options))
          .cloned(),
      );
    }
  }
  let program = if let Some(plan) = wrapper_ast {
    let body = plan.wrap(allocator, Span::new(head_base, head_base), body, ctx)?;
    structured_program(
      allocator,
      source_type,
      text_ref,
      comments,
      body,
      plan,
      head_base,
      footer_base,
    )?
  } else {
    let directives = chunk_directives(ctx, allocator, generated.imports_after_runtime);
    Program::new(SPAN, source_type, text_ref, comments, None, directives, body, &builder)
  };
  t.stop(Phase::AstAssemble);

  let t = Timer::start();
  let tables =
    if enable_sourcemap {
      let head = &text[head_base as usize..tail_base as usize];
      let tail = &text[tail_base as usize..];
      Arc::new(LineOffsetTables::concat(bases.iter().copied().zip(module_tables).chain([
        (head_base, LineOffsetTables::new(head)),
        (tail_base, LineOffsetTables::new(tail)),
      ])))
    } else {
      Arc::default()
    };
  let space_modules = modules
    .iter()
    .zip(bases.iter().zip(&lens))
    .map(|(module, (&base, &len))| SpaceModule {
      base,
      len,
      line: if enable_sourcemap { tables.line_index_of(base) } else { 0 },
      end_line: if enable_sourcemap { tables.line_index_of(base + len) } else { 0 },
      source_name: (!module.is_virtual()).then(|| module.id.to_string()),
      idx: module.idx,
      has_chain: !module.sourcemap_chain.is_empty(),
    })
    .collect();
  let space = ChunkSourceSpace {
    text,
    tables,
    modules: space_modules,
    scoping: None,
    private_member_mappings: None,
    class_offsets: Vec::new(),
  };
  t.stop(Phase::AstSpace);
  Ok((
    program,
    ChunkLayout {
      space,
      head_base,
      tail_base,
      runtime_before_imports: generated.imports_after_runtime
        && modules.first().is_some_and(|module| module.idx == ctx.link_output.runtime.id()),
    },
  ))
}

/// Parse only actual user text. Builtin wrapper syntax is already constructed in `body`.
#[expect(
  clippy::too_many_arguments,
  reason = "program parts and generated source boundaries have distinct ownership"
)]
fn structured_program<'a>(
  allocator: &'a Allocator,
  source_type: oxc::span::SourceType,
  text: &'a str,
  mut comments: ArenaVec<'a, oxc::ast::Comment>,
  body: ArenaVec<'a, Statement<'a>>,
  plan: &crate::ecmascript::WrapperAstPlan,
  head_base: u32,
  footer_base: u32,
) -> rolldown_error::BuildResult<Program<'a>> {
  use oxc::{ast::CommentPosition, span::GetSpan};
  let builder = AstBuilder::new(allocator);
  let mut directives = ArenaVec::new_in(&allocator);
  for raw in &plan.directives {
    let raw = allocator.alloc_str(raw);
    let parsed = parse_user_text(allocator, raw, source_type, head_base, "<directive>")?;
    for mut directive in parsed.directives {
      RebaseDirective(Span::new(head_base, head_base)).visit_directive(&mut directive);
      directives.push(directive);
    }
  }
  let first = body.first().map_or(head_base, |statement| statement.span().start);
  let end = u32::try_from(text.len()).expect("chunk sources fit u32");
  for (source, base, anchor) in
    [(plan.outer_prefix.as_str(), head_base, first), (plan.outer_suffix.as_str(), footer_base, end)]
  {
    if source.is_empty() {
      continue;
    }
    let source = &text[base as usize..base as usize + source.len()];
    let parsed = parse_user_text(allocator, source, source_type, base, "<addon>")?;
    debug_assert!(parsed.body.is_empty(), "executable addons use the final-text adapter");
    for mut comment in parsed.comments {
      comment.attached_to = anchor;
      // Codegen stores leading comments; the program-end anchor emits the footer last.
      comment.position = CommentPosition::Leading;
      comments.push(comment);
    }
  }
  comments.sort_unstable_by_key(|comment| comment.span.start);
  let span = Span::new(head_base, head_base);
  let hashbang = plan.hashbang.as_ref().map(|value| {
    oxc::ast::ast::Hashbang::new(span, oxc::ast::ast::Str::from_str_in(value, &builder), &builder)
  });
  Ok(Program::new(
    Span::new(0, end),
    source_type,
    text,
    comments,
    hashbang,
    directives,
    body,
    &builder,
  ))
}

struct RebaseDirective(Span);
impl VisitMut<'_> for RebaseDirective {
  fn visit_span(&mut self, span: &mut Span) {
    *span = self.0;
  }
}

/// Move each record's finalized statements into its factory before appending the file's code.
fn append_module_bodies<'a>(
  ctx: &GenerateContext<'_>,
  allocator: &'a Allocator,
  asts: &mut [super::owned::ChunkProgram],
  records: &[(rolldown_common::ChunkIdx, std::ops::Range<usize>)],
  generated_base: u32,
  body: &mut ArenaVec<'a, Statement<'a>>,
) {
  let own_len = records.first().map_or(asts.len(), |(_, range)| range.start);
  let mut carried = Vec::with_capacity(records.len());
  for (record, range) in records {
    let capacity = asts[range.clone()].iter().map(|ast| ast.body.len()).sum();
    let mut factory = ArenaVec::with_capacity_in(capacity, &allocator);
    for ast in &mut asts[range.clone()] {
      factory.extend(ast.take_body(allocator));
    }
    carried.push((*record, factory));
  }
  body.extend(crate::ecmascript::inline_records_ast(
    ctx,
    allocator,
    Span::new(generated_base, generated_base),
    carried,
  ));
  let mut graph = ctx
    .options
    .is_dev_mode_enabled()
    .then(|| {
      crate::hmr::module_graph_delta::register_graph_ast(
        &ctx.link_output.module_table,
        ctx.chunk.modules.iter().copied(),
        &ctx.link_output.user_defined_entry_modules,
        allocator,
        Span::new(generated_base, generated_base),
      )
    })
    .flatten();
  let runtime_pos = ctx
    .chunk
    .modules
    .iter()
    .filter(|idx| ctx.link_output.module_table[**idx].is_normal())
    .position(|idx| *idx == ctx.link_output.runtime.id());
  if runtime_pos.is_none() {
    body.extend(graph.take());
  }
  for (pos, ast) in asts[..own_len].iter_mut().enumerate() {
    body.extend(ast.take_body(allocator));
    if runtime_pos == Some(pos) {
      body.extend(graph.take());
    }
  }
}

pub(super) struct OptimizeContext<'a> {
  pub options: &'a NormalizedBundlerOptions,
  pub module_table: &'a ModuleTable,
  #[cfg(feature = "testing")]
  pub filename: Option<&'a str>,
}

pub(super) fn optimize_wrapped<'a>(
  ctx: &OptimizeContext<'_>,
  allocator: &'a Allocator,
  program: &mut Program<'a>,
  layout: ChunkLayout,
  minify_options: &OxcMinifierOptions,
) -> super::owned::OptimizedChunk {
  let space = layout.space;
  let ret = EcmaCompiler::minify_program(
    allocator,
    program,
    minify_options.clone(),
    matches!(ctx.options.minify, MinifyOptions::Enabled(_)),
    ctx.options.sourcemap.is_some(),
  );
  #[cfg(feature = "testing")]
  super::record_coverage(ctx.options, "optimized", "global_wrapped", ctx.filename);
  let mut print_options =
    codegen_options(ctx.options, 0, ctx.options.sourcemap.is_some().then(|| "<chunk>".into()));
  // Donor comments already passed the module filter. Generated addons first meet the
  // final-chunk filter here, matching the ordinary renderer followed by late codegen.
  print_options.comments.normal = !print_options.minify;
  if !print_options.minify {
    print_options.comments.legal = LegalComment::Inline;
  }
  let owners = space
    .modules
    .iter()
    .enumerate()
    .map(|(index, module)| oxc::codegen::OutputRangeOwner {
      source: module.base..module.base + module.len,
      owner: u32::try_from(index).expect("module count fits u32"),
    })
    .collect();
  let printed = Codegen::new()
    .with_output_range_owners(owners)
    .with_options(print_options)
    .with_line_offset_tables(Arc::clone(&space.tables))
    .with_scoping(ret.scoping)
    .with_private_member_mappings(ret.class_private_mappings)
    .build(program);
  let map = printed.map.map(|map| dedupe_sources(remap(&map, &space, None, ctx.module_table).0));
  let mut fragments = vec![String::new(); space.modules.len()];
  for range in printed.output_ranges {
    fragments[range.owner as usize].push_str(&printed.code[range.output]);
  }
  let modules = space
    .modules
    .iter()
    .zip(fragments)
    .map(|(module, code)| {
      let sources: Option<Arc<[Box<dyn rolldown_sourcemap::Source + Send + Sync>]>> =
        (!code.is_empty()).then(|| {
          Arc::from(vec![Box::new(code) as Box<dyn rolldown_sourcemap::Source + Send + Sync>])
        });
      (module.idx, sources)
    })
    .collect();
  super::owned::OptimizedChunk {
    code: printed.code,
    map,
    property_cache: ret.property_mangle_cache,
    modules,
  }
}

/// Optimize the exclusively retained whole program, then freeze module fragments for printing.
pub(super) fn optimize_chunk<'a>(
  ctx: &OptimizeContext<'_>,
  allocator: &'a Allocator,
  asts: &mut [super::owned::ChunkProgram],
  program: &mut Program<'a>,
  layout: ChunkLayout,
  minify_options: &OxcMinifierOptions,
) -> ChunkAstDceOutput {
  let ChunkLayout { mut space, head_base, tail_base, runtime_before_imports } = layout;
  let source_type = program.source_type;
  let enable_sourcemap = ctx.options.sourcemap.is_some();
  let t = Timer::start();
  // Direct codegen retains donor literal spans; parser-produced inputs retain their own
  // final-text provenance. Both enter the same optimizer.
  let minify_ret = EcmaCompiler::minify_program(
    allocator,
    program,
    minify_options.clone(),
    matches!(ctx.options.minify, MinifyOptions::Enabled(_)),
    enable_sourcemap,
  );
  t.stop(Phase::AstDce);
  #[cfg(feature = "testing")]
  super::record_coverage(ctx.options, "optimized", "global", ctx.filename);

  // 3. Split by span ranges.
  let t = Timer::start();
  #[cfg(debug_assertions)]
  let statement_order = statement_addresses(&program.body);
  let module_count = asts.len();
  let mut starts: Vec<_> = space.modules.iter().map(|module| module.base).collect();
  starts.push(head_base);
  starts.push(tail_base);
  let mut head = ArenaVec::new_in(&allocator);
  let mut tail = ArenaVec::new_in(&allocator);
  let mut segments: Vec<_> = (0..module_count).map(|_| ArenaVec::new_in(&allocator)).collect();
  let mut current = 0;
  // Generated statements (head/tail ranges) are placed by *position*, not span: oxc merges
  // `import { t } from "./x"` + `export { t }` into `export { t } from "./x"` at the import's
  // position but with the export's span, and the position decides when `./x` is evaluated.
  let mut seen_module_code = false;
  for stmt in program.body.take_in(&allocator) {
    let span = oxc::span::GetSpan::span(&stmt);
    let owner = starts.partition_point(|start| *start <= span.start).saturating_sub(1);
    // CJS places runtime and requires together before user modules. Keeping runtime-owned
    // statements in the generated segments preserves this order even after inlining.
    let cjs_runtime = runtime_before_imports && owner == 0;
    if owner >= module_count || module_count == 0 || cjs_runtime {
      if seen_module_code { tail.push(stmt) } else { head.push(stmt) }
    } else {
      seen_module_code = true;
      // A node the finalizer synthesized with `SPAN` was rebased to its module's base, so even
      // an empty span names its module. Only a statement the minifier created with `SPAN`
      // (still `0..0`) is ambiguous; it stays with the module before it.
      if span != SPAN {
        current = current.max(owner);
      }
      segments[current].push(stmt);
    }
  }
  #[cfg(debug_assertions)]
  assert_statement_order(&head, &segments, &tail, statement_order);
  // Codegen normally numbers classes from zero for each `Program`. Private-member mangling uses
  // chunk-wide class IDs, so preserve the numbering across the head, module segments and tail.
  // Without private members the offsets are never observed; avoid another whole-AST traversal.
  let has_private_members = minify_ret
    .class_private_mappings
    .as_ref()
    .is_some_and(|mappings| mappings.iter().any(|mapping| !mapping.is_empty()));
  let (class_offsets, tail_class_offset) = if has_private_members {
    let mut class_offsets = Vec::with_capacity(module_count);
    let mut next_class_offset = count_classes(&head);
    for segment in &segments {
      class_offsets.push(next_class_offset);
      next_class_offset += count_classes(segment);
    }
    (class_offsets, next_class_offset)
  } else {
    (vec![0; module_count], 0)
  };

  for (ast, segment) in asts.iter_mut().zip(segments) {
    // SAFETY: ChunkAst retains stable donor/scratch arenas until every root is retired.
    ast.body = unsafe {
      std::mem::transmute::<ArenaVec<'_, Statement<'_>>, ArenaVec<'static, Statement<'static>>>(
        segment,
      )
    };
  }

  t.stop(Phase::AstSplit);
  space.scoping = minify_ret.scoping.map(Arc::new);
  space.private_member_mappings = minify_ret.class_private_mappings.map(Arc::new);
  space.class_offsets = class_offsets;
  // Head and tail may hold code oxc moved out of modules (e.g. `var bar = require_foo().bar`
  // before `export { bar }` gets `require_foo`'s initializer inlined), so they are printed like
  // modules: with the user's comment options and a sourcemap into the moved code.
  let t = Timer::start();
  let (imports, imports_map) = print_generated_in_space(
    allocator,
    ctx.options,
    source_type,
    head,
    &space,
    ctx.module_table,
    0,
  );
  let (exports, exports_map) = print_generated_in_space(
    allocator,
    ctx.options,
    source_type,
    tail,
    &space,
    ctx.module_table,
    tail_class_offset,
  );
  t.stop(Phase::AstSplit);
  #[cfg(feature = "testing")]
  super::record_coverage(ctx.options, "split", "global", ctx.filename);
  ChunkAstDceOutput {
    imports,
    exports,
    imports_map,
    exports_map,
    space: Some(Arc::new(space)),
    property_mangle_cache: minify_ret.property_mangle_cache,
  }
}

#[cfg(debug_assertions)]
fn statement_addresses(body: &[Statement<'_>]) -> Vec<oxc::allocator::Address> {
  use oxc::allocator::GetAddress;
  body.iter().map(GetAddress::address).collect()
}

#[cfg(debug_assertions)]
fn assert_statement_order(
  head: &[Statement<'_>],
  segments: &[ArenaVec<'_, Statement<'_>>],
  tail: &[Statement<'_>],
  expected: Vec<oxc::allocator::Address>,
) {
  use oxc::allocator::GetAddress;
  assert!(
    head
      .iter()
      .chain(segments.iter().flatten())
      .chain(tail.iter())
      .map(GetAddress::address)
      .eq(expected),
    "chunk AST split must preserve the minifier's statement order"
  );
}

fn codegen_options(
  options: &NormalizedBundlerOptions,
  initial_indent: u32,
  source_map_path: Option<std::path::PathBuf>,
) -> CodegenOptions {
  let comments = options.comments;
  let (remove_whitespace, ascii_only) = match &options.minify {
    MinifyOptions::Enabled(options) => (options.remove_whitespace, options.ascii_only),
    _ => (false, false),
  };
  CodegenOptions {
    minify: remove_whitespace,
    comments: CommentOptions {
      // Match module printing followed by chunk printing: normal comments never survive the
      // first print, and legal comments must pass the user's filter before either mode prints.
      normal: false,
      legal: if comments.legal { LegalComment::Inline } else { LegalComment::None },
      jsdoc: comments.jsdoc && !remove_whitespace,
      annotation: comments.annotation,
    },
    initial_indent,
    ascii_only: options.charset.is_ascii() || ascii_only,
    source_map_path,
    ..CodegenOptions::default()
  }
}

fn print_generated_in_space<'a>(
  allocator: &'a Allocator,
  options: &NormalizedBundlerOptions,
  source_type: oxc::span::SourceType,
  stmts: ArenaVec<'a, Statement<'a>>,
  space: &ChunkSourceSpace,
  module_table: &ModuleTable,
  class_offset: usize,
) -> (Option<String>, Option<SourceMap<'static>>) {
  if stmts.is_empty() {
    return (None, None);
  }
  let builder = AstBuilder::new(allocator);
  // SAFETY: `space.text` outlives this print.
  let text: &'a str = unsafe { std::mem::transmute::<&str, &'a str>(&space.text) };
  let program = Program::new(SPAN, source_type, text, [], None, [], stmts, &builder);
  let enable_sourcemap = options.sourcemap.is_some();
  let ret = Codegen::new()
    .with_options(codegen_options(
      options,
      0,
      enable_sourcemap.then(|| std::path::PathBuf::from("<chunk>")),
    ))
    .with_line_offset_tables(Arc::clone(&space.tables))
    .with_shared_scoping(space.scoping.clone())
    .with_shared_private_member_mappings(space.private_member_mappings.clone())
    .with_class_id_offset(class_offset)
    .build(&program);
  // Head/tail maps go straight to the joiner, so they are always composed here.
  (Some(ret.code), ret.map.map(|map| remap(&map, space, None, module_table).0))
}

/// Prints module `pos` of the chunk against `V` and remaps its map to module coordinates.
pub fn print_module(
  module: &NormalModule,
  ast: &Program<'_>,
  options: &NormalizedBundlerOptions,
  initial_indent: u32,
  space: &ChunkSourceSpace,
  pos: usize,
  module_table: &ModuleTable,
) -> ModuleRenderOutput {
  // Even a virtual owner can hold real source after cross-module inlining. Always build the map
  // here; `remap` marks locations outside real source modules as unmapped.
  let enable_sourcemap = options.sourcemap.is_some();
  let ret = Codegen::new()
    .with_options(codegen_options(
      options,
      initial_indent,
      enable_sourcemap.then(|| std::path::PathBuf::from(module.id.as_str())),
    ))
    .with_line_offset_tables(Arc::clone(&space.tables))
    .with_shared_scoping(space.scoping.clone())
    .with_shared_private_member_mappings(space.private_member_mappings.clone())
    .with_class_id_offset(space.class_offsets[pos])
    .build(ast);
  let t = Timer::start();
  let (map, map_is_final) = ret.map.map_or((None, false), |map| {
    let (map, is_final) = remap(&map, space, Some(pos), module_table);
    (Some(map), is_final)
  });
  t.stop(Phase::AstRemap);
  ModuleRenderOutput { code: ret.code, map, map_is_final }
}

/// Remaps a map whose single source is `V` to the chunk's modules. The owner (the printed
/// module, `None` for the generated head/tail) comes first among the sources, so a map without
/// foreign tokens looks exactly like today's module map. Virtual/generated tokens become explicit
/// unmapped segments so preceding real mappings cannot extend into them.
/// See internal-docs/chunk-ast-minify/implementation.md.
///
/// Returns whether the map is final. It is not when the printed module is the only source: the
/// caller composes it with the owner's `sourcemap_chain`, as today. With tokens from other
/// modules (code oxc moved), each source's tokens are composed with that source's chain here
/// when needed, and the map is final even if none of the sources has a chain.
fn remap(
  map: &SourceMap<'_>,
  space: &ChunkSourceSpace,
  owner: Option<usize>,
  module_table: &ModuleTable,
) -> (SourceMap<'static>, bool) {
  // A module map usually has only its owner as a source. Keep that small set sparse instead of
  // initializing a chunk-sized source-id array for every module map.
  let mut source_modules: Vec<usize> = vec![];
  let mut add_source = |pos: usize| -> Option<u32> {
    if let Some(id) = source_modules.iter().position(|&module| module == pos) {
      return Some(u32::try_from(id).expect("sources fit in u32"));
    }
    space.modules[pos].source_name.as_ref()?;
    let id = u32::try_from(source_modules.len()).expect("sources fit in u32");
    source_modules.push(pos);
    Some(id)
  };
  if let Some(owner) = owner {
    add_source(owner);
  }
  let mut tokens = Vec::with_capacity(map.get_tokens().len());
  let owner_lines = owner.map(|owner| space.modules[owner].line..=space.modules[owner].end_line);
  for token in map.get_tokens() {
    let line = token.get_src_line();
    // Almost every token is the printed module's own: skip the search for those.
    let pos = if let (Some(owner), Some(lines)) = (owner, &owner_lines)
      && lines.contains(&line)
    {
      Some(owner)
    } else {
      space
        .modules
        .partition_point(|m| m.line <= line)
        .checked_sub(1)
        .filter(|&pos| line <= space.modules[pos].end_line)
    };
    let source = token
      .get_source_id()
      .and(pos)
      .and_then(|pos| add_source(pos).map(|source_id| (pos, source_id)));
    let remapped = if let Some((pos, source_id)) = source {
      Token::new(
        token.get_dst_line(),
        token.get_dst_col(),
        line - space.modules[pos].line,
        token.get_src_col(),
        Some(source_id),
        token.get_name_id(),
      )
    } else {
      Token::new(token.get_dst_line(), token.get_dst_col(), 0, 0, None, None)
    };
    tokens.push(remapped);
  }
  let names: Vec<Cow<'static, str>> =
    map.get_names().map(|name| Cow::Owned(name.to_string())).collect();
  // A virtual owner contributes no source entry. Its sole source can still be a foreign
  // module whose transform chain must be applied here, rather than the owner's chain later.
  let owner_only = owner.is_some_and(|owner| source_modules.iter().all(|&pos| pos == owner));
  if !owner_only && source_modules.iter().any(|pos| space.modules[*pos].has_chain) {
    return (compose_per_module(space, module_table, &source_modules, &names, tokens), true);
  }
  let (sources, source_contents) =
    source_modules.iter().map(|pos| source_and_content(space, *pos)).unzip();
  let map = SourceMap::from(SourceMapParts {
    file: None,
    names,
    source_root: None,
    sources,
    source_contents,
    tokens: tokens.into_boxed_slice(),
    token_chunks: None,
    ignore_list: None,
    debug_id: None,
  });
  // Foreign sources without transform chains already have final coordinates too. In particular,
  // a virtual owner's transform chain must never be applied to a foreign source's tokens.
  (map, !owner_only)
}

fn source_and_content(
  space: &ChunkSourceSpace,
  pos: usize,
) -> (Cow<'static, str>, Option<Cow<'static, str>>) {
  let module = &space.modules[pos];
  let content = &space.text[module.base as usize..(module.base + module.len) as usize];
  (
    Cow::Owned(module.source_name.clone().unwrap_or_default()),
    Some(Cow::Owned(content.to_string())),
  )
}

/// Splits `tokens` (module coordinates, source id = index into `source_modules`) by source
/// module, composes each group with its module's `sourcemap_chain` (`collapse_module_sourcemap`,
/// as a module's own map is composed today), and merges the groups back in generated order.
fn compose_per_module(
  space: &ChunkSourceSpace,
  module_table: &ModuleTable,
  source_modules: &[usize],
  names: &[Cow<'static, str>],
  tokens: Vec<Token>,
) -> SourceMap<'static> {
  let mut groups: Vec<Vec<Token>> = vec![vec![]; source_modules.len()];
  let mut out_tokens: Vec<Token> = vec![];
  for token in tokens {
    let Some(id) = token.get_source_id() else {
      out_tokens.push(token);
      continue;
    };
    let id = id as usize;
    groups[id].push(Token::new(
      token.get_dst_line(),
      token.get_dst_col(),
      token.get_src_line(),
      token.get_src_col(),
      Some(0),
      token.get_name_id(),
    ));
  }
  let mut out_names: Vec<Cow<'static, str>> = vec![];
  let mut out_sources: Vec<Cow<'static, str>> = vec![];
  let mut out_contents: Vec<Option<Cow<'static, str>>> = vec![];
  // Warnings for omitted plugin maps are reported by `render_ecma_module` for the owner.
  let mut ignored_warnings = vec![];
  for (pos, group) in source_modules.iter().zip(groups) {
    let (source, content) = source_and_content(space, *pos);
    let module = &space.modules[*pos];
    let chain = module_table[module.idx].as_normal().map_or(&[][..], |m| &*m.sourcemap_chain);
    // Collapse can omit unmapped segments that seem redundant within this producer. Foreign
    // producers can intervene on the same generated line, so restore every omitted boundary.
    // See internal-docs/chunk-ast-minify/implementation.md.
    let boundaries: Vec<_> = if chain.is_empty() {
      vec![]
    } else {
      group.iter().map(|token| (token.get_dst_line(), token.get_dst_col())).collect()
    };
    let group_map = SourceMap::from(SourceMapParts {
      file: None,
      names: names.to_vec(),
      source_root: None,
      sources: vec![source],
      source_contents: vec![content],
      tokens: group.into_boxed_slice(),
      token_chunks: None,
      ignore_list: None,
      debug_id: None,
    });
    let Some(composed) = collapse_module_sourcemap(
      chain,
      Some(group_map),
      module.source_name.as_deref().unwrap_or_default(),
      &mut ignored_warnings,
    ) else {
      continue;
    };
    let source_offset = u32::try_from(out_sources.len()).expect("sources fit in u32");
    let name_offset = u32::try_from(out_names.len()).expect("names fit in u32");
    let mut boundaries = boundaries.into_iter().peekable();
    for token in composed.get_tokens() {
      let destination = (token.get_dst_line(), token.get_dst_col());
      while let Some(&(line, column)) = boundaries.peek() {
        if (line, column) > destination {
          break;
        }
        boundaries.next();
        if (line, column) < destination {
          out_tokens.push(Token::new(line, column, 0, 0, None, None));
        }
      }
      out_tokens.push(Token::new(
        token.get_dst_line(),
        token.get_dst_col(),
        token.get_src_line(),
        token.get_src_col(),
        token.get_source_id().map(|id| id + source_offset),
        token.get_name_id().map(|id| id + name_offset),
      ));
    }
    out_tokens.extend(boundaries.map(|(line, column)| Token::new(line, column, 0, 0, None, None)));
    out_names.extend(composed.get_names().map(|n| Cow::Owned(n.to_string())));
    out_sources.extend(composed.get_sources().map(|n| Cow::Owned(n.to_string())));
    out_contents
      .extend(composed.get_source_contents().map(|c| c.map(|c| Cow::Owned(c.to_string()))));
  }
  out_tokens.sort_by_key(|t| (t.get_dst_line(), t.get_dst_col()));
  SourceMap::from(SourceMapParts {
    file: None,
    names: out_names,
    source_root: None,
    sources: out_sources,
    source_contents: out_contents,
    tokens: out_tokens.into_boxed_slice(),
    token_chunks: None,
    ignore_list: None,
    debug_id: None,
  })
}

/// Merges duplicate sources of a joined chunk map (a module appears once per module map that
/// references it).
pub fn dedupe_sources(map: SourceMap<'static>) -> SourceMap<'static> {
  let SourceMapParts {
    file,
    names,
    source_root,
    sources: all_sources,
    source_contents: all_contents,
    tokens,
    token_chunks,
    ignore_list,
    debug_id,
  } = map.into_parts();
  let source_count = all_sources.len();
  let mut index: rustc_hash::FxHashMap<Cow<'static, str>, u32> = rustc_hash::FxHashMap::default();
  let mut sources = Vec::with_capacity(source_count);
  let mut source_contents = Vec::with_capacity(source_count);
  let remap: Vec<u32> = all_sources
    .into_iter()
    .zip(all_contents.into_iter().chain(std::iter::repeat(None)))
    .map(|(source, content)| {
      *index.entry(source.clone()).or_insert_with(|| {
        sources.push(source);
        source_contents.push(content);
        u32::try_from(sources.len() - 1).expect("sources fit in u32")
      })
    })
    .collect();
  let (tokens, token_chunks) = if sources.len() == source_count {
    (tokens, token_chunks)
  } else {
    let tokens = tokens
      .iter()
      .map(|t| {
        Token::new(
          t.get_dst_line(),
          t.get_dst_col(),
          t.get_src_line(),
          t.get_src_col(),
          t.get_source_id().map(|id| remap[id as usize]),
          t.get_name_id(),
        )
      })
      .collect();
    (tokens, None)
  };
  SourceMap::from(SourceMapParts {
    file,
    names,
    source_root,
    sources,
    source_contents,
    tokens,
    token_chunks,
    ignore_list,
    debug_id,
  })
}
