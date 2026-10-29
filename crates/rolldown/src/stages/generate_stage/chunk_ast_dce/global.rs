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
use rolldown_common::{
  ModuleIdx, ModuleRenderOutput, ModuleTable, NormalModule, NormalizedBundlerOptions,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_utils::{
  poc_prof::{Phase, Timer},
  rayon::{
    IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
  },
};

use super::{ChunkAstDceOutput, GeneratedText, parse_generated};
use crate::{
  types::generator::GenerateContext, utils::render_ecma_module::collapse_module_sourcemap,
};

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
  t.stop(Phase::AstSpace);
  // The module's line tables are built in the same parallel job; `concat`ed into `V`'s below.
  let module_tables: Vec<LineOffsetTables> = asts
    .par_iter_mut()
    .zip(bases.par_iter())
    .map(|(ast, base)| {
      let t = Timer::start();
      let base = *base;
      let tables = if enable_sourcemap {
        LineOffsetTables::new(ast.program().source_text)
      } else {
        LineOffsetTables::default()
      };
      ast.program.with_mut(|fields| {
        Rebase(base).visit_program(fields.program);
        for comment in &mut fields.program.comments {
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
      tables
    })
    .collect();
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
  for (ast, segment) in asts.iter_mut().zip(segments) {
    ast.program.with_mut(|fields| {
      // SAFETY: see `super::dce_chunk`.
      fields.program.body = unsafe {
        std::mem::transmute::<ArenaVec<'_, Statement<'_>>, ArenaVec<'_, Statement<'_>>>(segment)
      };
    });
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
  let space = ChunkSourceSpace { text, tables, modules };
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
  );
  let (exports, exports_map) = print_generated_in_space(
    allocator,
    ctx.options,
    source_type,
    tail,
    &space,
    &ctx.link_output.module_table,
  );
  t.stop(Phase::AstSplit);
  ChunkAstDceOutput { imports, exports, imports_map, exports_map, space: Some(Arc::new(space)) }
}

fn codegen_options(
  options: &NormalizedBundlerOptions,
  initial_indent: u32,
  source_map_path: Option<std::path::PathBuf>,
) -> CodegenOptions {
  let comments = options.comments;
  CodegenOptions {
    comments: CommentOptions {
      normal: false,
      legal: if comments.legal { LegalComment::Inline } else { LegalComment::None },
      jsdoc: comments.jsdoc,
      annotation: comments.annotation,
    },
    initial_indent,
    ascii_only: options.charset.is_ascii(),
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
    .build(&program);
  // Head/tail maps go straight to the joiner, so they are always composed here.
  (Some(ret.code), ret.map.map(|map| remap(&map, space, None, module_table).0))
}

/// Prints module `pos` of the chunk against `V` and remaps its map to module coordinates.
pub fn print_module(
  module: &NormalModule,
  ast: &EcmaAst,
  options: &NormalizedBundlerOptions,
  initial_indent: u32,
  space: &ChunkSourceSpace,
  pos: usize,
  module_table: &ModuleTable,
) -> ModuleRenderOutput {
  let enable_sourcemap = options.sourcemap.is_some() && !module.is_virtual();
  let ret = Codegen::new()
    .with_options(codegen_options(
      options,
      initial_indent,
      enable_sourcemap.then(|| std::path::PathBuf::from(module.id.as_str())),
    ))
    .with_line_offset_tables(Arc::clone(&space.tables))
    .build(ast.program());
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
/// foreign tokens looks exactly like today's module map. Tokens into generated text are dropped.
///
/// Returns whether the map is final. It is not when the printed module is the only source: the
/// caller composes it with the owner's `sourcemap_chain`, as today. With tokens from other
/// modules (code oxc moved), when any of the sources has a chain, each source's tokens are
/// composed with that source's chain here and the map is final.
fn remap(
  map: &SourceMap<'_>,
  space: &ChunkSourceSpace,
  owner: Option<usize>,
  module_table: &ModuleTable,
) -> (SourceMap<'static>, bool) {
  // module position -> source id in the output, and back
  let mut source_ids: Vec<Option<u32>> = vec![None; space.modules.len()];
  let mut source_modules: Vec<usize> = vec![];
  let mut add_source = |pos: usize| -> Option<u32> {
    if let Some(id) = source_ids[pos] {
      return Some(id);
    }
    space.modules[pos].source_name.as_ref()?;
    let id = u32::try_from(source_modules.len()).expect("sources fit in u32");
    source_modules.push(pos);
    source_ids[pos] = Some(id);
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
      owner
    } else {
      let Some(pos) = space.modules.partition_point(|m| m.line <= line).checked_sub(1) else {
        continue;
      };
      if line > space.modules[pos].end_line {
        continue;
      }
      pos
    };
    let Some(source_id) = add_source(pos) else { continue };
    tokens.push(Token::new(
      token.get_dst_line(),
      token.get_dst_col(),
      line - space.modules[pos].line,
      token.get_src_col(),
      Some(source_id),
      token.get_name_id(),
    ));
  }
  let names: Vec<Cow<'static, str>> =
    map.get_names().map(|name| Cow::Owned(name.to_string())).collect();
  let owner_only = source_modules.len() <= 1 && owner.is_some();
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
  (map, false)
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
  for token in tokens {
    let id = token.get_source_id().expect("remapped tokens have a source") as usize;
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
  let mut out_tokens: Vec<Token> = vec![];
  // Warnings for omitted plugin maps are reported by `render_ecma_module` for the owner.
  let mut ignored_warnings = vec![];
  for (pos, group) in source_modules.iter().zip(groups) {
    let (source, content) = source_and_content(space, *pos);
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
    let module = &space.modules[*pos];
    let chain = module_table[module.idx].as_normal().map_or(&[][..], |m| &*m.sourcemap_chain);
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
    out_tokens.extend(composed.get_tokens().map(|t| {
      Token::new(
        t.get_dst_line(),
        t.get_dst_col(),
        t.get_src_line(),
        t.get_src_col(),
        t.get_source_id().map(|id| id + source_offset),
        t.get_name_id().map(|id| id + name_offset),
      )
    }));
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
