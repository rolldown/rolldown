use std::sync::Arc;

use oxc::{
  allocator::{Allocator, Vec as ArenaVec},
  ast::{
    ast::{Comment, Program},
    builder::AstBuilder,
  },
  span::GetSpan,
};
use rolldown_common::{
  AttachDebugInfo, ModuleRenderOutput, NormalModule, NormalizedBundlerOptions,
  SourcemapChainElement,
};
use rolldown_error::BuildDiagnostic;
use rolldown_sourcemap::{
  Source, SourceMap, SourceMapSource, collapse_sourcemaps_traced, empty_sourcemap,
};
use rolldown_utils::concat_string;

pub struct RenderEcmaModuleOutput {
  pub sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
  pub warnings: Vec<BuildDiagnostic>,
}

/// Prints `ast` and collapses the module sourcemap chain against the
/// codegen map. A non-empty chain is remapped without first
/// `into_owned`-ing the codegen map (that copy is discarded by the collapse).
/// An empty chain still owns the codegen map, which is the final module map.
pub fn render_normal_module(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  ast: &rolldown_ecmascript::EcmaAst,
  initial_indent: u32,
) -> ModuleRenderOutput {
  render_program(module, options, ast.program(), initial_indent)
}

/// Moves the body of a root or a member of a deferred group (the statements from
/// `body_start` on) out of the program of `ast`, and prints it on its own, as
/// `render_normal_module` does. The program keeps the top-level declarations, which
/// `render_normal_module` prints later. Each comment goes to the part where a print of the
/// whole program puts it (`take_group_body_comments`).
/// See internal-docs/import-defer/implementation.md ("Rendering").
pub fn render_normal_module_group_body(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  ast: &mut rolldown_ecmascript::EcmaAst,
  body_start: usize,
) -> ModuleRenderOutput {
  ast.program.with_mut(|fields| {
    let program = fields.program;
    let allocator = fields.allocator;
    let builder = AstBuilder::new(allocator);
    let comments = take_group_body_comments(program, body_start, allocator);
    let body = ArenaVec::from_iter_in(program.body.drain(body_start..), &allocator);
    let body_program = Program::new(
      program.span,
      program.source_type,
      program.source_text,
      comments,
      None,
      [],
      body,
      &builder,
    );
    render_program(module, options, &body_program, 0)
  })
}

/// Prints `program`, the program of a module or a part of it (see `render_normal_module`).
fn render_program(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  program: &Program<'_>,
  initial_indent: u32,
) -> ModuleRenderOutput {
  let enable_sourcemap = options.sourcemap.is_some() && !module.is_virtual();
  let mut print_comments: rolldown_ecmascript::PrintCommentsOptions = options.comments.into();
  // Annotation comments must survive into the rendered text because
  // `minify_chunks` re-parses it; without them the parser won't set
  // `expr.pure` and DCE can't eliminate unused pure calls.
  // `minify_chunks` codegen will honour the user's `comments.annotation`
  // setting when it emits the final output.
  print_comments.annotation = true;
  let printed = rolldown_ecmascript::EcmaCompiler::print_program_with(
    program,
    rolldown_ecmascript::PrintOptions {
      sourcemap: enable_sourcemap,
      filename: module.id.to_string(),
      comments: print_comments,
      initial_indent,
      ascii_only: options.charset.is_ascii(),
    },
  );
  let mut warnings = Vec::new();
  // Empty body: nothing from this module reaches the chunk, so do not
  // collapse (or warn). See `render_ecma_module`.
  let map = if enable_sourcemap && !printed.code.is_empty() {
    collapse_module_sourcemap(
      &module.sourcemap_chain,
      printed.map,
      module.id.as_str(),
      &mut warnings,
    )
  } else {
    None
  };
  ModuleRenderOutput { code: printed.code, map, warnings }
}

/// Takes from `program` the comments that a print of the whole program puts in its statements
/// from `body_start` on (the body of a root or a member of a deferred group). `program` keeps
/// the other comments. A comment goes to the declarations (the statements before `body_start`):
/// - at position 0: the print puts these at the start of the program.
/// - in a declaration: its anchor is there. A declaration is a hoisted function, a module
///   declaration, or a statement that the finalizer made, which has an empty span and no
///   comments in it.
/// - a legal comment before the start of a declaration: the print puts a legal comment before
///   the first statement that starts after it, also if its anchor is gone.
fn take_group_body_comments<'a>(
  program: &mut Program<'a>,
  body_start: usize,
  allocator: &'a Allocator,
) -> ArenaVec<'a, Comment> {
  let mut declaration_spans = program.body[..body_start]
    .iter()
    .map(GetSpan::span)
    .filter(|span| !span.is_empty())
    .collect::<Vec<_>>();
  declaration_spans.sort_unstable_by_key(|span| span.start);
  let last_declaration_start = declaration_spans.last().map(|span| span.start);
  let in_declarations = |comment: &Comment| {
    let position = comment.attached_to;
    let idx = declaration_spans.partition_point(|span| span.start <= position);
    position == 0
      || (idx > 0 && position < declaration_spans[idx - 1].end)
      || ((comment.is_legal() || comment.is_coverage_ignore_file())
        && last_declaration_start.is_some_and(|start| start > position))
  };
  let mut declaration_comments = ArenaVec::new_in(&allocator);
  let mut body_comments = ArenaVec::new_in(&allocator);
  for comment in &program.comments {
    if in_declarations(comment) {
      declaration_comments.push(comment.clone());
    } else {
      body_comments.push(comment.clone());
    }
  }
  program.comments = declaration_comments;
  body_comments
}

pub fn render_ecma_module(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  render_output: ModuleRenderOutput,
) -> RenderEcmaModuleOutput {
  if render_output.code.is_empty() {
    // Match Rollup: `SOURCEMAP_BROKEN` is reported only for modules whose
    // code actually reaches a chunk. A module rewritten to imports only
    // prints an empty body and must not warn.
    return RenderEcmaModuleOutput { sources: None, warnings: vec![] };
  }
  let mut sources: Vec<Box<dyn rolldown_sourcemap::Source + Send + Sync>> = Vec::with_capacity(6);
  let warnings = push_module_sources(module, options, render_output, true, &mut sources);
  RenderEcmaModuleOutput { sources: Some(Arc::from(sources.into_boxed_slice())), warnings }
}

/// Renders a root or a member of a deferred group in two parts: its top-level declarations, and
/// its body, which `render_normal_module_group_body` printed on its own. Returns the sources of
/// both parts in one list, and the number of sources of the first part. The chunk renderer puts
/// the second part in the init wrapper of the root.
/// See internal-docs/import-defer/implementation.md ("Rendering").
pub fn render_grouped_ecma_module(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  declarations: ModuleRenderOutput,
  body: Option<ModuleRenderOutput>,
) -> (RenderEcmaModuleOutput, usize) {
  let mut sources: Vec<Box<dyn rolldown_sourcemap::Source + Send + Sync>> = Vec::with_capacity(6);
  let mut warnings = vec![];
  let declarations = without_end_newline(declarations);
  if !declarations.code.is_empty() {
    warnings = push_module_sources(module, options, declarations, true, &mut sources);
  }
  let declarations_len = sources.len();
  if let Some(body) = body.map(without_end_newline)
    && !body.code.trim().is_empty()
  {
    // The chunk renderer adds the debug comment of the body, because only it knows if a member
    // body comes before it. The two parts have the same sourcemap warnings.
    let body_warnings = push_module_sources(module, options, body, false, &mut sources);
    if declarations_len == 0 {
      warnings = body_warnings;
    }
  }
  let sources = (!sources.is_empty()).then(|| Arc::from(sources.into_boxed_slice()));
  (RenderEcmaModuleOutput { sources, warnings }, declarations_len)
}

/// The source joiner adds a newline between two sources.
fn without_end_newline(mut output: ModuleRenderOutput) -> ModuleRenderOutput {
  let len = output.code.trim_end_matches('\n').len();
  output.code.truncate(len);
  output
}

/// Pushes the code of `render_output` to `sources`, and returns its sourcemap warnings.
fn push_module_sources(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
  render_output: ModuleRenderOutput,
  debug_comment: bool,
  sources: &mut Vec<Box<dyn rolldown_sourcemap::Source + Send + Sync>>,
) -> Vec<BuildDiagnostic> {
  // Mark only where a module starts, like esbuild and bun.
  if debug_comment && let Some(comment) = module_debug_comment(module, options) {
    sources.push(Box::new(comment));
  }

  let enable_sourcemap = options.sourcemap.is_some() && !module.is_virtual();
  // Chain collapse (and its `SOURCEMAP_BROKEN` warnings) already ran in
  // `render_normal_module` while the codegen map was still borrowed.
  let ModuleRenderOutput { code, map, warnings } = render_output;

  if enable_sourcemap {
    if let Some(sourcemap) = map {
      sources.push(Box::new(
        SourceMapSource::new(code, sourcemap)
          .with_pre_compute_sourcemap_data(options.is_sourcemap_enabled()),
      ));
    } else {
      sources.push(Box::new(code));
    }
  } else {
    sources.push(Box::new(code));
  }
  warnings
}

/// The comment that marks the start of the code of a module (`// <debug id>`), if the
/// `attachDebugInfo` option asks for it.
pub fn module_debug_comment(
  module: &NormalModule,
  options: &NormalizedBundlerOptions,
) -> Option<String> {
  matches!(
    options.experimental.attach_debug_info,
    Some(AttachDebugInfo::Simple | AttachDebugInfo::Full)
  )
  .then(|| concat_string!("// ", module.debug_id))
}

/// Collapses a module's `sourcemap_chain` together with the oxc codegen map
/// (`codegen_map`, always the last element of the chain) into the final module
/// sourcemap. Returns `None` when there is no sourcemap to emit.
///
/// `codegen_map` may borrow the module source (an oxc codegen map). The
/// result is always an owned map. When a non-empty chain is remapped,
/// `collapse_sourcemaps_traced` copies only the strings it keeps, so the
/// codegen map is not `into_owned`'d first. When the chain is empty the
/// codegen map *is* the result and is copied to `'static`.
///
/// Like Rollup, the result keeps only the sources and names that a traced token
/// references. So a transform that returns `{ mappings: '' }` or omits its map
/// removes the module's earlier code, and the names that came with it.
///
/// A `SOURCEMAP_BROKEN` warning is pushed onto `warnings` for every plugin that
/// omitted its sourcemap, since that breaks the mapping chain for `module_id`.
pub fn collapse_module_sourcemap(
  sourcemap_chain: &[SourcemapChainElement],
  codegen_map: Option<rolldown_sourcemap::SourceMapRef<'_>>,
  module_id: &str,
  warnings: &mut Vec<BuildDiagnostic>,
) -> Option<SourceMap> {
  if sourcemap_chain.is_empty() {
    return codegen_map.map(rolldown_sourcemap::SourceMapRef::into_owned);
  }

  let empty = empty_sourcemap();
  // Plugin/load maps are already `'static`. Keep them as `'static` refs and
  // only widen to `SourceMapRef<'_>` at the traced-collapse call, together
  // with the codegen map.
  let mut static_chain: Vec<&SourceMap> = Vec::with_capacity(sourcemap_chain.len());

  let mut original_content: Option<&str> = None;
  for element in sourcemap_chain {
    match element {
      SourcemapChainElement::Transform((_, sourcemap)) | SourcemapChainElement::Load(sourcemap) => {
        static_chain.push(sourcemap);
      }
      SourcemapChainElement::MagicString((_, pending)) => {
        static_chain.push(pending.get());
      }
      SourcemapChainElement::Omitted { plugin_name, .. } => {
        static_chain.push(&empty);
        warnings.push(
          BuildDiagnostic::sourcemap_broken(plugin_name.to_string(), Some(module_id.to_string()))
            .with_severity_warning(),
        );
      }
      SourcemapChainElement::Null { original_content: content, .. } => {
        // `map: null` does not remap positions, so it contributes nothing
        // to `collapse_sourcemaps`. We only keep its pre-transform content as a
        // fallback when there is no real map to provide one.
        if original_content.is_none() {
          original_content = Some(content);
        }
      }
    }
  }
  // The collapsed map takes its sources from the first map.
  // The chain is not changed, because `this.getCombinedSourcemap()` reads it.
  let resolved_first = static_chain
    .first()
    .and_then(|first| rolldown_sourcemap::resolve_sources_against_module(first, module_id));
  if let Some(resolved_first) = resolved_first.as_ref() {
    static_chain[0] = resolved_first;
  }
  if static_chain.is_empty() {
    // Only `map: null` transforms touched this module: keep the codegen
    // map's positions but swap in the pre-transform source content so the
    // transformed/injected code does not leak into `sourcesContent`.
    return codegen_map.map(|map| {
      let mut map = map.into_owned();
      if let Some(content) = original_content {
        map.set_source_contents(vec![Some(content)]);
      }
      map
    });
  }

  // Borrow the codegen map directly. `collapse_sourcemaps_traced` copies
  // only the strings it keeps, so an `into_owned` of the codegen map
  // first would duplicate sourcesContent/names and then drop them.
  let mut chain: Vec<&rolldown_sourcemap::SourceMapRef<'_>> =
    Vec::with_capacity(static_chain.len() + 1);
  for map in &static_chain {
    chain.push(coerce_static_map_ref(map));
  }
  if let Some(sourcemap) = codegen_map.as_ref() {
    chain.push(sourcemap);
  }
  Some(collapse_sourcemaps_traced(&chain))
}

/// Reborrow a `'static` sourcemap at a shorter lifetime so it can share a
/// collapse chain with a codegen map that borrows its source.
/// `oxc_sourcemap::SourceMap` is covariant in that lifetime.
fn coerce_static_map_ref(map: &SourceMap) -> &rolldown_sourcemap::SourceMapRef<'_> {
  map
}

#[cfg(test)]
mod tests {
  use super::collapse_module_sourcemap;
  use arcstr::ArcStr;
  use insta::assert_snapshot;
  use rolldown_common::{PluginIdx, SourcemapChainElement};
  use rolldown_sourcemap::{SourceMap, SourceMapBuilder, empty_sourcemap};

  const MODULE_ID: &str = "/project/src/index.js";

  fn plugin_idx() -> PluginIdx {
    PluginIdx::from_usize(0)
  }

  /// Builds a sourcemap for `source`/`content` with two tokens at columns 0 and 6 of the
  /// first line. The tokens are identity mappings so they line up across maps and survive
  /// `collapse_sourcemaps`' lookups.
  fn map(source: &str, content: &str) -> SourceMap {
    let mut builder = SourceMapBuilder::default();
    let source_id = builder.add_source_and_content(source, content);
    builder.add_token(0, 0, 0, 0, Some(source_id), None);
    builder.add_token(0, 6, 0, 6, Some(source_id), None);
    builder.into_sourcemap().into_owned()
  }

  /// The oxc codegen map is always the last element of the chain and, in production, uses
  /// the module id as its source name (see `render_normal_module`).
  fn codegen_map(content: &str) -> SourceMap {
    map(MODULE_ID, content)
  }

  fn omitted() -> SourcemapChainElement {
    SourcemapChainElement::Omitted { plugin_idx: plugin_idx(), plugin_name: ArcStr::from("plugin") }
  }

  fn null(original_content: &str) -> SourcemapChainElement {
    SourcemapChainElement::Null {
      plugin_idx: plugin_idx(),
      original_content: ArcStr::from(original_content),
    }
  }

  #[test]
  fn empty_chain_returns_codegen_map_unchanged() {
    let mut warnings = vec![];
    let codegen = codegen_map("const a = 1;\n");
    let result = collapse_module_sourcemap(&[], Some(codegen), MODULE_ID, &mut warnings).unwrap();
    assert_snapshot!(result.to_json_string(), @r#"{"version":3,"names":[],"sources":["/project/src/index.js"],"sourcesContent":["const a = 1;\n"],"mappings":"AAAA,MAAM"}"#);
    assert!(warnings.is_empty());

    assert!(collapse_module_sourcemap(&[], None, MODULE_ID, &mut warnings).is_none());
  }

  #[test]
  fn omitted_only_with_codegen_map_uses_module_id_as_source() {
    let mut warnings = vec![];
    let codegen = codegen_map("const a = 1;\n");
    let result =
      collapse_module_sourcemap(&[omitted()], Some(codegen), MODULE_ID, &mut warnings).unwrap();
    assert_snapshot!(result.to_json_string(), @r#"{"version":3,"names":[],"sources":[],"mappings":""}"#);
    // An omitted sourcemap breaks the chain, so a `SOURCEMAP_BROKEN` warning is emitted.
    assert_eq!(warnings.len(), 1);
  }

  #[test]
  fn null_only_with_codegen_map_swaps_source_content() {
    // `map: null` keeps the codegen map's positions/sources but swaps in the pre-transform
    // source content so the transformed code does not leak into `sourcesContent`.
    let mut warnings = vec![];
    let codegen = codegen_map("a(1);\n");
    let result =
      collapse_module_sourcemap(&[null("const a = 1;\n")], Some(codegen), MODULE_ID, &mut warnings)
        .unwrap();
    assert_snapshot!(result.to_json_string(), @r#"{"version":3,"names":[],"sources":["/project/src/index.js"],"sourcesContent":["const a = 1;\n"],"mappings":"AAAA,MAAM"}"#);
    assert!(warnings.is_empty());
  }

  #[test]
  fn null_only_without_codegen_map_returns_none() {
    let mut warnings = vec![];
    assert!(
      collapse_module_sourcemap(&[null("const a = 1;\n")], None, MODULE_ID, &mut warnings)
        .is_none()
    );
  }

  #[test]
  fn real_map_is_not_replaced_by_placeholder() {
    // A real `Load`/`Transform` map wins over the module id placeholder: its sources, content
    // and mappings flow through the collapse.
    let mut warnings = vec![];
    let load = SourcemapChainElement::Load(map("/project/src/loaded.js", "const a = 1;\n"));
    let codegen = codegen_map("a(1);\n");
    let result =
      collapse_module_sourcemap(&[load], Some(codegen), MODULE_ID, &mut warnings).unwrap();
    assert_snapshot!(result.to_json_string(), @r#"{"version":3,"names":[],"sources":["/project/src/loaded.js"],"sourcesContent":["const a = 1;\n"],"mappings":"AAAA,MAAM"}"#);
    assert!(warnings.is_empty());
  }

  #[test]
  fn map_without_mappings_drops_earlier_sources_and_names() {
    // `{ mappings: '' }` or an omitted map after a real map leaves no token that reaches the
    // original code, so its source, content and names must not survive (#11176).
    let mut builder = SourceMapBuilder::default();
    let source_id = builder.add_source_and_content(MODULE_ID, "const secret = 1;\n");
    let name_id = builder.add_name("secret");
    builder.add_token(0, 6, 0, 6, Some(source_id), Some(name_id));
    let earlier =
      SourcemapChainElement::Transform((plugin_idx(), builder.into_sourcemap().into_owned()));
    for last in [SourcemapChainElement::Transform((plugin_idx(), empty_sourcemap())), omitted()] {
      let mut warnings = vec![];
      let codegen = codegen_map("proxy();\n");
      let result = collapse_module_sourcemap(
        &[earlier.clone(), last],
        Some(codegen),
        MODULE_ID,
        &mut warnings,
      )
      .unwrap();
      assert_eq!(result.to_json_string(), r#"{"version":3,"names":[],"sources":[],"mappings":""}"#);
    }
  }

  #[cfg(not(windows))]
  #[test]
  fn relative_sources_of_the_first_map_are_resolved_against_the_module() {
    let mut builder = SourceMapBuilder::default();
    let a = builder.add_source_and_content("../lib/a.ts", "const a = 1;\n");
    let b = builder.add_source_and_content("../lib/b.ts", "const b = 2;\n");
    builder.add_token(0, 0, 0, 0, Some(a), None);
    builder.add_token(0, 6, 0, 6, Some(b), None);
    let transform = builder.into_sourcemap().into_owned();
    let chain = [SourcemapChainElement::Transform((plugin_idx(), transform))];

    let mut warnings = vec![];
    let codegen = codegen_map("const a = 1;\n");
    let result =
      collapse_module_sourcemap(&chain, Some(codegen), MODULE_ID, &mut warnings).unwrap();
    assert_eq!(
      result.get_sources().collect::<Vec<_>>(),
      [Some("/project/lib/a.ts"), Some("/project/lib/b.ts")]
    );
    let SourcemapChainElement::Transform((_, transform)) = &chain[0] else { unreachable!() };
    assert_eq!(transform.get_source(0), Some("../lib/a.ts"));
  }
}
