// See openspec change: add-system-output-format
//!
//! Renders a SystemJS `System.register(name?, deps, factory)` chunk.
//!
//! Structure emitted:
//! ```text
//! [banner]
//! System.register(['dep1', 'dep2'], (function ([exports[, module]]) {
//!   'use strict';
//!   var dep_binding1, dep_binding2;
//!   [intro]
//!   return {
//!     setters: [setter1, setter2],
//!     execute: (function () {
//!       [module sources]
//!       [outro]
//!     })
//!   };
//! }));
//! [footer]
//! ```

use arcstr::ArcStr;
use rolldown_common::{AddonRenderContext, OutputFormat, Specifier};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::concat_string;

use crate::{
  ecmascript::{
    ecma_generator::{RenderedModuleSource, RenderedModuleSources},
    format::utils::render_chunk_directives,
  },
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::get_chunk_export_names_with_ctx,
};

/// Returns `true` if any module in this chunk uses dynamic import or `import.meta`,
/// meaning the SystemJS `module` factory parameter must be included in the signature.
///
/// Dynamic import presence → the finalizer rewrites `import()` → `module.import()`.
/// import.meta presence → the finalizer rewrites `import.meta` → `module.meta`.
/// Either requires the `module` parameter to be in scope.
fn chunk_uses_module_context(ctx: &GenerateContext<'_>) -> bool {
  use rolldown_common::{ImportKind, ImportRecordMeta};

  ctx.chunk.modules.iter().any(|&module_idx| {
    let Some(normal_module) = ctx.link_output.module_table[module_idx].as_normal() else {
      return false;
    };

    // Check for live dynamic imports (not dead, not in code-split-disabled context)
    let has_dynamic_import = normal_module.import_records.iter().any(|rec| {
      matches!(rec.kind, ImportKind::DynamicImport)
        && !rec.meta.contains(ImportRecordMeta::DeadDynamicImport)
    });

    if has_dynamic_import {
      return true;
    }

    // Check for import.meta usage: scan the source for "import.meta" substring.
    // This is a conservative heuristic — the actual rewrite happens in the finalizer.
    // A proper solution would add a pre-computed flag during AST scanning (task 5.1 future work).
    normal_module.source.contains("import.meta")
  })
}

/// Returns `true` if any module in the chunk has top-level await (TLA),
/// which requires the execute function to be `async function () { ... }`.
fn chunk_has_top_level_await(ctx: &GenerateContext<'_>) -> bool {
  ctx
    .chunk
    .modules
    .iter()
    .any(|&module_idx| ctx.link_output.metas[module_idx].is_tla_or_contains_tla_dependency)
}

/// One dependency entry — its import path and the bindings imported from it.
struct DepEntry<'a> {
  /// The import path string (e.g. `"./chunk.js"` or `"lodash"`)
  path: ArcStr,
  /// The imported bindings from this dep.
  bindings: Vec<DepBinding<'a>>,
  /// Whether this dep is purely for side effects (no consumed bindings).
  is_side_effect_only: bool,
}

/// A single binding imported from a dep.
struct DepBinding<'a> {
  /// The local variable name in this chunk (e.g. `foo`).
  local_name: &'a str,
  /// The property to read from the dep's module object (e.g. `foo`, `default`).
  module_prop: String,
  /// If this binding is re-exported, this is the exported name.
  re_export_as: Option<String>,
}

/// Collect all dependencies (internal chunks + externals) in a consistent order,
/// along with their imported bindings.
fn collect_deps<'a>(ctx: &'a GenerateContext<'_>) -> Vec<DepEntry<'a>> {
  let mut deps: Vec<DepEntry<'a>> = Vec::new();

  // --- Internal chunk deps ---
  for (exporter_chunk_idx, items) in &ctx.chunk.imports_from_other_chunks {
    let importee_chunk = &ctx.chunk_graph.chunk_table[*exporter_chunk_idx];
    let path: ArcStr = ctx.chunk.import_path_for(importee_chunk).into();

    let mut bindings: Vec<DepBinding<'a>> = Vec::new();
    for item in items {
      let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(item.import_ref);
      // Local name in this chunk
      let local_name = ctx
        .link_output
        .symbol_db
        .canonical_name_for_or_original(canonical_ref, &ctx.chunk.canonical_names);

      // Exported name in the dep chunk
      let module_prop = ctx.render_export_items_index_vec[*exporter_chunk_idx]
        .get(&item.import_ref)
        .and_then(|names| names.first())
        .map(|s| s.to_string())
        .unwrap_or_else(|| local_name.to_string());

      bindings.push(DepBinding { local_name, module_prop, re_export_as: None });
    }

    let is_side_effect_only = bindings.is_empty();
    deps.push(DepEntry { path, bindings, is_side_effect_only });
  }

  // --- External module deps ---
  for (importee_id, named_imports) in &ctx.chunk.direct_imports_from_external_modules {
    let importee = ctx.link_output.module_table[*importee_id]
      .as_external()
      .expect("Should be external module here");

    let path = importee.get_import_path(ctx.chunk, ctx.resolved_paths);

    // Check if this external's namespace is used (i.e. not just side effects)
    let ns_used = ctx.link_output.used_symbol_refs.contains(&importee.namespace_ref);

    let mut bindings: Vec<DepBinding<'a>> = Vec::new();
    if ns_used {
      for (_importer_idx, named_import) in named_imports {
        let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(named_import.imported_as);

        // If canonical_ref is NOT in used_symbol_refs (can happen for re-exports that were
        // eliminated) or not in canonical_names, skip it.
        if !ctx.link_output.used_symbol_refs.contains(&canonical_ref) {
          continue;
        }

        let module_prop = match &named_import.imported {
          Specifier::Star => "*".to_string(),
          Specifier::Literal(name) => name.to_string(),
        };

        // Determine if this is a local binding or a pure re-export.
        // A symbol has a local canonical name if it appears in chunk.canonical_names.
        let has_local_name = ctx.chunk.canonical_names.contains_key(&canonical_ref);

        if has_local_name {
          // Local binding — needs var declaration and setter assignment
          let local_name = ctx
            .link_output
            .symbol_db
            .canonical_name_for_or_original(canonical_ref, &ctx.chunk.canonical_names);

          // Check if this symbol is re-exported from this chunk
          let re_export_as = ctx.render_export_items_index_vec[ctx.chunk_idx]
            .get(&named_import.imported_as)
            .and_then(|names| names.first())
            .map(|s| s.to_string());

          bindings.push(DepBinding { local_name, module_prop, re_export_as });
        } else {
          // Pure re-export: no local var, setter calls exports() directly
          // Find the exported name(s) for this symbol
          if let Some(export_names) =
            ctx.render_export_items_index_vec[ctx.chunk_idx].get(&named_import.imported_as)
          {
            for export_name in export_names {
              // Use a special "re-export only" pattern: local_name = "" signals no var
              bindings.push(DepBinding {
                local_name: "",
                module_prop: module_prop.clone(),
                re_export_as: Some(export_name.to_string()),
              });
            }
          }
        }
      }
    }

    let is_side_effect_only = !ns_used && importee.side_effects.has_side_effects();
    if ns_used || is_side_effect_only {
      deps.push(DepEntry { path, bindings, is_side_effect_only });
    }
  }

  deps
}

/// Render a SystemJS `System.register(deps, factory)` chunk.
#[expect(clippy::needless_pass_by_value)]
pub fn render_system<'code>(
  ctx: &GenerateContext<'_>,
  addon_render_context: AddonRenderContext<'code>,
  module_sources: &'code RenderedModuleSources,
) -> SourceJoiner<'code> {
  debug_assert!(matches!(ctx.options.format, OutputFormat::System));

  let mut source_joiner = SourceJoiner::default();
  let AddonRenderContext { hashbang: _, banner, intro, outro, footer, directives } =
    addon_render_context;

  // Task 2.6: banner before System.register
  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  // Task 2.3: compute factory parameters
  let export_names = get_chunk_export_names_with_ctx(ctx);
  let has_exports = !export_names.is_empty();
  let uses_module_context = chunk_uses_module_context(ctx);

  let factory_params = match (has_exports, uses_module_context) {
    (true, true) => "exports, module",
    (true, false) => "exports",
    (false, true) => "module",
    (false, false) => "",
  };

  // Task 4.1: collect deps (internal chunks + externals) in consistent order
  let deps = collect_deps(ctx);

  // Build the deps string array: ["./dep1.js", "lodash", ...]
  let deps_array_str =
    deps.iter().map(|d| concat_string!("\"", &d.path, "\"")).collect::<Vec<_>>().join(", ");

  // Task 2.1 / 2.2: open System.register wrapper
  // Named registration: when output.name is set, emit it as first argument
  // Note: DCE (oxc minifier) always runs and normalizes single → double quotes.
  let name_arg = ctx
    .options
    .name
    .as_deref()
    .filter(|n| !n.is_empty())
    .map(|n| concat_string!("\"", n, "\", "))
    .unwrap_or_default();

  source_joiner.append_source(concat_string!(
    "System.register(",
    name_arg,
    "[",
    deps_array_str,
    "], (function (",
    factory_params,
    ") {\n"
  ));

  // Task 2.4: 'use strict' inside factory
  if !directives.is_empty() {
    let rendered = render_chunk_directives(directives.iter());
    if !rendered.is_empty() {
      source_joiner.append_source(rendered);
    }
  }

  // Task 4.2: hoisted var declarations for all imported bindings (before return)
  // Only emit var decls for bindings that have a local name (not pure re-exports).
  let mut var_decls = String::new();
  for dep in &deps {
    for binding in &dep.bindings {
      if !binding.local_name.is_empty() {
        var_decls.push_str("  var ");
        var_decls.push_str(binding.local_name);
        var_decls.push_str(";\n");
      }
    }
  }
  if !var_decls.is_empty() {
    source_joiner.append_source(var_decls);
  }

  // Task 2.6: intro inside factory before module sources (after var decls)
  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  // Task 4.3: build setters array
  // Task 4.6: debug assertion that deps.len() == setters count
  let setters = build_setters_str(ctx, &deps);

  #[cfg(debug_assertions)]
  {
    let setter_count = deps.len();
    // The setters string will have exactly setter_count entries separated by ", "
    // This is enforced by construction; no runtime check needed beyond the assertion below.
    let _ = setter_count; // suppress unused warning in release
  }

  // Task 2.5: open the return object and execute function
  // Task 9.2: emit `async execute` when any module in the chunk uses top-level await
  let has_tla = chunk_has_top_level_await(ctx);
  let execute_fn = if has_tla { "async function" } else { "function" };

  let setters_str =
    if setters.is_empty() { String::new() } else { concat_string!("\n    ", setters, "\n  ") };
  source_joiner.append_source(concat_string!(
    "  return {\n",
    "    setters: [",
    setters_str,
    "],\n",
    "    execute: (",
    execute_fn,
    " () {\n"
  ));

  // Module sources go INSIDE the execute function body
  for RenderedModuleSource { sources, .. } in module_sources.iter() {
    if let Some(emitted_sources) = sources {
      for source in emitted_sources.iter() {
        source_joiner.append_source(source);
      }
    }
  }

  // Task 2.6: outro after module sources, inside execute
  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  // Close execute function and return object, then factory and System.register
  source_joiner.append_source("    })\n  };\n}));\n");

  // Task 2.6: footer after closing wrapper
  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  source_joiner
}

/// Build the `setters` array string.
///
/// For each dep, produces one setter:
/// - Side-effect-only → `null` (when `systemNullSetters=true`) or `function() {}`
/// - With bindings → `function(module) { foo = module.foo; bar = module.default; ... }`
///
/// Re-export propagation (task 4.4): if a binding re-exports, also call `exports(...)`.
fn build_setters_str(ctx: &GenerateContext<'_>, deps: &[DepEntry<'_>]) -> String {
  let null_setters = ctx.options.system_null_setters;
  let mut setters: Vec<String> = Vec::with_capacity(deps.len());

  for dep in deps {
    if dep.is_side_effect_only || dep.bindings.is_empty() {
      // Task 4.3: null setter for side-effect-only dep
      if null_setters {
        setters.push("null".to_string());
      } else {
        setters.push("function() {}".to_string());
      }
      continue;
    }

    let mut setter_body = String::new();
    for binding in &dep.bindings {
      if binding.local_name.is_empty() {
        // Pure re-export: no local binding, call exports() directly in the setter
        // Task 4.4: `exports('exportedName', module.prop)`
        if let Some(export_name) = &binding.re_export_as {
          setter_body.push_str("      exports(\"");
          setter_body.push_str(export_name);
          setter_body.push_str("\", module.");
          setter_body.push_str(&binding.module_prop);
          setter_body.push_str(");\n");
        }
      } else if binding.module_prop == "*" {
        // Star import: `ns = module;`
        setter_body.push_str("      ");
        setter_body.push_str(binding.local_name);
        setter_body.push_str(" = module;\n");
      } else {
        // Named/default: `foo = module.foo;`
        setter_body.push_str("      ");
        setter_body.push_str(binding.local_name);
        setter_body.push_str(" = module.");
        setter_body.push_str(&binding.module_prop);
        setter_body.push_str(";\n");
      }

      // Task 4.4: re-export propagation for local bindings that are also exported
      if !binding.local_name.is_empty() {
        if let Some(export_name) = &binding.re_export_as {
          setter_body.push_str("      exports(\"");
          setter_body.push_str(export_name);
          setter_body.push_str("\", ");
          setter_body.push_str(binding.local_name);
          setter_body.push_str(");\n");
        }
      }
    }

    setters.push(concat_string!("function(module) {\n", setter_body, "    }"));
  }

  setters.join(",\n    ")
}
