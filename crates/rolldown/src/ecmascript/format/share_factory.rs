//! Emission for `codeSplitting.inlineCommonChunks`.
//!
//! An inlined chunk is rendered once into a `__rd_share(id, factory)` registration. Every chunk that
//! carries it prints that registration in its own prologue, and every chunk that statically imported
//! it calls `__rd_share_require(id)` there instead. The registry itself is printed into the chunk
//! that holds the runtime module, so one JavaScript realm has exactly one factory and module table.

use rolldown_common::{ChunkIdx, ModuleId, RenderedModule};
use rolldown_utils::{concat_string, ecmascript::is_validate_identifier_name};
use rustc_hash::FxHashMap;

use crate::{
  stages::generate_stage::inline_common_chunks::{SHARE_DEFINE_NAME, SHARE_REQUIRE_NAME},
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::get_export_items,
};

/// Factory parameter names. They are reserved in every chunk's renamer while the feature is on, so
/// an inlined module's own `module`/`exports` binding can never capture them.
const MODULE_PARAM: &str = "__rd_m";
const EXPORTS_PARAM: &str = "__rd_e";
const DEFINE_PARAM: &str = "__rd_def";

/// One inlined chunk, rendered once and reused by every chunk that carries it.
///
/// The factory text is host-independent by construction: selection rejects any chunk whose body
/// would contain a chunk-relative path. The `import` declarations it needs are re-rendered per host,
/// because output directories differ.
#[derive(Debug, Default)]
pub struct InlinedChunkRender {
  /// The complete `__rd_share(...)` registration statement.
  pub factory: String,
  /// Rendered-module ledger entries, merged into every carrier's `chunk.modules`.
  pub rendered_modules: FxHashMap<ModuleId, RenderedModule>,
  pub module_ids: Vec<ModuleId>,
}

/// The registry key as it appears in emitted code. Minification renames the local bindings the
/// registry is imported under, so a bare numeric argument cannot be told apart from any other call
/// with a numeric argument. A short prefixed string literal survives minification verbatim, which is
/// what makes the `__share` before `__share_require` property checkable over emitted output.
pub fn share_key_of(ctx: &GenerateContext<'_>, chunk_idx: ChunkIdx) -> String {
  let id = ctx.chunk_graph.chunk_table[chunk_idx]
    .inline_share_id
    .expect("inlined chunk should have a registry id");
  format!("\"rd:{id}\"")
}

/// The registry. Printed into the chunk that holds the runtime module.
///
/// `__rd_share_require` caches the module record before running the factory, so a cycle that
/// re-enters it observes the partially populated exports object rather than running the factory
/// twice. A factory that throws records the failure and rethrows it on every later require, which
/// is the ESM rule rather than CommonJS's retry.
pub fn render_registry() -> String {
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
     \t\tif (record.error !== void 0) throw record.error;\n\
     \t\treturn record.module.exports;\n\
     \t}}\n\
     \tvar factory = __rd_factories[id];\n\
     \tif (factory === void 0) throw new Error(\"Shared module \" + id + \" is not defined\");\n\
     \trecord = __rd_records[id] = {{ module: {{ exports: {{}} }}, error: void 0 }};\n\
     \ttry {{\n\
     \t\tfactory(record.module, record.module.exports, __rd_share_define);\n\
     \t}} catch (error) {{\n\
     \t\trecord.error = error;\n\
     \t\tthrow error;\n\
     \t}}\n\
     \treturn record.module.exports;\n\
     }}\n\
     export {{ {define}, {require} }};\n",
    define = SHARE_DEFINE_NAME,
    require = SHARE_REQUIRE_NAME,
  )
}

/// Builds the single `__rd_share(...)` registration for one inlined chunk.
pub fn render_inlined_chunk_factory(ctx: &GenerateContext<'_>, body: &str) -> String {
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
      SHARE_REQUIRE_NAME,
      "(",
      share_key_of(ctx, *required),
      ");\n"
    ));
  }
  let exports = render_factory_exports(ctx);
  // A rendered module body can end inside a `//#endregion` line comment, which would swallow the
  // export glue that follows it.
  let body_terminator = if body.ends_with('\n') { "" } else { "\n" };
  concat_string!(
    SHARE_DEFINE_NAME,
    "(",
    share_key,
    ", (",
    MODULE_PARAM,
    ", ",
    EXPORTS_PARAM,
    ", ",
    DEFINE_PARAM,
    ") => {\n",
    requires,
    body,
    body_terminator,
    exports,
    "});\n"
  )
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
    let key = if is_validate_identifier_name(exported_name.as_str()) {
      exported_name.to_string()
    } else {
      format!("{:?}", exported_name.as_str())
    };
    entries.push(concat_string!(key, ": () => ", canonical_name));
  }
  concat_string!(prelude, DEFINE_PARAM, "(", EXPORTS_PARAM, ", { ", entries.join(", "), " });\n")
}

/// The prologue a carrying or consuming chunk prints before its own body.
pub fn render_share_prologue(ctx: &GenerateContext<'_>) -> Option<String> {
  let carried = &ctx.chunk.carried_inline_chunks;
  let required = &ctx.chunk.required_inline_chunks;
  if carried.is_empty() && required.is_empty() {
    return None;
  }
  let mut s = String::new();
  for carried_idx in carried {
    let source = &ctx.chunk_graph.chunk_table[*carried_idx];
    if let Some(imports) = crate::ecmascript::format::esm::render_esm_imports_of(ctx, source) {
      s.push_str(&imports);
    }
  }
  for carried_idx in carried {
    let render = ctx.inline_renders.get(carried_idx).expect("inlined chunk should be rendered");
    s.push_str(&render.factory);
  }
  for required_idx in required {
    let binding = ctx
      .chunk
      .inline_binding_names_for_other_chunks
      .get(required_idx)
      .expect("required inlined chunk should have a binding name");
    s.push_str(&concat_string!(
      "var ",
      binding,
      " = ",
      SHARE_REQUIRE_NAME,
      "(",
      share_key_of(ctx, *required_idx),
      ");\n"
    ));
  }
  Some(s)
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
  Some(concat_string!(
    "import { ",
    SHARE_DEFINE_NAME,
    ", ",
    SHARE_REQUIRE_NAME,
    " } from \"",
    path,
    "\";\n"
  ))
}
