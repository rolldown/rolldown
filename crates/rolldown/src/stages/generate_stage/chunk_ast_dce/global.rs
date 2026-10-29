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

use std::{borrow::Cow, sync::Arc};

use oxc::{
  allocator::{Allocator, TakeIn, Vec as ArenaVec},
  ast::{
    ast::{Program, Statement},
    builder::AstBuilder,
  },
  ast_visit::VisitMut,
  codegen::{Codegen, CodegenOptions, CommentOptions, LegalComment, LineOffsetTables},
  minifier::{Minifier, MinifierOptions as OxcMinifierOptions},
  span::{SPAN, Span},
};
use oxc_sourcemap::{SourceMap, SourceMapParts, Token};
use rolldown_common::{ModuleRenderOutput, NormalModule, NormalizedBundlerOptions};
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::{
  poc_prof::{Phase, Timer},
  rayon::{IntoParallelRefMutIterator, ParallelIterator},
};

use super::{ChunkAstDceOutput, GeneratedText, parse_generated, print_generated};
use crate::types::generator::GenerateContext;

/// The shared source space of one chunk (see the module docs).
pub struct ChunkSourceSpace {
  /// `V`: module sources, then head and tail text, each followed by `\n`.
  text: Arc<str>,
  /// Line tables of `text`; empty when sourcemaps are off.
  tables: Arc<LineOffsetTables>,
  /// Per module, in `chunk.modules` order (normal modules only).
  modules: Vec<SpaceModule>,
}

struct SpaceModule {
  /// Byte offset of the module's source in `V`.
  base: u32,
  len: u32,
  /// Line of `base` in `V`.
  line: u32,
  /// Source name in sourcemaps (`module.id`), `None` for virtual modules (no mappings).
  source_name: Option<String>,
  /// A module whose text came out of plugin transforms maps through its `sourcemap_chain`; only
  /// its own printed tokens are composed with it, so foreign tokens into it are dropped.
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

/// See the module docs. `modules[i]` is the module of `asts[i]`.
pub fn dce_chunk_global(
  ctx: &GenerateContext<'_>,
  allocator: &Allocator,
  asts: &mut [&mut EcmaAst],
  modules: &[&NormalModule],
  generated: &GeneratedText<'_>,
  minify_options: &OxcMinifierOptions,
) -> ChunkAstDceOutput {
  let t = Timer::start();
  let builder = AstBuilder::new(allocator);
  let source_type = ctx.options.format.source_type().with_jsx(true);
  let enable_sourcemap = ctx.options.sourcemap.is_some();

  // 1. `V` and the module offsets.
  let total = asts.iter().map(|ast| ast.program().source_text.len() + 1).sum::<usize>()
    + generated.imports.map_or(0, str::len)
    + generated.exports.map_or(0, str::len)
    + 2;
  let mut text = String::with_capacity(total);
  let mut bases = Vec::with_capacity(asts.len());
  let mut lens = Vec::with_capacity(asts.len());
  for ast in asts.iter() {
    bases.push(u32::try_from(text.len()).expect("chunk sources fit in u32"));
    lens.push(u32::try_from(ast.program().source_text.len()).expect("chunk sources fit in u32"));
    text.push_str(ast.program().source_text);
    text.push('\n');
  }
  let head_base = u32::try_from(text.len()).expect("chunk sources fit in u32");
  text.push_str(generated.imports.unwrap_or_default());
  text.push('\n');
  let tail_base = u32::try_from(text.len()).expect("chunk sources fit in u32");
  text.push_str(generated.exports.unwrap_or_default());
  text.push('\n');
  let text: Arc<str> = Arc::from(text);
  // SAFETY: `text` is kept alive by the returned `ChunkSourceSpace` until the module ASTs are
  // printed and dropped.
  let text_ref: &'static str = unsafe { std::mem::transmute::<&str, &'static str>(&text) };

  // Rebase every module's spans and comments into `V`.
  let mut jobs: Vec<(&mut &mut EcmaAst, u32)> =
    asts.iter_mut().zip(bases.iter().copied()).collect();
  t.stop(Phase::AstSpace);
  jobs.par_iter_mut().for_each(|(ast, base)| {
    let t = Timer::start();
    let base = *base;
    ast.program.with_mut(|fields| {
      Rebase(base).visit_program(fields.program);
      for comment in fields.program.comments.iter_mut() {
        comment.span.start += base;
        comment.span.end += base;
        // `attached_to == 0` is the finalizer's "print at the top of the module" marker for
        // comments whose anchor it removed (imports, directives); codegen prints those through
        // `print_comments_at(0)`. No real comment attaches to offset 0.
        if comment.attached_to != 0 {
          comment.attached_to += base;
        }
      }
      // SAFETY: see `text_ref`.
      fields.program.source_text = unsafe { std::mem::transmute::<&str, &str>(text_ref) };
    });
    t.stop(Phase::AstRebase);
  });
  drop(jobs);
  let t = Timer::start();

  // 2. Assemble without markers: imports, module bodies, tail.
  let mut body: ArenaVec<'_, Statement<'_>> = ArenaVec::with_capacity_in(
    asts.iter().map(|ast| ast.program().body.len()).sum::<usize>() + 2,
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
    ast.program.with_mut(|fields| {
      for stmt in fields.program.body.take_in(&fields.allocator) {
        // SAFETY: the module arena outlives the chunk program; see `super::dce_chunk`.
        body.push(unsafe { std::mem::transmute::<Statement<'_>, Statement<'_>>(stmt) });
      }
    });
  }
  if let Some(exports) = generated.exports {
    let start = body.len();
    parse_generated(allocator, exports, source_type, &mut body);
    for stmt in &mut body[start..] {
      Rebase(tail_base).visit_statement(stmt);
    }
  }
  let mut program = Program::new(SPAN, source_type, text_ref, [], None, [], body, &builder);
  t.stop(Phase::AstAssemble);

  let t = Timer::start();
  Minifier::new(minify_options.clone()).dce(allocator, &mut program);
  t.stop(Phase::AstDce);

  // 3. Split by span ranges.
  let t = Timer::start();
  let module_count = asts.len();
  let mut starts = bases.clone();
  starts.push(head_base);
  starts.push(tail_base);
  let mut head = ArenaVec::new_in(&allocator);
  let mut tail = ArenaVec::new_in(&allocator);
  let mut segments: Vec<_> = (0..module_count).map(|_| ArenaVec::new_in(&allocator)).collect();
  let mut current = 0;
  for stmt in program.body.take_in(&allocator) {
    let span = oxc::span::GetSpan::span(&stmt);
    let owner = starts.partition_point(|start| *start <= span.start).saturating_sub(1);
    if owner == module_count {
      head.push(stmt);
    } else if owner == module_count + 1 {
      tail.push(stmt);
    } else {
      // A node the finalizer synthesized with `SPAN` was rebased to its module's base, so even
      // an empty span names its module. Only a statement the minifier created with `SPAN`
      // (still `0..0`) is ambiguous; it stays with the module before it.
      if span != SPAN {
        current = current.max(owner);
      }
      if module_count == 0 {
        tail.push(stmt);
      } else {
        segments[current].push(stmt);
      }
    }
  }
  for (ast, segment) in asts.iter_mut().zip(segments) {
    ast.program.with_mut(|fields| {
      // SAFETY: see `super::dce_chunk`.
      fields.program.body = unsafe {
        std::mem::transmute::<ArenaVec<'_, Statement<'_>>, ArenaVec<'_, Statement<'_>>>(segment)
      };
    });
  }
  let imports = print_generated(allocator, ctx.options, source_type, head);
  let exports = print_generated(allocator, ctx.options, source_type, tail);

  t.stop(Phase::AstSplit);
  let t = Timer::start();
  let tables =
    if enable_sourcemap { Arc::new(LineOffsetTables::new(&text)) } else { Arc::default() };
  let modules = modules
    .iter()
    .zip(bases.iter().zip(&lens))
    .map(|(module, (&base, &len))| SpaceModule {
      base,
      len,
      line: if enable_sourcemap { tables.line_index_of(base) } else { 0 },
      source_name: (!module.is_virtual()).then(|| module.id.to_string()),
      has_chain: !module.sourcemap_chain.is_empty(),
    })
    .collect();
  let space = ChunkSourceSpace { text, tables, modules };
  t.stop(Phase::AstSpace);
  ChunkAstDceOutput { imports, exports, space: Some(Arc::new(space)) }
}

/// Prints module `pos` of the chunk against `V` and remaps its map to module coordinates.
pub fn print_module(
  module: &NormalModule,
  ast: &EcmaAst,
  options: &NormalizedBundlerOptions,
  initial_indent: u32,
  space: &ChunkSourceSpace,
  pos: usize,
) -> ModuleRenderOutput {
  let enable_sourcemap = options.sourcemap.is_some() && !module.is_virtual();
  let comments = options.comments;
  let ret = Codegen::new()
    .with_options(CodegenOptions {
      comments: CommentOptions {
        normal: false,
        legal: if comments.legal { LegalComment::Inline } else { LegalComment::None },
        jsdoc: comments.jsdoc,
        annotation: comments.annotation,
      },
      initial_indent,
      ascii_only: options.charset.is_ascii(),
      source_map_path: enable_sourcemap.then(|| std::path::PathBuf::from(module.id.as_str())),
      ..CodegenOptions::default()
    })
    .with_line_offset_tables(Arc::clone(&space.tables))
    .build(ast.program());
  let map = ret.map.map(|map| remap(&map, space, pos));
  ModuleRenderOutput { code: ret.code, map }
}

/// Remaps a map whose single source is `V` to the chunk's modules. The owner comes first among
/// the sources, so a map without foreign tokens looks exactly like today's module map.
fn remap(map: &SourceMap<'_>, space: &ChunkSourceSpace, owner: usize) -> SourceMap<'static> {
  let owner_has_chain = space.modules[owner].has_chain;
  // module position -> source id in the output
  let mut source_ids: Vec<Option<u32>> = vec![None; space.modules.len()];
  let mut sources: Vec<Cow<'static, str>> = vec![];
  let mut contents: Vec<Option<Cow<'static, str>>> = vec![];
  let mut add_source = |pos: usize, sources: &mut Vec<_>, contents: &mut Vec<_>| -> Option<u32> {
    if let Some(id) = source_ids[pos] {
      return Some(id);
    }
    let module = &space.modules[pos];
    let name = module.source_name.as_ref()?;
    let id = u32::try_from(sources.len()).expect("sources fit in u32");
    sources.push(Cow::Owned(name.clone()));
    let content = &space.text[module.base as usize..(module.base + module.len) as usize];
    contents.push(Some(Cow::Owned(content.to_string())));
    source_ids[pos] = Some(id);
    Some(id)
  };
  add_source(owner, &mut sources, &mut contents);
  let mut tokens = Vec::with_capacity(map.get_tokens().len());
  for token in map.get_tokens() {
    let line = token.get_src_line();
    let pos = space.modules.partition_point(|m| m.line <= line).saturating_sub(1);
    if pos != owner && (owner_has_chain || space.modules[pos].has_chain) {
      continue;
    }
    let Some(source_id) = add_source(pos, &mut sources, &mut contents) else { continue };
    tokens.push(Token::new(
      token.get_dst_line(),
      token.get_dst_col(),
      line - space.modules[pos].line,
      token.get_src_col(),
      Some(source_id),
      token.get_name_id(),
    ));
  }
  SourceMap::from(SourceMapParts {
    file: None,
    names: map.get_names().map(|name| Cow::Owned(name.to_string())).collect(),
    source_root: None,
    sources,
    source_contents: contents,
    tokens: tokens.into_boxed_slice(),
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
