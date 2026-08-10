use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use oxc::allocator::Allocator;
use oxc::ast::ast::{
  ArrayExpressionElement, BindingPattern, Declaration, ExportNamedDeclaration, Expression,
  ImportDeclaration, ImportDeclarationSpecifier, ModuleExportName, Statement,
  TSNamespaceDeclarationBody, VariableDeclaration,
};
use oxc::ast_visit::Visit;
use oxc::codegen::{Codegen, CodegenOptions};
use oxc::parser::Parser;
use oxc::span::{GetSpan, SourceType};
use oxc_sourcemap::{ConcatSourceMapBuilder, SourceMap, Token};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_utils::collect_reference_directives_from_program,
  codegen,
  dependencies::DependencyCollector,
  filename,
  helpers::HelperTransformer,
  import_export::ImportExportRewriter,
  inline_import::InlineImportCollector,
  parser::TypeScriptParser,
  type_params::TypeParamCollector,
  types::{ChunkInfo, DeclarationInfo, FakeJsOptions, PluginState, Result, TransformResult},
  visitor::{DeclarationCollector, DeclarationNode},
};

#[expect(clippy::large_enum_variant)]
enum RenderAction {
  Keep { text: String, map: Option<SourceMap<'static>> },
  Remove,
}

impl RenderAction {
  fn keep(text: impl Into<String>) -> Self {
    Self::Keep { text: text.into(), map: None }
  }

  fn keep_mapped(text: impl Into<String>, map: Option<SourceMap<'static>>) -> Self {
    Self::Keep { text: text.into(), map }
  }
}

/// Information extracted from a runtime binding variable declaration
/// e.g., `var Foo$1 = [0, (T$1) => [Bar$1, Baz], ["child1"]], Bar`
struct RuntimeBindingInfo {
  declaration_id: usize,
  /// Binding names from all var declarators (e.g., `Foo$1`, `Bar$1` after Rolldown renaming)
  binding_names: Vec<String>,
  renamed_params: Vec<String>,
  renamed_deps: Vec<String>,
}

#[derive(Debug)]
pub struct FakeJsPlugin {
  options: FakeJsOptions,
  state: Arc<Mutex<PluginState>>,
}

impl FakeJsPlugin {
  pub fn new(options: FakeJsOptions) -> Self {
    Self { options, state: Arc::new(Mutex::new(PluginState::new())) }
  }

  pub fn transform(&self, code: &str, id: &str) -> Result<TransformResult> {
    if !filename::is_dts(id) {
      return Ok(TransformResult { code: code.to_string(), map: None });
    }

    let mut state = self.state.lock().unwrap();
    self.transform_declarations(code, id, &mut state)
  }

  fn transform_declarations(
    &self,
    code: &str,
    id: &str,
    state: &mut PluginState,
  ) -> Result<TransformResult> {
    let allocator = Allocator::default();
    let parser = TypeScriptParser::new(&allocator);

    let parse_result = parser.parse(code, id)?;

    let directives = collect_reference_directives_from_program(&parse_result.program, code);
    if !directives.is_empty() {
      state.comments_map.insert(id.to_string(), directives);
    }

    let mut collector = DeclarationCollector::new();
    collector.visit_program(&parse_result.program);

    let mut output = Vec::new();
    let mut type_only_ids = Vec::new();

    for stmt in &parse_result.program.body {
      if let Some(rewritten) =
        ImportExportRewriter::rewrite_statement(stmt, code, &mut type_only_ids)
      {
        if !matches!(
          stmt,
          Statement::TSInterfaceDeclaration(_)
            | Statement::TSTypeAliasDeclaration(_)
            | Statement::TSEnumDeclaration(_)
            | Statement::FunctionDeclaration(_)
            | Statement::ClassDeclaration(_)
            | Statement::VariableDeclaration(_)
            | Statement::TSNamespaceDeclaration(_)
            | Statement::TSExternalModuleDeclaration(_)
            | Statement::TSGlobalDeclaration(_)
        ) {
          output.push(rewritten);
        }
      }
    }

    let mut seen_inline_imports = FxHashSet::default();

    for decl_node in collector.declarations {
      let transformed = Self::transform_declaration_node(&decl_node, code, id, state);
      let mut deduped_lines = Vec::new();
      for line in transformed.lines() {
        if (line.starts_with("import {") || line.starts_with("import * as"))
          && line.contains("from \"")
        {
          if seen_inline_imports.contains(line) {
            continue;
          }
          seen_inline_imports.insert(line.to_string());
        }
        deduped_lines.push(line);
      }
      output.push(deduped_lines.join("\n"));
    }

    if self.options.side_effects {
      output.push("sideEffect();".to_string());
    }

    state.type_only_map.insert(id.to_string(), type_only_ids);

    let transformed_code = output.join("\n");

    if self.options.sourcemap {
      let codegen_allocator = Allocator::default();
      let codegen_parser = TypeScriptParser::new(&codegen_allocator);
      let codegen_source = transformed_code.clone();
      let codegen_parse = codegen_parser.parse(&codegen_source, id)?;
      let codegen_result = Codegen::new()
        .with_options(CodegenOptions {
          source_map_path: Some(id.into()),
          ..CodegenOptions::default()
        })
        .with_source_text(&codegen_source)
        .build(&codegen_parse.program);

      return Ok(TransformResult {
        code: transformed_code,
        map: codegen_result.map.map(|m| m.into_owned().to_json_string()),
      });
    }

    Ok(TransformResult { code: transformed_code, map: None })
  }

  fn transform_declaration_node(
    decl_node: &DeclarationNode,
    source: &str,
    id: &str,
    state: &mut PluginState,
  ) -> String {
    let bindings = if decl_node.is_side_effect {
      let idx = state.get_identifier_index("");
      vec![format!("_{idx}")]
    } else {
      decl_node.bindings.clone()
    };

    if bindings.is_empty() {
      return String::new();
    }

    #[expect(clippy::cast_possible_truncation)]
    let span_start = find_leading_comment_start(source, decl_node.span.start as usize) as u32;
    let decl_source = codegen::extract_source_text(source, span_start, decl_node.span.end);

    let (rewritten_source, inline_imports) = InlineImportCollector::rewrite_source(&decl_source);

    {
      let all_imports = InlineImportCollector::collect(&decl_source);
      for info in &all_imports {
        if !info.is_local {
          state
            .external_inline_imports
            .entry(id.to_string())
            .or_default()
            .insert(info.source.clone());
        }
      }
    }

    let mut type_param_collector = TypeParamCollector::new();

    let decl_allocator = Allocator::default();
    let decl_parser = TypeScriptParser::new(&decl_allocator);
    if let Ok(decl_parse) = decl_parser.parse(&rewritten_source, "fragment.d.ts") {
      for stmt in &decl_parse.program.body {
        type_param_collector.visit_statement(stmt);
      }
    }

    let type_params = type_param_collector.into_params();

    // Only type parameters are excluded from deps (scoped bindings). Declaration
    // names stay eligible so same-name type references work (`const Stuff: Stuff`).
    let mut dep_bindings = Vec::new();
    for param in &type_params {
      dep_bindings.push(param.name.clone());
    }

    let mut dep_collector = DependencyCollector::new(dep_bindings);
    if let Ok(decl_parse) = decl_parser.parse(&rewritten_source, "fragment.d.ts") {
      for stmt in &decl_parse.program.body {
        dep_collector.visit_statement(stmt);
      }
    }

    let deps = dep_collector.deps.clone();
    let dep_refs = dep_collector.dep_refs;
    let children = dep_collector.children;
    let child_literals: Vec<String> = children
      .iter()
      .filter_map(|(start, end)| {
        let start = *start as usize;
        let end = *end as usize;
        if end > rewritten_source.len() || start >= end {
          return None;
        }
        let text = &rewritten_source[start..end];
        if text.starts_with('\'') || text.starts_with('"') { Some(text.to_string()) } else { None }
      })
      .collect();

    let decl_info = DeclarationInfo {
      id: 0,
      bindings: bindings.clone(), // synthetic `_N` for side-effect declarations
      type_params,
      deps: deps.clone(),
      dep_refs,
      children: children.clone(),
      child_literals,
      source: rewritten_source,
      is_side_effect: decl_node.is_side_effect,
      module_id: id.to_string(),
    };

    let decl_id = state.register_declaration(decl_info);

    let type_param_names: Vec<String> =
      state.get_declaration(decl_id).unwrap().type_params.iter().map(|p| p.name.clone()).collect();

    let runtime_binding = codegen::RuntimeBindingGenerator::generate_runtime_binding(
      &bindings,
      decl_id,
      &deps,
      &type_param_names,
      &children,
      decl_node.is_side_effect,
    );

    if decl_node.is_export {
      if decl_node.is_default {
        let export_line = format!("export {{ {} as default }}", bindings[0]);
        let mut parts = inline_imports;
        parts.push(runtime_binding);
        parts.push(export_line);
        parts.join("\n")
      } else {
        let mut parts = inline_imports;
        parts.push(format!("export {runtime_binding}"));
        parts.join("\n")
      }
    } else {
      let mut parts = inline_imports;
      parts.push(runtime_binding);
      parts.join("\n")
    }
  }

  #[expect(clippy::too_many_lines)]
  pub fn render_chunk(&self, code: &str, chunk: &ChunkInfo) -> Result<TransformResult> {
    if !filename::is_dts(&chunk.filename) {
      return Ok(TransformResult { code: code.to_string(), map: None });
    }

    let state = self.state.lock().unwrap();

    let allocator = Allocator::default();
    let parser = TypeScriptParser::new(&allocator);
    let parse_result = parser.parse(code, &chunk.filename)?;

    let mut all_type_only_ids = FxHashSet::default();
    for module_id in &chunk.module_ids {
      if let Some(type_ids) = state.type_only_map.get(module_id) {
        for id in type_ids {
          all_type_only_ids.insert(id.clone());
        }
      }
    }

    let mut export_mappings = FxHashMap::default();
    for stmt in &parse_result.program.body {
      HelperTransformer::collect_export_mappings_public(stmt, &mut export_mappings);
    }

    // When the bundle starts with a rolldown:runtime region, emit a closing //#endregion
    // before the first user module region matching Babel's renderChunk behavior.
    let mut needs_runtime_close = code.trim_start().starts_with("//#region rolldown:runtime");

    let mut output_parts: Vec<String> = Vec::new();
    let mut last_end: usize = 0;
    let mut last_was_import = false;
    let mut saw_kept_output = false;
    let mut pending_endregions: Vec<String> = Vec::new();
    // Track whether we're inside a rolldown:runtime region so we can skip its closing //#endregion
    let mut in_runtime_region = false;
    // Suppressed //#region for namespace decls; re-inserted if the namespace is later resolved.
    let mut suppressed_ns_regions: FxHashMap<String, String> = FxHashMap::default();

    let mut source_map_segments: Vec<(SourceMap<'static>, u32)> = Vec::new();

    for stmt in &parse_result.program.body {
      let stmt_start = stmt.span().start as usize;
      let stmt_end = stmt.span().end as usize;

      let result = self.process_render_chunk_statement(
        stmt,
        code,
        &state,
        &all_type_only_ids,
        &export_mappings,
        self.options.sourcemap,
      );

      let (is_kept, is_import) = match &result {
        RenderAction::Keep { text, .. } => {
          (!text.trim().is_empty(), matches!(stmt, Statement::ImportDeclaration(_)))
        }
        RenderAction::Remove => (false, false),
      };

      let (mut gap_regions, mut gap_endregions) = (Vec::new(), Vec::new());
      if stmt_start > last_end {
        let between = &code[last_end..stmt_start];
        for line in between.lines() {
          let lt = line.trim();
          if lt.contains("rolldown:runtime") {
            if lt.starts_with("//#region") {
              in_runtime_region = true;
            }
            continue;
          }
          if in_runtime_region && lt.starts_with("//#endregion") {
            in_runtime_region = false;
            continue;
          }
          if lt.starts_with("//#region") {
            gap_regions.push(line.to_string());
          } else if lt.starts_with("//#endregion") {
            gap_endregions.push(line.to_string());
          }
        }
      }

      // Namespace decls suppress their //#region (may re-insert if resolved later).
      let is_export_all_namespace = matches!(&result, RenderAction::Keep { text, .. } if {
        let trimmed = text.trim_start();
        trimmed.starts_with("declare namespace ") && trimmed.contains("_exports")
      });
      let mut suppressed_region: Option<String> = None;
      if is_export_all_namespace {
        suppressed_region = gap_regions.first().cloned();
        gap_regions.clear();
        // Drop preceding //#endregion (kept namespaces merge into the previous module region).
        gap_endregions.clear();
      }

      let has_gap_region_comments = !gap_regions.is_empty() || !gap_endregions.is_empty();

      // Blank line after imports only when a region/declaration gap sits between them.
      if is_kept && last_was_import && !is_import && has_gap_region_comments {
        Self::push_output_part(&mut output_parts, String::new());
      }

      // Defer internal //#endregion until a module boundary or end of output.
      if !gap_regions.is_empty() {
        if needs_runtime_close && gap_endregions.is_empty() {
          Self::push_output_part(&mut output_parts, "//#endregion".to_string());
          needs_runtime_close = false;
        }
        if gap_endregions.is_empty() {
          for comment in pending_endregions.drain(..) {
            Self::push_output_part(&mut output_parts, comment);
          }
        } else {
          // Real module boundary already has a closer (drop deferred ones).
          pending_endregions.clear();
        }
        for comment in gap_endregions {
          Self::push_output_part(&mut output_parts, comment);
        }
        for comment in gap_regions {
          Self::push_output_part(&mut output_parts, comment);
        }
      } else if !gap_endregions.is_empty() {
        if !saw_kept_output || !is_kept {
          for comment in gap_endregions {
            Self::push_output_part(&mut output_parts, comment);
          }
        } else {
          pending_endregions.extend(gap_endregions);
        }
      }

      // Flush deferred endregions before entry re-exports that live outside module regions.
      let is_chunk_export = matches!(
        stmt,
        Statement::ExportNamedDeclaration(_)
          | Statement::ExportFromDeclaration(_)
          | Statement::ExportAllDeclaration(_)
      );
      let is_cjs_export_equals = matches!(
        &result,
        RenderAction::Keep { text, .. } if text.trim_start().starts_with("export =")
      );
      if is_kept && is_chunk_export && !pending_endregions.is_empty() && !is_cjs_export_equals {
        for comment in pending_endregions.drain(..) {
          Self::push_output_part(&mut output_parts, comment);
        }
      }

      last_end = stmt_end;

      match result {
        RenderAction::Keep { text, map } => {
          if !text.trim().is_empty() {
            if let Some(map) = map {
              source_map_segments.push((map, Self::output_parts_line_count(&output_parts)));
            }
            if let Some(region) = suppressed_region {
              let trimmed_text = text.trim_start();
              if let Some(after_prefix) = trimmed_text.strip_prefix("declare namespace ") {
                if let Some(name_end) =
                  after_prefix.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                {
                  let ns_name = &after_prefix[..name_end];
                  suppressed_ns_regions.insert(ns_name.to_string(), region);
                }
              }
            }
            Self::push_output_part(&mut output_parts, text);
            saw_kept_output = true;
            last_was_import = is_import;
          }
        }
        RenderAction::Remove => {}
      }
    }

    if last_end < code.len() {
      let trailing = &code[last_end..];
      let mut trailing_regions = Vec::new();
      let mut trailing_endregions = Vec::new();
      for line in trailing.lines() {
        let lt = line.trim();
        if lt.contains("rolldown:runtime") {
          if lt.starts_with("//#region") {
            in_runtime_region = true;
          }
          continue;
        }
        if in_runtime_region && lt.starts_with("//#endregion") {
          in_runtime_region = false;
          continue;
        }
        if lt.starts_with("//#region") {
          trailing_regions.push(line.to_string());
        } else if lt.starts_with("//#endregion") {
          trailing_endregions.push(line.to_string());
        }
      }
      if trailing_regions.is_empty() {
        for comment in pending_endregions.drain(..) {
          Self::push_output_part(&mut output_parts, comment);
        }
        for comment in trailing_endregions {
          Self::push_output_part(&mut output_parts, comment);
        }
      } else {
        for comment in pending_endregions.drain(..) {
          Self::push_output_part(&mut output_parts, comment);
        }
        for comment in trailing_endregions {
          Self::push_output_part(&mut output_parts, comment);
        }
        for comment in trailing_regions {
          Self::push_output_part(&mut output_parts, comment);
        }
      }
    } else {
      for comment in pending_endregions.drain(..) {
        Self::push_output_part(&mut output_parts, comment);
      }
    }

    if output_parts.is_empty() {
      return Ok(TransformResult { code: "export { };".to_string(), map: None });
    }

    let has_content = output_parts.iter().any(|p| {
      let t = p.trim();
      !t.is_empty() && !t.starts_with("//#region") && !t.starts_with("//#endregion")
    });

    if !has_content {
      return Ok(TransformResult { code: "export { };".to_string(), map: None });
    }

    // Rolldown turns `import * as ns from "./mod"` into synthetic
    // `declare namespace mod_d_exports { export { Foo }; }` + `mod_d_exports.Foo` uses.
    // Flatten member access; keep the namespace only when used standalone.
    let ns_decl_re = regex::Regex::new(
      r"^declare namespace (\w+_exports)\s*\{\s*\n?\s*export \{([^}]*)\};\s*\n?\s*\}$",
    )
    .unwrap();

    let mut ns_member_map: FxHashMap<String, Vec<(String, String)>> = FxHashMap::default();
    for part in &output_parts {
      let trimmed = part.trim();
      if let Some(caps) = ns_decl_re.captures(trimmed) {
        let ns_name = caps[1].to_string();
        let members_str = &caps[2];
        let members: Vec<(String, String)> = members_str
          .split(',')
          .filter_map(|m| {
            let m = m.trim();
            if m.is_empty() {
              return None;
            }
            if let Some((local, exported)) = m.split_once(" as ") {
              Some((exported.trim().to_string(), local.trim().to_string()))
            } else {
              Some((m.to_string(), m.to_string()))
            }
          })
          .collect();
        if !members.is_empty() {
          ns_member_map.insert(ns_name, members);
        }
      }
    }

    // If a namespace name appears as a standalone identifier (e.g., in `export { foo_d_exports as fooNs }`),
    // we must keep it. Only resolve namespaces used exclusively as `ns.member`.
    if !ns_member_map.is_empty() {
      let mut standalone_ns: FxHashSet<String> = FxHashSet::default();
      for part in &output_parts {
        let trimmed = part.trim();
        if ns_decl_re.is_match(trimmed) {
          continue;
        }
        for ns_name in ns_member_map.keys() {
          if !part.contains(ns_name) {
            continue;
          }
          // Manual scan instead of regex look-ahead (unsupported by regex crate)
          let has_standalone = {
            let bytes = part.as_bytes();
            let name_len = ns_name.len();
            let mut found = false;
            let mut start = 0;
            while let Some(pos) = part[start..].find(ns_name) {
              let abs = start + pos;
              let end = abs + name_len;
              let before_ok = abs == 0 || {
                let c = bytes[abs - 1];
                !c.is_ascii_alphanumeric() && c != b'_'
              };
              let after_ok = end >= bytes.len() || {
                let c = bytes[end];
                !c.is_ascii_alphanumeric() && c != b'_'
              };
              let not_dot = end >= bytes.len() || bytes[end] != b'.';
              if before_ok && after_ok && not_dot {
                found = true;
                break;
              }
              start = abs + 1;
            }
            found
          };
          if has_standalone {
            standalone_ns.insert(ns_name.clone());
          }
        }
      }

      // Always flatten `ns.Member` → `Member`. Keep the namespace decl only if used standalone
      // (e.g. `export { ns_exports as ns }`).
      let removable: FxHashSet<String> =
        ns_member_map.keys().filter(|ns_name| !standalone_ns.contains(*ns_name)).cloned().collect();

      if !ns_member_map.is_empty() {
        // Re-insert suppressed //#region for resolved namespaces (remaining same-module content).
        let mut regions_to_insert: Vec<(usize, String)> = Vec::new();
        for (i, part) in output_parts.iter().enumerate() {
          let trimmed = part.trim();
          if let Some(caps) = ns_decl_re.captures(trimmed) {
            let ns_name = caps[1].to_string();
            if removable.contains(&ns_name) {
              if let Some(region) = suppressed_ns_regions.get(&ns_name) {
                regions_to_insert.push((i, region.clone()));
              }
            }
          }
        }
        // Reverse insert to preserve indices. Close the previous module first when needed
        // (avoid a leading orphan //#endregion).
        for (i, region) in regions_to_insert.into_iter().rev() {
          let needs_preceding_endregion = output_parts[..i].iter().any(|part| {
            let t = part.trim();
            !t.is_empty()
              && !t.starts_with("import ")
              && !t.starts_with("//#region")
              && !t.starts_with("//#endregion")
          });
          output_parts.insert(i, region);
          if needs_preceding_endregion {
            output_parts.insert(i, "//#endregion".to_string());
          }
        }

        output_parts.retain(|part| {
          let trimmed = part.trim();
          if let Some(caps) = ns_decl_re.captures(trimmed) {
            let ns_name = caps[1].to_string();
            !removable.contains(&ns_name)
          } else {
            true
          }
        });

        for part in &mut output_parts {
          for (ns_name, members) in &ns_member_map {
            for (exported, local) in members {
              let qualified = format!("{ns_name}.{exported}");
              if part.contains(&qualified) {
                *part = part.replace(&qualified, local);
              }
            }
          }
        }
      }
    }

    let mut reference_comments: Vec<String> = Vec::new();
    let mut seen_comments = FxHashSet::default();
    for module_id in &chunk.module_ids {
      if let Some(comments) = state.comments_map.get(module_id) {
        for comment in comments {
          if !seen_comments.contains(comment) {
            seen_comments.insert(comment.clone());
            reference_comments.push(comment.clone());
          }
        }
      }
    }

    let mut allowed_external_imports = FxHashSet::default();
    for module_id in &chunk.module_ids {
      if let Some(sources) = state.external_inline_imports.get(module_id) {
        allowed_external_imports.extend(sources.iter().cloned());
      }
    }

    let mut joined = output_parts.join("\n");
    let mut external_imports: Vec<String> = Vec::new();
    if !allowed_external_imports.is_empty() {
      let (rewritten, ext_imports) =
        InlineImportCollector::rewrite_external_imports(&joined, &allowed_external_imports);
      if !ext_imports.is_empty() {
        joined = rewritten;
        external_imports = ext_imports;
        output_parts = joined.split('\n').map(std::string::ToString::to_string).collect();
      }
    }

    let mut result = String::new();

    for comment in &reference_comments {
      result.push_str(comment);
      result.push('\n');
    }

    if !external_imports.is_empty() {
      for import in &external_imports {
        result.push_str(import);
        result.push('\n');
      }
      result.push('\n');
    }

    result.push_str(&output_parts.join("\n"));

    result = Self::prune_unused_imports(&result);

    result = Self::cleanup_region_markers(&result);
    result = Self::restore_runtime_import_regions(&result);
    result = Self::ensure_blank_line_after_imports(&result);
    result = Self::normalize_whitespace(&result);

    if self.options.sourcemap {
      let prefix_lines = Self::count_prefix_lines(&reference_comments, &external_imports);
      let map =
        Self::compose_render_source_map(&source_map_segments, &chunk.filename, prefix_lines);
      return Ok(TransformResult { code: result, map });
    }

    Ok(TransformResult { code: result, map: None })
  }

  fn prune_unused_imports(source: &str) -> String {
    let allocator = Allocator::default();
    let parser = TypeScriptParser::new(&allocator);
    let Ok(parse_result) = parser.parse(source, "chunk.d.ts") else {
      return source.to_string();
    };

    let mut non_import_parts = Vec::new();
    for stmt in &parse_result.program.body {
      if !matches!(stmt, Statement::ImportDeclaration(_)) {
        non_import_parts.push(codegen::extract_source_text(
          source,
          stmt.span().start,
          stmt.span().end,
        ));
      }
    }
    let usage_text = non_import_parts.join("\n");

    let mut remove_ranges: Vec<(usize, usize)> = Vec::new();
    for stmt in &parse_result.program.body {
      if let Statement::ImportDeclaration(import) = stmt {
        if !Self::is_import_used(import, &usage_text) {
          let start = stmt.span().start as usize;
          let mut end = stmt.span().end as usize;
          if end < source.len() && source.as_bytes()[end] == b'\n' {
            end += 1;
          }
          remove_ranges.push((start, end));
        }
      }
    }

    if remove_ranges.is_empty() {
      return source.to_string();
    }

    remove_ranges.sort_by_key(|(start, _)| std::cmp::Reverse(*start));
    let mut result = source.to_string();
    for (start, end) in remove_ranges {
      if start <= result.len() && end <= result.len() && start < end {
        result.replace_range(start..end, "");
      }
    }

    result
  }

  fn is_import_used(import: &ImportDeclaration, usage_text: &str) -> bool {
    let Some(specifiers) = &import.specifiers else {
      return true;
    };

    for spec in specifiers {
      let local_name = match spec {
        ImportDeclarationSpecifier::ImportSpecifier(s) => s.local.name.as_str(),
        ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => s.local.name.as_str(),
        ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => s.local.name.as_str(),
      };

      if Self::is_identifier_used(local_name, usage_text) {
        return true;
      }
    }

    false
  }

  fn is_identifier_used(name: &str, text: &str) -> bool {
    let re = regex::Regex::new(&format!(r"\b{}\b", regex::escape(name))).unwrap();
    re.is_match(text)
  }

  fn push_output_part(parts: &mut Vec<String>, part: String) {
    parts.push(part);
  }

  fn output_parts_line_count(parts: &[String]) -> u32 {
    if parts.is_empty() {
      return 0;
    }
    #[expect(clippy::cast_possible_truncation)]
    {
      parts.join("\n").matches('\n').count() as u32 + 1
    }
  }

  fn count_prefix_lines(reference_comments: &[String], external_imports: &[String]) -> u32 {
    #[expect(clippy::cast_possible_truncation)]
    {
      let mut lines = 0u32;
      for comment in reference_comments {
        lines += comment.matches('\n').count() as u32 + 1;
      }
      for import in external_imports {
        lines += import.matches('\n').count() as u32 + 1;
      }
      if !external_imports.is_empty() {
        lines += 1;
      }
      lines
    }
  }

  fn process_render_chunk_statement(
    &self,
    stmt: &Statement,
    code: &str,
    state: &PluginState,
    type_only_ids: &FxHashSet<String>,
    export_mappings: &FxHashMap<String, String>,
    sourcemap: bool,
  ) -> RenderAction {
    match stmt {
      Statement::ImportDeclaration(import) => {
        if HelperTransformer::is_helper_import_public(import) {
          return RenderAction::Remove;
        }
        let import_text = codegen::extract_source_text(code, stmt.span().start, stmt.span().end);
        RenderAction::keep(Self::patch_import_source(&import_text))
      }

      // Bundler-only keep-alives (`sideEffect();`, etc.) (not part of the restored dts).
      Statement::ExpressionStatement(_) => RenderAction::Remove,

      Statement::ExportDeclaration(export) => {
        if let Declaration::VariableDeclaration(var_decl) = &export.declaration {
          if let Some(runtime_info) = Self::extract_runtime_binding_info(var_decl, code) {
            if let Some(decl_info) = state.get_declaration(runtime_info.declaration_id) {
              let (decl, map) = Self::generate_declaration_with_renames(
                decl_info,
                &runtime_info.binding_names,
                &runtime_info.renamed_params,
                &runtime_info.renamed_deps,
                sourcemap,
              );
              let replacement =
                if decl.trim().starts_with("export ") { decl } else { format!("export {decl}") };
              return RenderAction::keep_mapped(replacement, map);
            }
          }
        }

        let text = codegen::extract_source_text(code, stmt.span().start, stmt.span().end);
        RenderAction::keep(text)
      }

      Statement::ExportNamedDeclaration(export) => {
        if export.specifiers.is_empty() {
          return RenderAction::Remove;
        }

        self.process_export_specifiers(export, code, type_only_ids, export_mappings)
      }

      Statement::ExportFromDeclaration(_export) => {
        let text = codegen::extract_source_text(code, stmt.span().start, stmt.span().end);
        RenderAction::keep(Self::patch_import_source(&text))
      }

      Statement::ExportAllDeclaration(export_all) => {
        let source_value = &export_all.source.value;
        let patched_source = filename::patch_dts_extension(source_value);
        if let Some(exported) = export_all.exported.as_ref() {
          let exported_name = exported.name().to_string();
          RenderAction::keep(format!("export * as {exported_name} from \"{patched_source}\";"))
        } else {
          RenderAction::keep(format!("export * from \"{patched_source}\";"))
        }
      }

      Statement::VariableDeclaration(var_decl) => {
        if let Some(runtime_info) = Self::extract_runtime_binding_info(var_decl, code) {
          if let Some(decl_info) = state.get_declaration(runtime_info.declaration_id) {
            let (decl, map) = Self::generate_declaration_with_renames(
              decl_info,
              &runtime_info.binding_names,
              &runtime_info.renamed_params,
              &runtime_info.renamed_deps,
              sourcemap,
            );
            return RenderAction::keep_mapped(decl, map);
          }
        }

        if let Some(transformed) = HelperTransformer::transform_export_all_public(var_decl, code) {
          return RenderAction::keep(transformed);
        }

        if let Some(transformed) =
          HelperTransformer::transform_member_access_public(var_decl, export_mappings)
        {
          return RenderAction::keep(transformed);
        }

        // Drop non-runtime-binding vars, including secondary declarators Rolldown may split
        // out of multi-binding runtime vars (`var a = [...], b` → `var a = [...]; var b`).
        RenderAction::Remove
      }

      _ => {
        let text = codegen::extract_source_text(code, stmt.span().start, stmt.span().end);
        RenderAction::keep(text)
      }
    }
  }

  fn process_export_specifiers(
    &self,
    export: &ExportNamedDeclaration,
    _code: &str,
    type_only_ids: &FxHashSet<String>,
    export_mappings: &FxHashMap<String, String>,
  ) -> RenderAction {
    if export.specifiers.len() == 1 {
      if let Some(spec) = export.specifiers.first() {
        let local_name = spec.local.name().to_string();
        if let Some(mapped_name) = export_mappings.get(&local_name) {
          let exported_name = spec.exported.name().to_string();

          // CJS default: export { x as default } → export = x
          if self.options.cjs_default && exported_name == "default" {
            return RenderAction::keep(format!("export = {mapped_name};"));
          }

          return RenderAction::keep(format!("export {{ {mapped_name} as {exported_name} }};"));
        }
      }
    }

    let mut new_specifiers = Vec::new();
    for spec in &export.specifiers {
      let local_name = spec.local.name().to_string();
      // String-literal exports need quotes in the restored specifier text.
      let exported_name = match &spec.exported {
        ModuleExportName::StringLiteral(lit) => serde_json::to_string(lit.value.as_str())
          .unwrap_or_else(|_| format!("\"{}\"", lit.value.escape_default())),
        other => other.name().to_string(),
      };
      let exported_is_string = matches!(&spec.exported, ModuleExportName::StringLiteral(_));

      let is_type_only = type_only_ids.contains(spec.exported.name().as_str());

      if is_type_only {
        if !exported_is_string && local_name == exported_name {
          new_specifiers.push(format!("type {local_name}"));
        } else {
          new_specifiers.push(format!("type {local_name} as {exported_name}"));
        }
      } else if !exported_is_string && local_name == exported_name {
        new_specifiers.push(local_name);
      } else {
        new_specifiers.push(format!("{local_name} as {exported_name}"));
      }
    }

    if self.options.cjs_default && export.specifiers.len() == 1 {
      if let Some(spec) = export.specifiers.first() {
        let exported_name = spec.exported.name().to_string();
        if exported_name == "default" {
          let local_name = spec.local.name().to_string();
          return RenderAction::keep(format!("export = {local_name};"));
        }
      }
    }

    RenderAction::keep(format!("export {{ {} }};", new_specifiers.join(", ")))
  }

  /// Runtime binding format: `var X = [id, (params...) => [deps...], [children...]]`
  /// where children are a flat `[start,end,…]` span list.
  /// After bundling, Rolldown may have renamed params/deps (extract those for restore).
  fn extract_runtime_binding_info(
    var_decl: &VariableDeclaration,
    code: &str,
  ) -> Option<RuntimeBindingInfo> {
    if var_decl.declarations.is_empty() {
      return None;
    }

    let declarator = &var_decl.declarations[0];

    let mut binding_names = Vec::new();
    for decl in &var_decl.declarations {
      match &decl.id {
        BindingPattern::BindingIdentifier(id) => {
          binding_names.push(id.name.to_string());
        }
        _ => return None,
      }
    }
    if binding_names.is_empty() {
      return None;
    }

    let init = declarator.init.as_ref()?;

    let Expression::ArrayExpression(arr) = init else {
      return None;
    };

    if arr.elements.len() < 3 {
      return None;
    }

    let declaration_id = match arr.elements.first() {
      Some(ArrayExpressionElement::NumericLiteral(num)) => {
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
          num.value as usize
        }
      }
      _ => return None,
    };

    let mut renamed_params = Vec::new();
    let mut renamed_deps = Vec::new();

    if let Some(ArrayExpressionElement::ArrowFunctionExpression(arrow)) = arr.elements.get(1) {
      for param in &arrow.params.items {
        if let BindingPattern::BindingIdentifier(id) = &param.pattern {
          renamed_params.push(id.name.to_string());
        }
      }

      if let Some(Expression::ArrayExpression(deps_arr)) = arrow.get_expression() {
        for elem in &deps_arr.elements {
          let dep_text = match elem {
            ArrayExpressionElement::Identifier(id) => id.name.to_string(),
            other => {
              let span = other.span();
              codegen::extract_source_text(code, span.start, span.end)
            }
          };
          renamed_deps.push(dep_text);
        }
      }
    }

    Some(RuntimeBindingInfo { declaration_id, binding_names, renamed_params, renamed_deps })
  }

  fn generate_declaration_with_renames(
    decl_info: &DeclarationInfo,
    binding_names: &[String],
    renamed_params: &[String],
    renamed_deps: &[String],
    sourcemap: bool,
  ) -> (String, Option<SourceMap<'static>>) {
    // Keep comments out of rename passes so identifiers inside them are not rewritten.
    let (leading_comments, decl_only) = Self::split_leading_comments(&decl_info.source);

    let mut source = decl_only;

    let mut renames: Vec<(String, String)> = Vec::new();

    // Dep renames first at original spans, applied reverse-order so earlier edits don't shift later ones.
    let mut dep_span_replacements: Vec<(u32, u32, String)> = Vec::new();
    for (i, original_dep) in decl_info.deps.iter().enumerate() {
      if let Some(renamed) = renamed_deps.get(i) {
        // Rolldown may rewrite free `undefined` to `void 0` in the deps array; treat as no-op.
        let renamed = if renamed == "void 0" { "undefined" } else { renamed.as_str() };
        if original_dep != renamed {
          renames.push((original_dep.clone(), renamed.to_string()));
          for (name, start, end) in &decl_info.dep_refs {
            if name == original_dep {
              dep_span_replacements.push((*start, *end, renamed.to_string()));
            }
          }
        }
      }
    }
    dep_span_replacements.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    for (start, end, renamed) in dep_span_replacements {
      let start = start as usize;
      let end = end as usize;
      if start < end && end <= source.len() {
        source.replace_range(start..end, &renamed);
      }
    }

    // Binding renames after deps (spans stay valid). Skip dep_ref spans and synthetic `_N`
    // side-effect bindings (`declare global` / `declare module`).
    let primary_binding = binding_names.first().map(String::as_str).unwrap_or("");
    if !decl_info.is_side_effect {
      let skip: Vec<(u32, u32)> = decl_info.dep_refs.iter().map(|(_, s, e)| (*s, *e)).collect();
      for (i, original_binding) in decl_info.bindings.iter().enumerate() {
        if let Some(renamed_binding) = binding_names.get(i) {
          if original_binding != renamed_binding {
            renames.push((original_binding.clone(), renamed_binding.clone()));
            source =
              replace_identifier_except_spans(&source, original_binding, renamed_binding, &skip);
          }
        }
      }
    }

    for (i, original_param) in decl_info.type_params.iter().enumerate() {
      if let Some(renamed) = renamed_params.get(i) {
        if original_param.name != *renamed {
          renames.push((original_param.name.clone(), renamed.clone()));
          source = replace_identifier(&source, &original_param.name, renamed);
        }
      }
    }

    if !renames.is_empty() {
      source = fix_namespace_export_specifiers(&source, &renames);
    }

    let source = Self::insert_unnamed_function_name(&source, primary_binding);

    let (generated, map) = if sourcemap {
      let source_path = Self::module_id_to_ts_source(&decl_info.module_id);
      codegen::generate_declaration_with_source_map(&source, &source_path)
    } else {
      (codegen::generate_declaration_from_source(&source), None)
    };
    let generated = generated.replace("void 0", "undefined");
    let generated = Self::restore_child_literal_quotes(&decl_info.child_literals, &generated);

    let result = if leading_comments.is_empty() {
      generated
    } else {
      format!("{}\n{}", leading_comments.trim_end(), generated.trim_start())
    };
    (result, map)
  }

  // oxc_codegen may flip `'lit'` ↔ `"lit"`; restore using transform-time child_literals.
  fn restore_child_literal_quotes(child_literals: &[String], generated: &str) -> String {
    let mut result = generated.to_string();
    for original in child_literals {
      if original.len() < 2 {
        continue;
      }
      let quote = original.chars().next().filter(|c| *c == '\'' || *c == '"');
      let Some(quote) = quote else { continue };
      if !original.ends_with(quote) {
        continue;
      }
      let inner = &original[1..original.len() - 1];
      let alternate = if quote == '\'' { format!("\"{inner}\"") } else { format!("'{inner}'") };
      if result.contains(&alternate) {
        result = result.replace(&alternate, original);
      }
    }
    result
  }

  fn split_leading_comments(source: &str) -> (String, String) {
    let mut comment_end = 0;
    let mut in_block_comment = false;

    for (i, line) in source.lines().enumerate() {
      let trimmed = line.trim();

      if in_block_comment {
        comment_end += line.len() + 1; // +1 for newline
        if trimmed.contains("*/") {
          in_block_comment = false;
        }
        continue;
      }

      if trimmed.starts_with("/**") || trimmed.starts_with("/*") {
        in_block_comment = true;
        comment_end += line.len() + 1;
        if trimmed.contains("*/") {
          in_block_comment = false;
        }
        continue;
      }

      if trimmed.starts_with("//") {
        comment_end += line.len() + 1;
        continue;
      }

      if trimmed.is_empty() && i == 0 {
        comment_end += line.len() + 1;
        continue;
      }

      break;
    }

    if comment_end == 0 || comment_end > source.len() {
      return (String::new(), source.to_string());
    }

    let comments = source[..comment_end].trim_end().to_string();
    let rest = source[comment_end..].to_string();

    if comments.is_empty() { (String::new(), rest) } else { (comments, rest) }
  }

  /// If the source is an unnamed function declaration (e.g., `function <T>(...)` or `function(...)`),
  /// insert the binding name after the `function` keyword.
  /// Also handles `export default function <T>(...)` forms.
  fn insert_unnamed_function_name(source: &str, binding_name: &str) -> String {
    let trimmed = source.trim_start();

    if let Some(rest) = trimmed.strip_prefix("export default function") {
      let rest_trimmed = rest.trim_start();
      if rest_trimmed.starts_with('<') || rest_trimmed.starts_with('(') || rest.is_empty() {
        let prefix_len = source.len() - trimmed.len();
        let prefix = &source[..prefix_len];
        return format!("{prefix}export default function {binding_name} {rest_trimmed}");
      }
    }

    if let Some(rest) = trimmed.strip_prefix("function") {
      let rest_trimmed = rest.trim_start();
      if rest_trimmed.starts_with('<') || rest_trimmed.starts_with('(') {
        let prefix_len = source.len() - trimmed.len();
        let prefix = &source[..prefix_len];
        return format!("{prefix}function {binding_name} {rest_trimmed}");
      }
    }

    source.to_string()
  }

  fn cleanup_region_markers(code: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut region_depth: usize = 0;

    let is_basename_module_region = |line: &str| {
      let trimmed = line.trim();
      if !trimmed.starts_with("//#region ") {
        return false;
      }
      let name = trimmed.trim_start_matches("//#region ").trim();
      name.ends_with(".d.ts") && !name.contains('/') && name != "rolldown:runtime"
    };

    let mut i = 0;
    while i < lines.len() {
      let line = lines[i];
      let trimmed = line.trim();

      // Drop dependency-module regions like `//#region a.d.ts` when they only
      // contain flattened import/type bindings (preserve regions with real declarations).
      if is_basename_module_region(line) {
        let region_start = i + 1;
        let mut region_end = region_start;
        while region_end < lines.len() && lines[region_end].trim() != "//#endregion" {
          region_end += 1;
        }
        let flatten = lines[region_start..region_end].iter().all(|inner| {
          let t = inner.trim();
          t.is_empty() || t.starts_with("import ") || t.starts_with("type ")
        });
        if flatten {
          i = region_end + 1;
          for inner in &lines[region_start..region_end] {
            kept.push(inner);
          }
          continue;
        }
      }

      if trimmed == "//#endregion" {
        if region_depth > 0 {
          region_depth -= 1;
          kept.push(line);
        } else {
          // Keep endregions that close a removed/suppressed region before a path-based
          // module region or the final chunk export list.
          let next_meaningful = lines[i + 1..].iter().find(|next| !next.trim().is_empty());
          let keep = next_meaningful.is_some_and(|next| {
            let t = next.trim();
            (t.starts_with("//#region ") && t.contains('/')) || t.starts_with("export ")
          });
          if keep {
            kept.push(line);
          }
        }
        i += 1;
        continue;
      }

      if trimmed.starts_with("//#region") {
        let mut j = i + 1;
        while j < lines.len() && lines[j].trim().is_empty() {
          j += 1;
        }
        if j < lines.len() && lines[j].trim() == "//#endregion" {
          // Keep the opening region marker but drop the closing marker for empty regions
          // (matches Babel fake-js: empty modules still emit `//#region path`).
          region_depth += 1;
          kept.push(line);
          i = j + 1;
          continue;
        }
        region_depth += 1;
      }

      kept.push(line);
      i += 1;
    }

    // Strip trailing //#endregion only when it follows the final export / block close
    // (cjs `export =`, declare module). Keep endregions that precede a final export list.
    while kept.last().is_some_and(|line| line.trim() == "//#endregion") {
      let prev = kept.iter().rev().nth(1).map(|line| line.trim()).unwrap_or("");
      if prev.starts_with("export ") || prev == "}" {
        kept.pop();
      } else {
        break;
      }
    }

    // Drop leading orphan endregions only when they precede an import (flattened dependency).
    while kept.first().is_some_and(|line| line.trim() == "//#endregion") {
      let mut next_idx = 1;
      while next_idx < kept.len() && kept[next_idx].trim().is_empty() {
        next_idx += 1;
      }
      let followed_by_import =
        kept.get(next_idx).is_some_and(|next| next.trim().starts_with("import "));
      if followed_by_import {
        kept.remove(0);
      } else {
        break;
      }
    }

    kept.join("\n")
  }

  /// Re-wrap imports that back `export { ns as alias }` re-exports after dependency
  /// module regions are flattened (matches Babel's `//#region rolldown:runtime` layout).
  fn restore_runtime_import_regions(code: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
      let line = lines[i];
      let trimmed = line.trim();

      if trimmed.starts_with("export * from ") {
        result.push(line.to_string());
        i += 1;

        while i < lines.len() && lines[i].trim().is_empty() {
          result.push(lines[i].to_string());
          i += 1;
        }

        if i < lines.len() && lines[i].trim().starts_with("import * as ") {
          let import_line = lines[i].trim();
          let import_name = import_line
            .strip_prefix("import * as ")
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or("");

          let mut j = i + 1;
          while j < lines.len() && lines[j].trim().is_empty() {
            j += 1;
          }

          if j < lines.len() {
            let export_line = lines[j].trim();
            if export_line.starts_with("export { ")
              && !import_name.is_empty()
              && export_line.contains(import_name)
              && export_line.contains(" as ")
            {
              if result.last().is_some_and(|prev| !prev.trim().is_empty()) {
                result.push(String::new());
              }
              result.push("//#region rolldown:runtime".to_string());
              result.push(String::new());
              result.push(lines[i].to_string());
              result.push("//#endregion".to_string());
              i = j;
              continue;
            }
          }
        }

        continue;
      }

      result.push(line.to_string());
      i += 1;
    }

    result.join("\n")
  }

  fn ensure_blank_line_after_imports(code: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    let mut result: Vec<String> = Vec::new();

    for (i, line) in lines.iter().enumerate() {
      result.push((*line).to_string());

      let trimmed = line.trim();
      if trimmed.starts_with("import ") || trimmed.starts_with("export * from ") {
        if lines.get(i + 1).is_some_and(|next| next.trim().starts_with("//#region")) {
          result.push(String::new());
        }
      }
    }

    result.join("\n")
  }

  /// Keep the module id for per-declaration maps. Final `.ts` vs `.d.ts` source
  /// paths come from Rolldown composition (generate-step maps) in generateBundle.
  fn module_id_to_ts_source(module_id: &str) -> String {
    module_id.to_string()
  }

  fn compose_render_source_map(
    segments: &[(SourceMap<'static>, u32)],
    chunk_filename: &str,
    line_offset: u32,
  ) -> Option<String> {
    if segments.is_empty() {
      return None;
    }

    compose_render_source_map_inner(segments, chunk_filename, line_offset)
  }

  fn normalize_whitespace(code: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    let mut result_lines: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
      let line = lines[i];
      if line.trim().starts_with("import ") {
        result_lines.push(line);
        i += 1;
        while i < lines.len() && lines[i].trim().is_empty() {
          let next_is_decl = lines.get(i + 1).is_some_and(|next| {
            let t = next.trim();
            t.starts_with("type ")
              || t.starts_with("export ")
              || t.starts_with("declare ")
              || t.starts_with("interface ")
          });
          if next_is_decl {
            i += 1;
          } else {
            result_lines.push(lines[i]);
            i += 1;
          }
        }
        continue;
      }
      result_lines.push(line);
      i += 1;
    }

    let mut result = result_lines.join("\n");

    let re = regex::Regex::new(r"\n\n\n+").unwrap();
    result = re.replace_all(&result, "\n\n").to_string();

    result.trim_start().trim_end().to_string()
  }

  fn patch_import_source(import_text: &str) -> String {
    let re_double = regex::Regex::new(r#""([^"]+)\.d\.(ts|mts|cts)""#).unwrap();
    let re_single = regex::Regex::new(r"'([^']+)\.d\.(ts|mts|cts)'").unwrap();

    let result = re_double.replace_all(import_text, |caps: &regex::Captures| {
      let path = &caps[1];
      let ext = &caps[2];
      let js_ext = match ext {
        "mts" => "mjs",
        "cts" => "cjs",
        _ => "js",
      };
      format!("\"{path}.{js_ext}\"")
    });

    let result = re_single.replace_all(&result, |caps: &regex::Captures| {
      let path = &caps[1];
      let ext = &caps[2];
      let js_ext = match ext {
        "mts" => "mjs",
        "cts" => "cjs",
        _ => "js",
      };
      format!("'{path}.{js_ext}'")
    });

    result.to_string()
  }
}

fn compose_render_source_map_inner(
  segments: &[(SourceMap<'static>, u32)],
  chunk_filename: &str,
  line_offset: u32,
) -> Option<String> {
  let refs: Vec<(&SourceMap<'_>, u32)> = segments
    .iter()
    .map(|(map, offset)| (map as &SourceMap<'_>, offset.saturating_add(line_offset)))
    .collect();
  let builder = ConcatSourceMapBuilder::from_sourcemaps(&refs);
  let map = builder.into_sourcemap();
  map.get_tokens().next()?;
  let mut map = map.into_owned();
  map = dedupe_source_map_sources_standalone(map);
  map.set_file(chunk_filename);
  map.set_source_contents(vec![]);
  Some(map.to_json_string())
}

fn dedupe_source_map_sources_standalone(map: SourceMap<'static>) -> SourceMap<'static> {
  let old_sources: Vec<String> = map.get_sources().map(str::to_owned).collect();
  let mut canonical_sources: Vec<String> = Vec::new();
  let mut old_to_new: Vec<u32> = Vec::with_capacity(old_sources.len());

  #[expect(clippy::cast_possible_truncation)]
  for source in &old_sources {
    if let Some(idx) = canonical_sources.iter().position(|s| s == source) {
      old_to_new.push(idx as u32);
    } else {
      canonical_sources.push(source.clone());
      old_to_new.push(canonical_sources.len() as u32 - 1);
    }
  }

  #[expect(clippy::cast_possible_truncation)]
  if old_to_new.iter().enumerate().all(|(i, &n)| n == i as u32) {
    return map;
  }

  let tokens: Vec<Token> = map
    .get_tokens()
    .map(|token| {
      let source_id = token.get_source_id().and_then(|id| old_to_new.get(id as usize).copied());
      Token::new(
        token.get_dst_line(),
        token.get_dst_col(),
        token.get_src_line(),
        token.get_src_col(),
        source_id,
        token.get_name_id(),
      )
    })
    .collect();

  SourceMap::new(
    map.get_file().map(|f| Cow::Owned(f.to_owned())),
    map.get_names().map(|n| Cow::Owned(n.to_owned())).collect(),
    map.get_source_root().map(|s| Cow::Owned(s.to_owned())),
    canonical_sources.into_iter().map(Cow::Owned).collect(),
    vec![],
    tokens.into_boxed_slice(),
    None,
  )
}

/// Include leading JSDoc/block/line comments when computing a declaration's source span.
fn find_leading_comment_start(source: &str, decl_start: usize) -> usize {
  let bytes = source.as_bytes();
  let mut pos = decl_start;

  loop {
    while pos > 0 && matches!(bytes[pos - 1], b' ' | b'\t' | b'\r') {
      pos -= 1;
    }

    if pos == 0 {
      break;
    }

    // Only skip newlines when they precede a comment (not a previous declaration).
    let mut scan = pos;
    while scan > 0 && bytes[scan - 1].is_ascii_whitespace() {
      scan -= 1;
    }

    if scan == 0 {
      break;
    }

    if scan >= 2 && bytes[scan - 2] == b'*' && bytes[scan - 1] == b'/' {
      pos = scan;
    } else if scan > 0 {
      let mut line_start = scan;
      while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
      }
      let line = &source[line_start..scan];
      let trimmed = line.trim();
      if trimmed.starts_with("//") && !trimmed.starts_with("/// <reference") {
        pos = line_start;
      } else {
        break;
      }
    } else {
      break;
    }

    if pos >= 2 && bytes[pos - 2] == b'*' && bytes[pos - 1] == b'/' {
      let end_pos = pos;
      pos -= 2;
      let mut found = false;
      while pos > 0 {
        if bytes[pos - 1] == b'/' && bytes[pos] == b'*' {
          pos -= 1; // include the `/`
          found = true;
          break;
        }
        pos -= 1;
      }
      if bytes.get(pos) == Some(&b'/') && bytes.get(pos + 1) == Some(&b'*') {
        found = true;
      }
      if !found {
        pos = end_pos;
        break;
      }
      continue;
    }

    if pos > 0 {
      let mut line_start = pos;
      while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
      }
      let line = &source[line_start..pos];
      let trimmed = line.trim();
      if trimmed.starts_with("//") && !trimmed.starts_with("/// <reference") {
        pos = line_start;
        continue;
      }
    }

    break;
  }

  pos
}

/// After rename, rewrite `export { Item$1 }` inside namespaces to `export { Item$1 as Item }`.
fn fix_namespace_export_specifiers(source: &str, renames: &[(String, String)]) -> String {
  let rename_map: FxHashMap<&str, &str> =
    renames.iter().map(|(old, new)| (new.as_str(), old.as_str())).collect();

  let allocator = Allocator::default();
  let source_type = SourceType::d_ts();
  let parser = Parser::new(&allocator, source, source_type);
  let parse_result = parser.parse();

  if parse_result.panicked {
    return source.to_string();
  }

  let mut replacements: Vec<(usize, usize, String)> = Vec::new();

  for stmt in &parse_result.program.body {
    collect_ns_export_fixes(stmt, &rename_map, &mut replacements);
  }

  if replacements.is_empty() {
    return source.to_string();
  }

  replacements.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
  let mut result = source.to_string();
  for (start, end, replacement) in replacements {
    if start < result.len() && end <= result.len() {
      result = format!("{}{}{}", &result[..start], replacement, &result[end..]);
    }
  }

  result
}

fn collect_ns_export_fixes(
  stmt: &Statement,
  rename_map: &FxHashMap<&str, &str>,
  replacements: &mut Vec<(usize, usize, String)>,
) {
  if let Statement::TSNamespaceDeclaration(module) = stmt {
    collect_ns_export_fixes_from_body(&module.body, rename_map, replacements);
  }
}

fn collect_ns_export_fixes_from_body(
  body: &TSNamespaceDeclarationBody,
  rename_map: &FxHashMap<&str, &str>,
  replacements: &mut Vec<(usize, usize, String)>,
) {
  match body {
    TSNamespaceDeclarationBody::TSModuleBlock(block) => {
      for inner_stmt in &block.body {
        if let Statement::ExportNamedDeclaration(export) = inner_stmt
          && !export.specifiers.is_empty()
        {
          let mut new_specifiers = Vec::new();
          let mut any_changed = false;

          for spec in &export.specifiers {
            let local_name = spec.local.name().to_string();
            let exported_name = spec.exported.name().to_string();

            if let Some(&original_name) = rename_map.get(local_name.as_str()) {
              if exported_name == local_name {
                new_specifiers.push(format!("{local_name} as {original_name}"));
                any_changed = true;
              } else {
                new_specifiers.push(format!("{local_name} as {exported_name}"));
              }
            } else if local_name == exported_name {
              new_specifiers.push(local_name);
            } else {
              new_specifiers.push(format!("{local_name} as {exported_name}"));
            }
          }

          if any_changed {
            let export_span = export.span();
            let replacement = format!("export {{ {} }};", new_specifiers.join(", "));
            replacements.push((export_span.start as usize, export_span.end as usize, replacement));
          }
        }
        collect_ns_export_fixes(inner_stmt, rename_map, replacements);
      }
    }
    TSNamespaceDeclarationBody::TSNamespaceDeclaration(inner) => {
      collect_ns_export_fixes_from_body(&inner.body, rename_map, replacements);
    }
  }
}

/// Replace an identifier in source text using word-boundary matching.
/// This avoids partial matches (e.g., replacing "T" in "TypeInfo").
fn replace_identifier(source: &str, old_name: &str, new_name: &str) -> String {
  replace_identifier_except_spans(source, old_name, new_name, &[])
}

/// Like [`replace_identifier`], but skips matches that overlap any of `skip_spans`.
fn replace_identifier_except_spans(
  source: &str,
  old_name: &str,
  new_name: &str,
  skip_spans: &[(u32, u32)],
) -> String {
  let pattern = format!(r"\b{}\b", regex::escape(old_name));
  let re = regex::Regex::new(&pattern).unwrap();

  // `\b` treats `$` as non-word, so `\bB\b` matches the `B` in `B$1` (skip those).
  let mut result = String::with_capacity(source.len());
  let mut last_end = 0;

  for mat in re.find_iter(source) {
    let match_start = mat.start();
    let match_end = mat.end();
    if source[match_end..].starts_with('$') {
      result.push_str(&source[last_end..match_end]);
      last_end = match_end;
      continue;
    }
    let overlaps_skip = skip_spans.iter().any(|&(s, e)| {
      let s = s as usize;
      let e = e as usize;
      match_start < e && match_end > s
    });
    if overlaps_skip {
      result.push_str(&source[last_end..match_end]);
      last_end = match_end;
      continue;
    }
    // `typeof` / `infer` operands are dep refs, not bindings.
    let before = &source[last_end..match_start];
    if before.trim_end().ends_with("typeof") {
      result.push_str(&source[last_end..match_end]);
      last_end = match_end;
      continue;
    }
    if before.trim_end().ends_with("infer") {
      result.push_str(&source[last_end..match_end]);
      last_end = match_end;
      continue;
    }
    result.push_str(&source[last_end..match_start]);
    result.push_str(new_name);
    last_end = match_end;
  }
  result.push_str(&source[last_end..]);
  result
}

#[cfg(test)]
mod tests {
  use super::*;

  fn replace_dependency_spans(
    source: &mut String,
    dep_refs: &[(String, u32, u32)],
    old_name: &str,
    new_name: &str,
  ) {
    let mut spans: Vec<(u32, u32)> = dep_refs
      .iter()
      .filter(|(name, _, _)| name == old_name)
      .map(|(_, start, end)| (*start, *end))
      .collect();
    spans.sort_by_key(|(start, _)| std::cmp::Reverse(*start));
    for (start, end) in spans {
      let start = start as usize;
      let end = end as usize;
      if start < end && end <= source.len() {
        source.replace_range(start..end, new_name);
      }
    }
  }

  #[test]
  fn test_plugin_creation() {
    let options = FakeJsOptions::default();
    let plugin = FakeJsPlugin::new(options);
    assert!(!plugin.options.sourcemap);
  }

  #[test]
  fn test_transform_non_dts() {
    let options = FakeJsOptions::default();
    let plugin = FakeJsPlugin::new(options);
    let code = "const x = 1;";
    let result = plugin.transform(code, "test.ts").unwrap();
    assert_eq!(result.code, code);
  }

  #[test]
  fn test_transform_simple_interface() {
    let options = FakeJsOptions::default();
    let plugin = FakeJsPlugin::new(options);
    let code = "export interface Foo { bar: string; }";
    let result = plugin.transform(code, "test.d.ts").unwrap();
    assert!(result.code.contains("var Foo"));
    assert!(result.code.contains("export"));
  }

  #[test]
  fn test_transform_declare_global_side_effect() {
    let options = FakeJsOptions { side_effects: true, ..FakeJsOptions::default() };
    let plugin = FakeJsPlugin::new(options);
    let code = "declare global {\n  let sideEffectExecuted: boolean\n}\n\nexport {}";
    let result = plugin.transform(code, "mod.d.ts").unwrap();
    assert!(result.code.contains("var _0"), "expected runtime binding, got: {}", result.code);
  }

  #[test]
  fn test_infer_false_branch_render_with_local_u() {
    let options = FakeJsOptions::default();
    let plugin = FakeJsPlugin::new(options);
    let code = "type U = 'local'\nexport type Test<T> = T extends Array<infer U> ? (T extends Array<infer U2> ? U2 : U) : U";
    plugin.transform(code, "index.d.ts").unwrap();
    let chunk = "var U = [0, () => [], []]\nexport var Test = [1, (T) => [U$1], []]";
    let result = plugin
      .render_chunk(
        chunk,
        &ChunkInfo { filename: "index.d.ts".into(), module_ids: vec!["index.d.ts".into()] },
      )
      .unwrap();
    assert!(
      result.code.contains(": U$1"),
      "expected false-branch dep rename, got: {}",
      result.code
    );
  }

  #[test]
  fn test_infer_false_branch_render() {
    let options = FakeJsOptions::default();
    let plugin = FakeJsPlugin::new(options);
    let code =
      "export type Test<T> = T extends Array<infer U> ? (T extends Array<infer U2> ? U2 : U) : U";
    plugin.transform(code, "index.d.ts").unwrap();
    let chunk = "export var Test = [0, (T) => [U$1], []]";
    let result = plugin
      .render_chunk(
        chunk,
        &ChunkInfo { filename: "index.d.ts".into(), module_ids: vec!["index.d.ts".into()] },
      )
      .unwrap();
    assert!(
      result.code.contains(": U$1"),
      "expected false-branch dep rename, got: {}",
      result.code
    );
  }

  #[test]
  fn test_replace_identifier() {
    assert_eq!(replace_identifier("T extends Foo<T>", "T", "T$1"), "T$1 extends Foo<T$1>");
    // \bT\b should NOT match T in TypeInfo since T is followed by word char 'y'
    assert_eq!(replace_identifier("type T = TypeInfo", "T", "T$1"), "type T$1 = TypeInfo");
    assert_eq!(replace_identifier("Foo extends Bar", "Foo", "Baz"), "Baz extends Bar");
    // Should NOT double-rename: B$1 should stay B$1, not become B$1$1
    assert_eq!(
      replace_identifier("B$1 extends SomeInterface", "B", "B$1"),
      "B$1 extends SomeInterface"
    );
    // Should still rename B that is NOT followed by $
    assert_eq!(replace_identifier("B extends B$1", "B", "B$2"), "B$2 extends B$1");
    // Dependency renames use span-based replacement; replace_identifier is for bindings/params only.
    let mut nested =
      "T extends Array<infer U> ? (T extends Array<infer U2> ? U2 : U) : U".to_string();
    let last_u = u32::try_from(nested.rfind('U').unwrap()).unwrap();
    replace_dependency_spans(&mut nested, &[("U".to_string(), last_u, last_u + 1)], "U", "U$2");
    assert_eq!(nested, "T extends Array<infer U> ? (T extends Array<infer U2> ? U2 : U) : U$2");
  }
}
