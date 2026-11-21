use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::fmt::Write as _;

use oxc_str::CompactStr;
use rolldown_common::{
  Chunk, ChunkKind, ExportsKind, GetLocalDb, IndexModules, ModuleIdx, NormalizedBundlerOptions,
  OutputExports, OutputFormat, Platform, SymbolRef, SymbolRefDb, SymbolRefFlags, WrapKind,
};
use rolldown_utils::{
  concat_string,
  ecmascript::{
    escape_non_ascii, property_access_str_with_ascii, to_ecma_string_literal,
    to_module_import_export_name_with_ascii,
  },
  indexmap::{FxIndexMap, FxIndexSet},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ast_scanner::const_eval::eval_can_assign;
use crate::esm_init_obligations::{WrappedEsmInitTarget, collect_entry_reexported_wrapper_inits};
use crate::stages::generate_stage::SharedReadDeclaration;
use crate::utils::chunk::namespace_alias_origin::{NamespaceAliasOrigin, namespace_alias_origin};
use crate::{stages::link_stage::LinkStageOutput, types::generator::GenerateContext};

pub fn render_wrapped_entry_chunk(
  ctx: &GenerateContext<'_>,
  export_mode: Option<&OutputExports>,
) -> Option<String> {
  if let ChunkKind::EntryPoint { module: entry_id, .. } = ctx.chunk.kind {
    let entry_meta = &ctx.link_output.metas[entry_id];
    if matches!(entry_meta.wrap_kind(), WrapKind::Cjs) {
      if matches!(ctx.options.format, OutputFormat::Cjs)
        && let Some(host_idx) = ctx.chunk_graph.module_to_chunk[entry_id]
        && host_idx != ctx.chunk_idx
        && ctx.chunk_graph.chunk_table[host_idx].entry_module_idx() == Some(entry_id)
      {
        let require_binding = &ctx.chunk.require_binding_names_for_other_chunks[&host_idx];
        return Some(concat_string!("module.exports = ", require_binding, ";\n"));
      }
      let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();

      let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
        *wrapper_ref,
        ctx.chunk_idx,
        &ctx.chunk.canonical_names,
      );
      let wrapper_ref_name = escape_non_ascii(&wrapper_ref_name, ctx.options.charset.is_ascii());

      return match ctx.options.format {
        OutputFormat::Esm => {
          // export default require_xxx();
          Some(concat_string!("export default ", wrapper_ref_name.as_ref(), "();\n"))
        }
        OutputFormat::Cjs => {
          if matches!(&export_mode, Some(OutputExports::Named)) {
            Some(render_object_define_property(
              "default",
              &concat_string!(wrapper_ref_name, "()"),
              ctx.options.charset.is_ascii(),
            ))
          } else {
            // module.exports = require_xxx();
            Some(concat_string!("module.exports = ", wrapper_ref_name, "();\n"))
          }
        }
        OutputFormat::Iife | OutputFormat::Umd => {
          if matches!(&export_mode, Some(OutputExports::Named)) {
            Some(render_object_define_property(
              "default",
              &concat_string!(wrapper_ref_name, "()"),
              ctx.options.charset.is_ascii(),
            ))
          } else {
            // return require_xxx();
            Some(concat_string!("return ", wrapper_ref_name, "();\n"))
          }
        }
      };
    }

    if let Some(targets) = ctx.order_wrap_state.consumer_local_namespace_targets(entry_id) {
      let mut rendered = String::new();
      for target in targets {
        let wrapper_ref = match target {
          WrappedEsmInitTarget::Module(module_idx) => {
            let target =
              ctx.esm_init_target(*module_idx).expect("entry init target should have a wrapper");
            debug_assert!(!target.tla_tainted);
            target.wrapper_ref
          }
          WrappedEsmInitTarget::CjsCarrier(key) => {
            ctx
              .order_wrap_state
              .order_cjs_carrier(*key)
              .expect("entry CJS carrier should exist")
              .wrapper_ref
          }
        };
        let wrapper_name = ctx.finalized_string_pattern_for_symbol_ref(
          wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );
        let wrapper_name = escape_non_ascii(&wrapper_name, ctx.options.charset.is_ascii());
        writeln!(rendered, "{wrapper_name}();").expect("writing to a string cannot fail");
      }
      return Some(rendered);
    }

    let reexport_init_calls = render_entry_reexported_wrapper_init_calls(ctx, entry_id, entry_meta);

    match ctx.esm_init_target(entry_id) {
      Some(target) => {
        let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
          target.wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );
        let wrapper_ref_name = escape_non_ascii(&wrapper_ref_name, ctx.options.charset.is_ascii());
        let own_init_call = if target.tla_tainted {
          concat_string!("await ", wrapper_ref_name, "();")
        } else {
          concat_string!(wrapper_ref_name, "();")
        };
        match reexport_init_calls {
          Some(mut calls) => {
            calls.push_str(&own_init_call);
            Some(calls)
          }
          None => Some(own_init_call),
        }
      }
      None => reexport_init_calls,
    }
  } else {
    None
  }
}

/// Off-strict, an unwrapped entry can re-export bindings owned by an ESM-wrapped module while
/// every statement that could call its `init_*` was tree-shaken. The re-export chain resolves
/// bindings symbol-to-symbol, but without an init call those bindings read as `undefined` at
/// runtime (issue #10543).
///
/// `register_entry_export_depended_symbols` imports the wrappers from the same collector used
/// here. An entry hosting its own body gets the calls as a body prelude from the finalizer
/// (`entry_reexported_wrapper_init_prelude`); this chunk-tail path serves facade chunks that host
/// no entry body. Wrapped entries initialize their re-export sources through their own wrapper
/// instead, preserving source order even when the sources are hosted in another chunk.
fn render_entry_reexported_wrapper_init_calls(
  ctx: &GenerateContext<'_>,
  entry_id: ModuleIdx,
  entry_meta: &crate::types::linking_metadata::LinkingMetadata,
) -> Option<String> {
  if ctx.options.is_strict_execution_order_enabled() {
    // Strict execution order routes entry initialization through order-wrap lowering
    // (consumer-local targets and entry facades) instead.
    return None;
  }
  if ctx.chunk_graph.module_to_chunk[entry_id] == Some(ctx.chunk_idx) {
    return None;
  }
  let inits = collect_entry_reexported_wrapper_inits(
    entry_id,
    entry_meta,
    &ctx.link_output.metas,
    &ctx.link_output.module_table.modules,
    &ctx.link_output.symbol_db,
    Some(&ctx.chunk.canonical_names),
  );
  let mut rendered = String::new();
  for init in inits {
    let wrapper_name = ctx.finalized_string_pattern_for_symbol_ref(
      init.wrapper_ref,
      ctx.chunk_idx,
      &ctx.chunk.canonical_names,
    );
    let wrapper_name = escape_non_ascii(&wrapper_name, ctx.options.charset.is_ascii());
    if init.tla_tainted {
      // Defensive parity with `wrapped_esm_init_call_expr`'s `await_if_tla`; believed
      // unreachable today. Non-`esm` formats reject top-level await at scan time
      // (`AstScanner::handle_top_level_await`), so this cannot produce an `await` inside a
      // plain iife/umd/cjs factory. Under `esm`, every off-strict `WrapKind::Esm` cause is
      // blocked from combining with TLA: a `require` reaching a TLA subtree is a build error
      // (`REQUIRE_TLA`), and the dynamic-import-with-splitting-disabled cause is single-chunk,
      // where a statement-position or metadata init always covers the module first.
      writeln!(rendered, "await {wrapper_name}();").expect("writing to a string cannot fail");
    } else {
      writeln!(rendered, "{wrapper_name}();").expect("writing to a string cannot fail");
    }
  }
  (!rendered.is_empty()).then_some(rendered)
}

#[expect(clippy::too_many_lines)] // Dispatches over every output format and export mode inline.
pub fn render_chunk_exports(
  ctx: &GenerateContext<'_>,
  export_mode: Option<&OutputExports>,
) -> Option<String> {
  let GenerateContext { chunk, link_output, options, .. } = ctx;
  let ascii_only = options.charset.is_ascii();
  let mut export_items: Vec<(CompactStr, SymbolRef)> = ctx.render_export_items_index_vec
    [ctx.chunk_idx]
    .iter()
    .flat_map(|(symbol_ref, names)| {
      let symbol_ref = *symbol_ref;
      names.iter().map(move |name| (name.clone(), symbol_ref))
    })
    .collect();

  match options.format {
    OutputFormat::Esm => {
      // If this is an entry point with a CJS wrapper, render_wrapped_entry_chunk already handles
      // the default export, so we should filter it out from export_items to avoid duplicates.
      if let ChunkKind::EntryPoint { module: entry_id, .. } = chunk.kind {
        let entry_meta = &link_output.metas[entry_id];
        if matches!(entry_meta.wrap_kind(), WrapKind::Cjs) {
          export_items.retain(|(exported_name, _)| exported_name.as_str() != "default");
        }
      }

      if export_items.is_empty() && !matches!(ctx.options.platform, Platform::Node) {
        return None;
      }
      let mut s = String::new();
      render_shared_read_declarations(&mut s, ctx);
      // The saved value of each CommonJS property (`save_commonjs_property`).
      let mut saved_commonjs_properties = FxHashMap::default();
      // The live re-exports of a marked facade from each external module
      // (`export { a } from 'ext'`).
      let mut external_reexports: FxIndexMap<ModuleIdx, Vec<String>> = FxIndexMap::default();
      let rendered_items = export_items
        .into_iter()
        .filter_map(|(exported_name, export_ref)| {
          let canonical_ref = link_output.symbol_db.canonical_ref_for(export_ref);
          let symbol = link_output.symbol_db.get(canonical_ref);
          // An ESM export needs a binding of this file; a symbol an inline common chunk record
          // owns is reachable only through a bridge, so selection keeps such a record a file.
          assert!(
            symbol
              .chunk_idx
              .is_none_or(|owner| owner == ctx.chunk_idx || !ctx.inline_state.is_record(owner)),
            "chunk {:?} exports `{}`, a symbol of inline common chunk record {:?}",
            ctx.chunk_idx,
            exported_name,
            symbol.chunk_idx
          );
          let mut canonical_name = link_output
            .symbol_db
            .canonical_name_for_or_original(canonical_ref, &chunk.canonical_names);
          let mut rendered_canonical_name = escape_non_ascii(canonical_name, ascii_only);
          if let Some(binding) = ctx.shared_commonjs_reads.binding(ctx.chunk_idx, canonical_ref) {
            // The chunk imports the public export of an entry chunk that reads the property, and
            // exports this import (`SharedCommonJsReads::binding`).
            canonical_name =
              link_output.symbol_db.canonical_name_for_or_original(binding, &chunk.canonical_names);
            rendered_canonical_name = escape_non_ascii(canonical_name, ascii_only);
          } else if let Some(ns_alias) = &symbol.namespace_alias {
            let origin = chunk
              .is_cross_chunk_export_facade
              .then(|| namespace_alias_origin(link_output, ns_alias));
            if let Some(NamespaceAliasOrigin::External(external_idx)) = origin {
              // A bundled barrel gets the binding through `export *` from the external module.
              // The facade re-exports it from that module, so the binding stays live.
              let imported =
                to_module_import_export_name_with_ascii(&ns_alias.property_name, ascii_only);
              let specifier = if ns_alias.property_name == exported_name {
                imported
              } else {
                concat_string!(
                  imported,
                  " as ",
                  to_module_import_export_name_with_ascii(&exported_name, ascii_only)
                )
              };
              external_reexports.entry(external_idx).or_default().push(specifier);
              return None;
            }
            let canonical_ns_name = link_output
              .symbol_db
              .canonical_name_for_or_original(ns_alias.namespace_ref, &chunk.canonical_names);
            let namespace = escape_non_ascii(canonical_ns_name, ascii_only);
            let property_name = &ns_alias.property_name;
            let shared_read = ctx.shared_commonjs_reads.call(ctx.chunk_idx, canonical_ref);
            if shared_read.is_some() || origin.is_some() {
              let read = match shared_read {
                Some(callee) => SavedRead::Shared {
                  callee,
                  call: render_shared_read_call(ctx, callee, &namespace),
                },
                None => SavedRead::Local {
                  namespace_ref: link_output.symbol_db.canonical_ref_for(ns_alias.namespace_ref),
                  namespace: &namespace,
                  property: property_name.as_str(),
                },
              };
              if let Some(saved_name) = save_commonjs_property(
                &mut s,
                &mut saved_commonjs_properties,
                read,
                canonical_name,
                &rendered_canonical_name,
                ascii_only,
              ) {
                canonical_name = saved_name;
                rendered_canonical_name = escape_non_ascii(saved_name, ascii_only);
              }
            } else {
              s.push_str("var ");
              s.push_str(&rendered_canonical_name);
              s.push_str(" = ");
              s.push_str(&property_access_str_with_ascii(&namespace, property_name, ascii_only));
              s.push_str(";\n");
            }
          }

          Some(if canonical_name == exported_name {
            rendered_canonical_name
          } else {
            Cow::Owned(concat_string!(
              rendered_canonical_name,
              " as ",
              to_module_import_export_name_with_ascii(&exported_name, ascii_only)
            ))
          })
        })
        .collect::<Vec<_>>();
      for (external_idx, specifiers) in &external_reexports {
        render_external_reexport(&mut s, ctx, *external_idx, specifiers);
      }
      if rendered_items.is_empty() && !external_reexports.is_empty() {
        return Some(s);
      }
      s.push_str("export { ");
      for (i, item) in rendered_items.iter().enumerate() {
        if i > 0 {
          s.push_str(", ");
        }
        s.push_str(item);
      }
      s.push_str(" };");
      Some(s)
    }
    OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd => {
      let mut s = String::new();
      render_shared_read_declarations(&mut s, ctx);
      match chunk.kind {
        ChunkKind::EntryPoint { module, .. } => {
          let module =
            &link_output.module_table[module].as_normal().expect("should be normal module");
          if !matches!(module.exports_kind, ExportsKind::Esm) {
            export_items.retain(|(_, export_ref)| {
              let canonical_ref = link_output.symbol_db.canonical_ref_for(*export_ref);
              canonical_ref.owner != module.idx
            });
          }
          if !export_items.is_empty() {
            // The saved value of each CommonJS property in a marked facade
            // (`save_commonjs_property`).
            let mut saved_commonjs_properties = FxHashMap::default();
            let rendered_items = export_items
              .into_iter()
              .map(|(exported_name, export_ref)| {
                let canonical_ref = link_output.symbol_db.canonical_ref_for(export_ref);
                let value_pattern = ctx.finalized_string_pattern_for_symbol_ref(
                  canonical_ref,
                  ctx.chunk_idx,
                  &chunk.canonical_names,
                );
                let mut exported_value = escape_non_ascii(&value_pattern, ascii_only);
                // In a marked facade, the origin of a namespace alias decides the export. A
                // CommonJS property gets a saved value. A property of an external module keeps the
                // getter of `external_live_bindings`, and a property of an unknown origin keeps a
                // getter.
                let alias_origin = link_output
                  .symbol_db
                  .get(canonical_ref)
                  .namespace_alias
                  .as_ref()
                  .filter(|_| {
                    chunk.is_cross_chunk_export_facade
                      && matches!(export_mode, Some(OutputExports::Named | OutputExports::Default))
                  })
                  .map(|ns_alias| (ns_alias, namespace_alias_origin(link_output, ns_alias)));
                let binding = ctx.shared_commonjs_reads.binding(ctx.chunk_idx, canonical_ref);
                let commonjs_property = alias_origin.and_then(|(ns_alias, origin)| {
                  matches!(origin, NamespaceAliasOrigin::CommonJs(_)).then_some(ns_alias)
                });
                let is_saved_value = commonjs_property.is_some() || binding.is_some();
                let shared_read = ctx.shared_commonjs_reads.call(ctx.chunk_idx, canonical_ref);
                if let Some(binding) = binding {
                  // The chunk reads the public export of an entry chunk. The getter of that export
                  // saves its first read, so this export is a saved value too
                  // (`SharedCommonJsReads::binding`).
                  exported_value = Cow::Owned(
                    escape_non_ascii(
                      &ctx.finalized_binding_pattern(
                        binding,
                        ctx.chunk_idx,
                        &chunk.canonical_names,
                      ),
                      ascii_only,
                    )
                    .into_owned(),
                  );
                } else if let Some(ns_alias) = commonjs_property {
                  let name = link_output
                    .symbol_db
                    .canonical_name_for_or_original(canonical_ref, &chunk.canonical_names);
                  let rendered_name = escape_non_ascii(name, ascii_only);
                  let namespace = escape_non_ascii(
                    &ctx.finalized_namespace_pattern(
                      ns_alias,
                      ctx.chunk_idx,
                      &chunk.canonical_names,
                    ),
                    ascii_only,
                  )
                  .into_owned();
                  let read = match shared_read {
                    Some(callee) => SavedRead::Shared {
                      callee,
                      call: render_shared_read_call(ctx, callee, &namespace),
                    },
                    None => SavedRead::Local {
                      namespace_ref: link_output
                        .symbol_db
                        .canonical_ref_for(ns_alias.namespace_ref),
                      namespace: &namespace,
                      property: ns_alias.property_name.as_str(),
                    },
                  };
                  exported_value = match save_commonjs_property(
                    &mut s,
                    &mut saved_commonjs_properties,
                    read,
                    name,
                    &rendered_name,
                    ascii_only,
                  ) {
                    Some(saved_name) => escape_non_ascii(saved_name, ascii_only),
                    None => rendered_name,
                  };
                } else if let Some(callee) = shared_read {
                  // An export that `share_commonjs_property_reads` gives a shared read keeps the
                  // getter or the value of its chunk. Only the read changes.
                  exported_value =
                    Cow::Owned(shared_read_call_for_export(ctx, callee, canonical_ref));
                }

                match export_mode {
                  Some(OutputExports::Named) => {
                    // A strict-execution-order wrapper (`init_*`) self-rebinds on first call
                    // (`function init_x() { return (init_x = __esmMin(cb))() }`), so any
                    // cross-chunk export of it must expose a *live* getter. A value snapshot
                    // (`exports.init_x = init_x`) would freeze the pre-rebind function and re-run
                    // the module body on every later call. On current inputs wrapper refs only
                    // render through the common-chunk arm below (an entry chunk holding an
                    // order-wrapped module is split into a facade), which is unconditionally a
                    // getter; this entry-chunk pin is defense-in-depth so no future topology or
                    // optimization can route a wrapper into the value-snapshot branch.
                    let is_order_wrapper_ref =
                      ctx.order_wrap_state.is_execution_order_wrapper_ref(canonical_ref);
                    let keep_live_binding = match alias_origin {
                      Some((_, NamespaceAliasOrigin::External(_))) => {
                        options.external_live_bindings
                      }
                      Some((_, NamespaceAliasOrigin::Unknown)) => true,
                      _ => must_keep_live_binding(
                        export_ref,
                        &link_output.symbol_db,
                        options,
                        &link_output.module_table.modules,
                      ),
                    };
                    // Today `must_keep_live_binding` keeps wrapper refs live on its own, but only
                    // because the facade ref happens to lack `IsNotReassigned` — an emergent
                    // property, not a guaranteed one. If it ever breaks (e.g. a single-assignment
                    // optimization starts flagging facades), fail loudly in debug builds instead
                    // of silently leaning on the explicit `is_order_wrapper_ref` disjunct below.
                    debug_assert!(
                      !is_order_wrapper_ref || keep_live_binding,
                      "a strict-execution-order wrapper ref must never take the value-snapshot \
                       export branch (`exports.{exported_name} = {exported_value}`): it would \
                       freeze the pre-rebind function and re-run the module body",
                    );
                    // A saved value does not change. Thus its export needs no getter.
                    if !is_saved_value && (is_order_wrapper_ref || keep_live_binding) {
                      render_object_define_property(&exported_name, &exported_value, ascii_only)
                    } else if exported_name.as_str() == "__proto__" {
                      // `__proto__` has special semantics - assigning to it sets the prototype
                      // instead of creating a property. We must use Object.defineProperty to
                      // set an actual property named "__proto__".
                      render_object_define_property_value(
                        &exported_name,
                        &exported_value,
                        ascii_only,
                      )
                    } else {
                      concat_string!(
                        property_access_str_with_ascii(
                          "exports",
                          exported_name.as_str(),
                          ascii_only
                        ),
                        " = ",
                        exported_value.as_ref(),
                        ";"
                      )
                    }
                  }
                  Some(OutputExports::Default) => {
                    if matches!(options.format, OutputFormat::Cjs) {
                      concat_string!("module.exports = ", exported_value.as_ref(), ";")
                    } else {
                      concat_string!("return ", exported_value.as_ref(), ";")
                    }
                  }
                  Some(OutputExports::None) => String::new(),
                  _ => unreachable!(),
                }
              })
              .collect::<Vec<_>>();
            s.push_str(&rendered_items.join("\n"));
          }

          let meta = &ctx.link_output.metas[module.idx];
          let external_modules = meta
            .star_exports_from_external_modules
            .iter()
            .filter_map(|rec_idx| module.ecma_view.import_records[*rec_idx].resolved_module)
            .chain(ctx.chunk.entry_level_external_module_idx.iter().copied())
            .collect::<FxIndexSet<ModuleIdx>>();

          // Track already imported external modules to avoid duplicates
          // First check if any of these external modules have already been imported elsewhere in the chunk
          let mut imported_external_modules: FxHashSet<SymbolRef> = ctx
            .chunk
            .direct_imports_from_external_modules
            .iter()
            .map(|(idx, _)| {
              let external = &ctx.link_output.module_table[*idx]
                .as_external()
                .expect("Should be external module here");
              external.namespace_ref
            })
            .chain(ctx.chunk.import_symbol_from_external_modules.iter().map(|idx| {
              let external = &ctx.link_output.module_table[*idx]
                .as_external()
                .expect("Should be external module here");
              external.namespace_ref
            }))
            .collect();
          external_modules.iter().for_each(|idx| {
          let external = &ctx.link_output.module_table[*idx].as_external().expect("Should be external module here");
          let binding_ref_name = ctx
            .link_output
            .symbol_db
            .canonical_name_for_or_original(external.namespace_ref, &ctx.chunk.canonical_names);
          let binding_ref_name = escape_non_ascii(binding_ref_name, ascii_only);
          let import_stmt = if ctx.options.external_live_bindings {
            concat_string!(
              "Object.keys(",binding_ref_name, ").forEach(function (k) {\n",
              "  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) Object.defineProperty(exports, k, {\n",
              "    enumerable: true,\n",
              "    get: function () { return ",binding_ref_name,"[k]; }\n",
              "  });\n",
              "});\n"
            )
          } else {
            // Assigning `exports['__proto__']` would replace the prototype instead of adding an export.
            concat_string!(
              "Object.prototype.hasOwnProperty.call(",binding_ref_name, ", '__proto__') &&\n",
              "  !Object.prototype.hasOwnProperty.call(exports, '__proto__') &&\n",
              "  Object.defineProperty(exports, '__proto__', { enumerable: true, value: ",binding_ref_name,"['__proto__'] });\n",
              "Object.keys(",binding_ref_name, ").forEach(function (k) {\n",
              "  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) exports[k] = ",binding_ref_name,"[k];\n",
              "});\n"
            )
          };

          s.push('\n');
          // Only generate require statement if this external module hasn't been imported yet
          if imported_external_modules.insert(external.namespace_ref) {
            let import_path = to_ecma_string_literal(
              &external.get_import_path(chunk, ctx.resolved_paths),
              ascii_only,
            );
            writeln!(s, "var {binding_ref_name} = require({import_path});").unwrap();
          }
          s.push_str(&import_stmt);
        });
        }
        ChunkKind::Common => {
          let rendered_items = export_items
            .into_iter()
            .map(|(exported_name, export_ref)| {
              let canonical_ref = link_output.symbol_db.canonical_ref_for(export_ref);
              let exported_value =
                match ctx.shared_commonjs_reads.call(ctx.chunk_idx, canonical_ref) {
                  Some(callee) => shared_read_call_for_export(ctx, callee, canonical_ref),
                  None => ctx.finalized_string_pattern_for_symbol_ref(
                    export_ref,
                    ctx.chunk_idx,
                    &chunk.canonical_names,
                  ),
                };
              render_object_define_property(
                &exported_name,
                &escape_non_ascii(&exported_value, ascii_only),
                ascii_only,
              )
            })
            .collect::<Vec<_>>();
          s.push_str(&rendered_items.join("\n"));
        }
      }

      if s.is_empty() {
        return None;
      }
      Some(s)
    }
  }
}

#[inline]
pub fn render_object_define_property(key: &str, value: &str, ascii_only: bool) -> String {
  let key = to_ecma_string_literal(key, ascii_only);
  concat_string!(
    "Object.defineProperty(exports, ",
    key,
    ", {
  enumerable: true,
  get: function () {
    return ",
    value,
    ";
  }
});"
  )
}

#[inline]
pub fn render_object_define_property_value(key: &str, value: &str, ascii_only: bool) -> String {
  let key = to_ecma_string_literal(key, ascii_only);
  concat_string!(
    "Object.defineProperty(exports, ",
    key,
    ", {
  enumerable: true,
  value: ",
    value,
    "
});"
  )
}

/// The read that `save_commonjs_property` saves.
enum SavedRead<'a, 'name> {
  /// A call of the function that `share_commonjs_property_reads` gives to the export.
  Shared { callee: SymbolRef, call: String },
  /// A read of the property in the chunk.
  Local { namespace_ref: SymbolRef, namespace: &'a str, property: &'name str },
}

/// The key of a saved read in a chunk: the function of a shared read, or the canonical namespace
/// and the property name.
#[derive(PartialEq, Eq, Hash)]
enum SavedReadKey<'a> {
  Shared(SymbolRef),
  Local(SymbolRef, &'a str),
}

/// Saves the value of a CommonJS property in a facade that `Chunk::is_cross_chunk_export_facade`
/// marks, or in a chunk that `share_commonjs_property_reads` gives a shared read. For the first
/// export of a read, the function writes the read to `s` and returns `None`. For the other exports
/// of the read, it returns the variable of the first export. Thus all names of the property
/// export the value of one read.
///
/// - A shared read writes `var name = read(ns);`.
/// - A read in the chunk writes `var name = ns; try { name = name.property; } catch (e) { name =
///   void 0; }`. The namespace is resolved before the `try`, and only the property read can throw
///   into the `catch`.
///
/// Without the cross-chunk export, the group chunk replaces the facade, and no file reads the
/// property when it loads. Node reads each export of a CommonJS module one time when it imports
/// the module. If the read throws, the export is `undefined`. See
/// internal-docs/code-splitting/implementation.md.
fn save_commonjs_property<'name>(
  s: &mut String,
  saved: &mut FxHashMap<SavedReadKey<'name>, &'name str>,
  read: SavedRead<'_, 'name>,
  name: &'name str,
  rendered_name: &str,
  ascii_only: bool,
) -> Option<&'name str> {
  let key = match &read {
    SavedRead::Shared { callee, .. } => SavedReadKey::Shared(*callee),
    SavedRead::Local { namespace_ref, property, .. } => {
      SavedReadKey::Local(*namespace_ref, property)
    }
  };
  match saved.entry(key) {
    Entry::Occupied(first) => Some(*first.get()),
    Entry::Vacant(entry) => {
      entry.insert(name);
      match read {
        SavedRead::Shared { call, .. } => {
          s.push_str("var ");
          s.push_str(rendered_name);
          s.push_str(" = ");
          s.push_str(&call);
          s.push_str(";\n");
        }
        SavedRead::Local { namespace, property, .. } => {
          // The `catch` parameter must not hide the variable.
          let error = if rendered_name == "e" { "error" } else { "e" };
          s.push_str("var ");
          s.push_str(rendered_name);
          s.push_str(" = ");
          s.push_str(namespace);
          s.push_str(";\ntry {\n\t");
          s.push_str(rendered_name);
          s.push_str(" = ");
          s.push_str(&property_access_str_with_ascii(rendered_name, property, ascii_only));
          s.push_str(";\n} catch (");
          s.push_str(error);
          s.push_str(") {\n\t");
          s.push_str(rendered_name);
          s.push_str(" = void 0;\n}\n");
        }
      }
      None
    }
  }
}

/// `read(ns)`: the call of the function `callee` of a shared read with the namespace `namespace`,
/// as the chunk of `ctx` renders it.
fn render_shared_read_call(
  ctx: &GenerateContext<'_>,
  callee: SymbolRef,
  namespace: &str,
) -> String {
  let callee =
    ctx.finalized_string_pattern_for_symbol_ref(callee, ctx.chunk_idx, &ctx.chunk.canonical_names);
  concat_string!(escape_non_ascii(&callee, ctx.options.charset.is_ascii()), "(", namespace, ")")
}

/// The shared read of the export `export_ref`, which is a namespace alias.
fn shared_read_call_for_export(
  ctx: &GenerateContext<'_>,
  callee: SymbolRef,
  export_ref: SymbolRef,
) -> String {
  let ns_alias = ctx
    .link_output
    .symbol_db
    .get(export_ref)
    .namespace_alias
    .as_ref()
    .expect("an export with a shared read is a namespace alias");
  let namespace =
    ctx.finalized_namespace_pattern(ns_alias, ctx.chunk_idx, &ctx.chunk.canonical_names);
  render_shared_read_call(
    ctx,
    callee,
    &escape_non_ascii(&namespace, ctx.options.charset.is_ascii()),
  )
}

/// Writes the functions of the shared reads that the chunk of `ctx` declares
/// (`SharedReadDeclaration`).
fn render_shared_read_declarations(s: &mut String, ctx: &GenerateContext<'_>) {
  let ascii_only = ctx.options.charset.is_ascii();
  let name_of = |symbol_ref: SymbolRef| {
    escape_non_ascii(
      &ctx.finalized_string_pattern_for_symbol_ref(
        symbol_ref,
        ctx.chunk_idx,
        &ctx.chunk.canonical_names,
      ),
      ascii_only,
    )
    .into_owned()
  };
  for declaration in ctx.shared_commonjs_reads.declarations(ctx.chunk_idx) {
    match declaration {
      SharedReadDeclaration::Accessor { accessor, value, is_read, property } => {
        let (accessor, value, is_read) = (name_of(*accessor), name_of(*value), name_of(*is_read));
        writeln!(
          s,
          "var {value}, {is_read};\nfunction {accessor}(ns) {{\n\tif (!{is_read}) {{\n\t\ttry {{\n\t\t\t{value} = {read};\n\t\t}} catch (e) {{\n\t\t\tif (ns == null) throw e;\n\t\t}}\n\t\t{is_read} = true;\n\t}}\n\treturn {value};\n}}",
          read = property_access_str_with_ascii("ns", property, ascii_only),
        )
        .expect("writing to a string cannot fail");
      }
      SharedReadDeclaration::Forward { forward, next } => {
        writeln!(s, "function {}(ns) {{\n\treturn {}(ns);\n}}", name_of(*forward), name_of(*next))
          .expect("writing to a string cannot fail");
      }
    }
  }
}

/// Writes `export { a, b as c } from "ext";` to `s`. A facade that
/// `Chunk::is_cross_chunk_export_facade` marks re-exports live bindings of the external module
/// `external_idx` in this way (`NamespaceAliasOrigin::External`). The chunk lists the module in
/// `static_imports`.
fn render_external_reexport(
  s: &mut String,
  ctx: &GenerateContext<'_>,
  external_idx: ModuleIdx,
  specifiers: &[String],
) {
  let ascii_only = ctx.options.charset.is_ascii();
  let external =
    ctx.link_output.module_table[external_idx].as_external().expect("should be external module");
  s.push_str("export { ");
  s.push_str(&specifiers.join(", "));
  s.push_str(" } from ");
  s.push_str(&to_ecma_string_literal(
    &external.get_import_path(ctx.chunk, ctx.resolved_paths),
    ascii_only,
  ));
  if let Some(with_clause) = external.attributes.to_with_clause(ascii_only) {
    s.push(' ');
    s.push_str(&with_clause);
  }
  s.push_str(";\n");
}

pub fn get_export_items(chunk: &Chunk) -> Vec<(CompactStr, SymbolRef)> {
  let mut export_items = chunk
    .exports_to_other_chunks
    .iter()
    .flat_map(|(export_ref, alias_list)| {
      alias_list.iter().map(|alias| (alias.clone(), *export_ref))
    })
    .collect::<Vec<_>>();

  export_items.sort_unstable_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

  export_items
}

pub fn get_chunk_export_names(chunk: &Chunk, graph: &LinkStageOutput) -> Vec<CompactStr> {
  if let ChunkKind::EntryPoint { module: entry_id, .. } = &chunk.kind {
    let entry_meta = &graph.metas[*entry_id];
    if matches!(entry_meta.wrap_kind(), WrapKind::Cjs) {
      return vec![CompactStr::new("default")];
    }
  }

  get_export_items(chunk).into_iter().map(|(exported_name, _)| exported_name).collect::<Vec<_>>()
}

pub fn get_chunk_export_names_with_ctx(ctx: &GenerateContext<'_>) -> Vec<CompactStr> {
  let GenerateContext { chunk, link_output, render_export_items_index_vec, .. } = ctx;
  if let ChunkKind::EntryPoint { module: entry_id, .. } = &chunk.kind {
    let entry_meta = &link_output.metas[*entry_id];
    if matches!(entry_meta.wrap_kind(), WrapKind::Cjs) {
      return vec![CompactStr::new("default")];
    }
  }
  render_export_items_index_vec[ctx.chunk_idx].values().flatten().cloned().collect::<Vec<_>>()
}

fn must_keep_live_binding(
  export_ref: SymbolRef,
  symbol_db: &SymbolRefDb,
  options: &NormalizedBundlerOptions,
  modules: &IndexModules,
) -> bool {
  let canonical_ref = symbol_db.canonical_ref_for(export_ref);

  // Local write analysis cannot prove an external export is immutable.
  if canonical_ref.is_created_by_import_stmt_that_target_external(symbol_db, modules) {
    return options.external_live_bindings;
  }

  if canonical_ref.is_declared_by_const(symbol_db) {
    return false;
  }

  // Unlike `is_not_reassigned`, this ignores `ReadBeforeAssigned`: a read before the declaration
  // assigns the binding does not change the value the export ends up with. `IsNotReassigned` does
  // not see an assignment inside direct `eval`. For unknown case, we consider it as reassigned.
  if canonical_ref
    .flags(symbol_db)
    .is_some_and(|flags| flags.contains(SymbolRefFlags::IsNotReassigned))
    && !eval_can_assign(symbol_db.local_db(canonical_ref.owner).scoping(), canonical_ref.symbol)
  {
    return false;
  }

  true
}
