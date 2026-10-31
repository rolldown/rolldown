//! `ROLLDOWN_CHUNK_AST_DCE=global`: the chunk AST DCE without a module barrier, with correct
//! sourcemaps. See `POC_REPORT.md` ("Size parity").
//!
//! The barrier exists because oxc moves code across module boundaries (single-use inlining), and
//! a moved node keeps spans into its *original* module's source while it is printed with the
//! *target* module's source text and line tables. This mode removes the ambiguity instead:
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
  minifier::{Minifier, MinifierOptions as OxcMinifierOptions},
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
use rolldown_utils::{
  poc_prof::{Phase, Timer},
  rayon::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
  },
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

/// See the module docs. `modules[i]` is the module of `asts[i]`.
#[cfg_attr(
  feature = "testing",
  expect(clippy::too_many_lines, reason = "test-only execution evidence in the ordered pipeline")
)]
pub fn dce_chunk_global(
  ctx: &GenerateContext<'_>,
  allocator: &Allocator,
  asts: &mut [super::owned::ChunkProgram],
  modules: &[&NormalModule],
  generated: &GeneratedText<'_>,
  source_text: &mut Option<Arc<str>>,
  minify_options: &OxcMinifierOptions,
) -> ChunkAstDceOutput {
  let t = Timer::start();
  let builder = AstBuilder::new(allocator);
  let source_type = ctx.options.format.source_type().with_jsx(true);
  let enable_sourcemap = ctx.options.sourcemap.is_some();

  // 1. `V` and the module offsets.
  let total = asts.iter().map(|ast| ast.source_text.len() + 1).sum::<usize>()
    + generated.imports.map_or(0, str::len)
    + generated.exports.map_or(0, str::len)
    + 2;
  let mut text = String::with_capacity(total);
  let mut bases = Vec::with_capacity(asts.len());
  let mut lens = Vec::with_capacity(asts.len());
  for ast in asts.iter() {
    bases.push(u32::try_from(text.len()).expect("chunk sources fit in u32"));
    lens.push(u32::try_from(ast.source_text.len()).expect("chunk sources fit in u32"));
    text.push_str(ast.source_text);
    text.push('\n');
  }
  let head_base = u32::try_from(text.len()).expect("chunk sources fit in u32");
  text.push_str(generated.imports.unwrap_or_default());
  text.push('\n');
  let tail_base = u32::try_from(text.len()).expect("chunk sources fit in u32");
  text.push_str(generated.exports.unwrap_or_default());
  text.push('\n');
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
        // Zero marks comments whose original anchor was removed by finalization.
        if comment.attached_to != 0 {
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
  if let Some(imports) = generated.imports {
    let start = body.len();
    parse_generated(allocator, imports, source_type, &mut body);
    for stmt in &mut body[start..] {
      Rebase(head_base).visit_statement(stmt);
    }
  }
  for ast in asts.iter_mut() {
    for stmt in ast.take_body(allocator) {
      body.push(stmt);
    }
  }
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
  if enable_sourcemap {
    for ast in asts.iter() {
      comments.extend(ast.comments.iter().cloned());
    }
  }
  let mut program = Program::new(SPAN, source_type, text_ref, comments, None, [], body, &builder);
  t.stop(Phase::AstAssemble);

  let t = Timer::start();
  let minify_ret = if *rolldown_utils::poc_prof::ENABLED {
    EcmaCompiler::minify_profiled(
      allocator,
      &mut program,
      minify_options.clone(),
      matches!(ctx.options.minify, MinifyOptions::Enabled(_)),
      enable_sourcemap,
    )
  } else {
    // Direct codegen needs donor literal spans after cross-module inlining.
    // See internal-docs/chunk-ast-minify/implementation.md.
    let minifier = Minifier::new(minify_options.clone()).with_literal_spans(enable_sourcemap);
    if matches!(ctx.options.minify, MinifyOptions::Enabled(_)) {
      minifier.minify(allocator, &mut program)
    } else {
      minifier.dce(allocator, &mut program)
    }
  };
  t.stop(Phase::AstDce);
  #[cfg(feature = "testing")]
  super::record_coverage(
    ctx.options,
    "optimized",
    "global",
    ctx.chunk.preliminary_filename.as_deref().map(arcstr::ArcStr::as_str),
  );

  // 3. Split by span ranges.
  let t = Timer::start();
  #[cfg(debug_assertions)]
  let statement_order = statement_addresses(&program.body);
  let module_count = asts.len();
  let mut starts = bases.clone();
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
    if owner >= module_count || module_count == 0 {
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
  let modules = modules
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
    modules,
    scoping: minify_ret.scoping.map(Arc::new),
    private_member_mappings: minify_ret.class_private_mappings.map(Arc::new),
    class_offsets,
  };
  t.stop(Phase::AstSpace);
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
    &ctx.link_output.module_table,
    0,
  );
  let (exports, exports_map) = print_generated_in_space(
    allocator,
    ctx.options,
    source_type,
    tail,
    &space,
    &ctx.link_output.module_table,
    tail_class_offset,
  );
  t.stop(Phase::AstSplit);
  #[cfg(feature = "testing")]
  super::record_coverage(
    ctx.options,
    "split",
    "global",
    ctx.chunk.preliminary_filename.as_deref().map(arcstr::ArcStr::as_str),
  );
  ChunkAstDceOutput { imports, exports, imports_map, exports_map, space: Some(Arc::new(space)) }
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
