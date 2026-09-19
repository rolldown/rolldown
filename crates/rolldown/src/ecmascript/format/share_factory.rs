//! Emission for `experimentalInlineCommonChunks`.
//!
//! An inlined chunk is rendered once into a `__rd_share(id, factory)` registration. Every chunk that
//! carries it prints that registration in its own prologue, and every chunk that statically imported
//! it calls `__rd_share_require(id)` there instead. The registry itself is printed into the chunk
//! that holds the runtime module, so one loaded output graph has exactly one factory and module
//! table.

use std::ops::Range;

use json_escape_simd::escape;
use rolldown_common::{ChunkIdx, ModuleId, RenderedModule};
use rolldown_sourcemap::{SourceJoiner, SourceMap, SourceMapSource};
use rolldown_utils::{concat_string, ecmascript::is_validate_identifier_name};
use rustc_hash::FxHashMap;

use crate::{
  stages::generate_stage::inline_common_chunks::{SHARE_DEFINE_NAME, SHARE_REQUIRE_NAME},
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::get_export_items,
};

/// Factory parameter names. They are reserved in every inlined logical chunk's renamer, so an
/// inlined module's own `module`/`exports` binding can never capture them.
const MODULE_PARAM: &str = "__rd_m";
const EXPORTS_PARAM: &str = "__rd_e";
const DEFINE_PARAM: &str = "__rd_def";
const REQUIRE_PARAM: &str = "__rd_req";

/// One inlined chunk, rendered once and reused by every chunk that carries it.
///
/// The factory body and suffix are host-independent by construction: selection rejects any chunk
/// whose body would contain a chunk-relative path. Its leading registration callee is localized and
/// the `import` declarations it needs are re-rendered per host, because output directories differ.
#[derive(Debug)]
pub struct InlinedChunkRender {
  /// The complete `__rd_share(...)` registration statement.
  pub factory: String,
  /// Byte range of the rendered chunk body inside `factory`.
  pub body_range: Range<usize>,
  /// Mappings for the body before the factory prefix is added. Carriers split `factory` at
  /// `body_range` so `SourceJoiner` can offset a clone of this map at the correct generated line.
  pub body_map: Option<SourceMap>,
  /// Rendered-module ledger entries, merged into every carrier's `chunk.modules`.
  pub rendered_modules: FxHashMap<ModuleId, RenderedModule>,
}

/// The prefixed string key as it appears in emitted code. Minification can rename registry bindings
/// but leaves the logical key unchanged.
pub fn share_key_of(ctx: &GenerateContext<'_>, chunk_idx: ChunkIdx) -> String {
  let key = ctx.chunk_graph.chunk_table[chunk_idx]
    .inline_share_key
    .as_deref()
    .expect("inlined chunk should have a registry key");
  format!("\"rd:{key}\"")
}

/// The registry. Printed into the chunk that holds the runtime module.
///
/// `__rd_share_require` caches the module record before running the factory, so a cycle that
/// re-enters it observes the partially populated exports object rather than running the factory
/// twice. A factory that throws records the failure and rethrows it on every later require, which
/// is the ESM rule rather than CommonJS's retry.
pub fn render_registry() -> String {
  let define = SHARE_DEFINE_NAME;
  let require = SHARE_REQUIRE_NAME;
  format!(
    "var __rd_factories = {{}}, __rd_records = {{}};\n\
     function {define}(id, factory) {{\n\
     \tif (__rd_factories[id] === void 0) __rd_factories[id] = factory;\n\
     }}\n\
     function __rd_share_define(target, all) {{\n\
     \tfor (var name in all) Object.defineProperty(target, name, {{ get: all[name], enumerable: true }});\n\
     }}\n\
     function {require}(id) {{\n\
     \tvar record = __rd_records[id];\n\
     \tif (record !== void 0) {{\n\
     \t\tif (record.failed) throw record.error;\n\
     \t\treturn record.module.exports;\n\
     \t}}\n\
     \tvar factory = __rd_factories[id];\n\
     \tif (factory === void 0) throw new Error(\"Shared module \" + id + \" is not defined\");\n\
     \trecord = __rd_records[id] = {{ module: {{ exports: {{}} }}, failed: false, error: void 0 }};\n\
     \ttry {{\n\
     \t\tfactory(record.module, record.module.exports, __rd_share_define, {require});\n\
     \t}} catch (error) {{\n\
     \t\trecord.failed = true;\n\
     \t\trecord.error = error;\n\
     \t\tthrow error;\n\
     \t}}\n\
     \treturn record.module.exports;\n\
     }}\n\
     export {{ {define}, {require} }};\n",
  )
}

/// Builds the single `__rd_share(...)` registration for one inlined chunk.
pub fn render_inlined_chunk_factory(
  ctx: &GenerateContext<'_>,
  body: &str,
) -> (String, Range<usize>) {
  let share_key = share_key_of(ctx, ctx.chunk_idx);
  let mut requires = String::new();
  for required in &ctx.chunk.required_inline_chunks {
    let binding = ctx
      .chunk
      .inline_binding_names_for_other_chunks
      .get(required)
      .expect("required inlined chunk should have a binding name");
    requires.push_str(&concat_string!(
      "var ",
      binding,
      " = ",
      REQUIRE_PARAM,
      "(",
      share_key_of(ctx, *required),
      ");\n"
    ));
  }
  let exports = render_factory_exports(ctx);
  // A rendered module body can end inside a `//#endregion` line comment, which would swallow the
  // export glue that follows it.
  let body_terminator = if body.ends_with('\n') { "" } else { "\n" };
  let mut factory = concat_string!(
    SHARE_DEFINE_NAME,
    "(",
    share_key,
    ", (",
    MODULE_PARAM,
    ", ",
    EXPORTS_PARAM,
    ", ",
    DEFINE_PARAM,
    ", ",
    REQUIRE_PARAM,
    ") => {\n",
    requires
  );
  let body_start = factory.len();
  factory.push_str(body);
  let body_end = factory.len();
  factory.push_str(body_terminator);
  factory.push_str(&exports);
  factory.push_str("});\n");
  (factory, body_start..body_end)
}

/// The factory's export interface. Getters keep a reassigned export live across the boundary, which
/// a snapshot binding would not.
fn render_factory_exports(ctx: &GenerateContext<'_>) -> String {
  let export_items = get_export_items(ctx.chunk);
  if export_items.is_empty() {
    return String::new();
  }
  let symbol_db = &ctx.link_output.symbol_db;
  let mut prelude = String::new();
  let mut entries: Vec<String> = Vec::with_capacity(export_items.len());
  for (exported_name, export_ref) in export_items {
    let canonical_ref = symbol_db.canonical_ref_for(export_ref);
    let symbol = symbol_db.get(canonical_ref);
    let canonical_name =
      symbol_db.canonical_name_for_or_original(canonical_ref, &ctx.chunk.canonical_names);
    if let Some(ns_alias) = &symbol.namespace_alias {
      let canonical_ns_name = symbol_db
        .canonical_name_for_or_original(ns_alias.namespace_ref, &ctx.chunk.canonical_names);
      prelude.push_str(&concat_string!(
        "var ",
        canonical_name,
        " = ",
        canonical_ns_name,
        ".",
        ns_alias.property_name.as_str(),
        ";\n"
      ));
    }
    let key = render_export_key(exported_name.as_str());
    entries.push(concat_string!(key, ": () => ", canonical_name));
  }
  concat_string!(prelude, DEFINE_PARAM, "(", EXPORTS_PARAM, ", { ", entries.join(", "), " });\n")
}

fn render_export_key(exported_name: &str) -> String {
  if exported_name != "__proto__" && is_validate_identifier_name(exported_name) {
    exported_name.to_string()
  } else {
    format!("[{exported_name:?}]")
  }
}

/// Appends the prologue a carrying or consuming chunk prints before its own body. A mapped factory
/// is split around its body so the body's original mappings survive every physical placement.
pub fn append_share_prologue(ctx: &GenerateContext<'_>, source_joiner: &mut SourceJoiner<'_>) {
  let carried = &ctx.chunk.carried_inline_chunks;
  let required = &ctx.chunk.required_inline_chunks;
  if carried.is_empty() && required.is_empty() {
    return;
  }
  let mut imports = String::new();
  for carried_idx in carried {
    let source = &ctx.chunk_graph.chunk_table[*carried_idx];
    if let Some(rendered) = crate::ecmascript::format::esm::render_esm_imports_of(ctx, source) {
      imports.push_str(&rendered);
    }
  }
  if !imports.is_empty() {
    source_joiner.append_source(imports);
  }
  let share_define_name = ctx
    .chunk
    .inline_share_define_name
    .as_deref()
    .expect("a carrier should have a local share-definition binding");
  for carried_idx in carried {
    let render = ctx.inline_renders.get(carried_idx).expect("inlined chunk should be rendered");
    debug_assert!(render.factory.starts_with(SHARE_DEFINE_NAME));
    if let Some(map) = &render.body_map {
      source_joiner.append_source(concat_string!(
        share_define_name,
        &render.factory[SHARE_DEFINE_NAME.len()..render.body_range.start]
      ));
      source_joiner.append_source(SourceMapSource::new(
        render.factory[render.body_range.clone()].to_string(),
        map.clone(),
      ));
      source_joiner.append_source(render.factory[render.body_range.end..].to_string());
    } else {
      source_joiner.append_source(concat_string!(
        share_define_name,
        &render.factory[SHARE_DEFINE_NAME.len()..]
      ));
    }
  }
  let share_require_name = ctx
    .chunk
    .inline_share_require_name
    .as_deref()
    .expect("an inline-chunk consumer should have a local share-require binding");
  let mut requires = String::new();
  for required_idx in required {
    let binding = ctx
      .chunk
      .inline_binding_names_for_other_chunks
      .get(required_idx)
      .expect("required inlined chunk should have a binding name");
    requires.push_str(&concat_string!(
      "var ",
      binding,
      " = ",
      share_require_name,
      "(",
      share_key_of(ctx, *required_idx),
      ");\n"
    ));
  }
  if !requires.is_empty() {
    source_joiner.append_source(requires);
  }
}

/// The registry import a carrying or consuming chunk needs, unless it is the registry chunk itself.
pub fn render_registry_import(ctx: &GenerateContext<'_>) -> Option<String> {
  let registry_chunk = ctx.inline_registry_chunk?;
  if registry_chunk == ctx.chunk_idx {
    return None;
  }
  if ctx.chunk.carried_inline_chunks.is_empty() && ctx.chunk.required_inline_chunks.is_empty() {
    return None;
  }
  let path = ctx.chunk.import_path_for(&ctx.chunk_graph.chunk_table[registry_chunk]);
  let define_name = ctx
    .chunk
    .inline_share_define_name
    .as_deref()
    .expect("a carrier should have a local share-definition binding");
  let require_name = ctx
    .chunk
    .inline_share_require_name
    .as_deref()
    .expect("an inline-chunk consumer should have a local share-require binding");
  let define_specifier = if define_name == SHARE_DEFINE_NAME {
    SHARE_DEFINE_NAME.to_string()
  } else {
    concat_string!(SHARE_DEFINE_NAME, " as ", define_name)
  };
  let require_specifier = if require_name == SHARE_REQUIRE_NAME {
    SHARE_REQUIRE_NAME.to_string()
  } else {
    concat_string!(SHARE_REQUIRE_NAME, " as ", require_name)
  };
  Some(concat_string!(
    "import { ",
    define_specifier,
    ", ",
    require_specifier,
    " } from ",
    escape(&path),
    ";\n"
  ))
}

#[cfg(test)]
mod tests {
  use super::render_export_key;

  #[test]
  fn factory_export_keys_are_safe_object_literal_keys() {
    assert_eq!(render_export_key("normal"), "normal");
    assert_eq!(render_export_key("with space"), r#"["with space"]"#);
    assert_eq!(render_export_key("__proto__"), r#"["__proto__"]"#);
  }
}
