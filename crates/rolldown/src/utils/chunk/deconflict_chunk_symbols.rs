use oxc_str::CompactStr;

use crate::{
  stages::{
    generate_stage::{FileInlineNames, order_wrap_state::OrderWrapState},
    link_stage::LinkStageOutput,
  },
  utils::{
    external_import_interop::{
      ChunkAssignments, chunk_external_interop_modes, chunk_has_node_esm_reader,
    },
    renamer::{
      InnerBindingNames, NestedScopeRenamer, Renamer, RootBindingKind, cjs_wrapper_fixed_params,
      fixed_names_in_module_body,
    },
  },
};
use arcstr::ArcStr;
use rolldown_common::{
  Chunk, ChunkIdx, ChunkKind, GetLocalDb, ImportKind, ModuleIdx, NormalModule, OutputFormat,
  StmtInfo, SymbolRef, WrapKind,
};
use rolldown_utils::{concat_string, ecmascript::legitimize_identifier_name};
use rustc_hash::FxHashMap;

/// What `experimentalInlineCommonChunks` adds to one file's naming. The modules and synthetic
/// statements of every record the file carries are named in the file's namespace, because their
/// factories are printed into it; the file also declares one bridge per record it reads, one per
/// record each carried factory reads, and the factories' `exports` parameter. See
/// internal-docs/inline-common-chunks/implementation.md ("Deconflicting").
#[derive(Debug)]
pub struct InlineNamingInput {
  /// The file's own modules and every carried record's, in ascending execution order.
  pub modules: Vec<ModuleIdx>,
  /// The file's chunk and every carried record.
  pub chunks: Vec<ChunkIdx>,
  /// Records the file reads directly, with the chunk name the bridge is named after.
  pub file_bridges: Vec<(ChunkIdx, ArcStr)>,
  /// Carried records, each with the records its factory reads.
  pub factories: Vec<(ChunkIdx, Vec<(ChunkIdx, ArcStr)>)>,
}

/// Give names to the bindings of one chunk. See internal-docs/renaming/implementation.md.
///
/// `inline` is `Some` for a file that reads the records of inline common chunks (see
/// [`InlineNamingInput`]).
#[tracing::instrument(level = "trace", skip_all)]
#[expect(clippy::too_many_arguments)]
pub fn deconflict_chunk_symbols(
  chunk_idx: ChunkIdx,
  chunk: &mut Chunk,
  link_output: &LinkStageOutput,
  order_wrap_state: &OrderWrapState,
  format: OutputFormat,
  index_chunk_id_to_name: &FxHashMap<ChunkIdx, ArcStr>,
  chunk_assignments: ChunkAssignments<'_>,
  inline: Option<&InlineNamingInput>,
) -> Option<FileInlineNames> {
  // The modules and chunks whose root-scope declarations land in this file.
  let own_chunk = [chunk_idx];
  let (modules, chunks): (&[ModuleIdx], &[ChunkIdx]) = match inline {
    Some(inline) => (&inline.modules, &inline.chunks),
    None => (&chunk.modules, &own_chunk),
  };
  let inner_binding_names = InnerBindingNames::new(
    modules
      .iter()
      .filter_map(|&idx| {
        let scoping = link_output.symbol_db[idx].as_ref()?.ast_scopes.scoping();
        Some((scoping, matches!(link_output.metas[idx].wrap_kind(), WrapKind::Cjs)))
      })
      .collect(),
  );
  let mut renamer = Renamer::new(&link_output.symbol_db, format, inner_binding_names);
  reserve_fixed_names(&mut renamer, modules, link_output, format);

  if matches!(format, OutputFormat::Iife | OutputFormat::Umd | OutputFormat::Cjs) {
    // deconflict iife introduce symbols by external
    // Also AMD, but we don't support them yet.
    chunk
      .direct_imports_from_external_modules
      .iter()
      .map(|(idx, _)| *idx)
      .chain(chunk.entry_level_external_module_idx.iter().copied())
      .filter_map(|idx| link_output.module_table[idx].as_external())
      .for_each(|external_module| {
        renamer.add_symbol_in_root_scope(external_module.namespace_ref, RootBindingKind::Authored);
      });

    chunk
      .import_symbol_from_external_modules
      .iter()
      .filter_map(|idx| link_output.module_table[*idx].as_external())
      .for_each(|external_module| {
        renamer.add_symbol_in_root_scope(external_module.namespace_ref, RootBindingKind::Authored);
      });
  }

  match chunk.kind {
    ChunkKind::EntryPoint { module, .. } => {
      let meta = &link_output.metas[module];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(
        |(symbol_ref, came_from_cjs)| {
          if !came_from_cjs {
            renamer
              .add_symbol_in_root_scope(*symbol_ref, root_binding_kind(link_output, *symbol_ref));
          }
        },
      );
    }
    ChunkKind::Common => {}
  }
  if matches!(format, OutputFormat::Esm) {
    chunk.direct_imports_from_external_modules.iter().for_each(|(module, _)| {
      let db = link_output.symbol_db.local_db(*module);
      db.classic_data.iter_enumerated().for_each(|(symbol, _)| {
        let symbol_ref = (*module, symbol).into();
        if link_output.used_external_symbols.contains(&symbol_ref) {
          renamer.add_symbol_in_root_scope(symbol_ref, RootBindingKind::Authored);
        }
      });
    });
  }

  // The renamer relies on `modules` being in ascending exec_order so that
  // `.rev()` yields entry-first / descending exec_order — the same priority as
  // `deconflict_order_key`. Enforce that invariant in debug builds (was only a
  // prose + pinned-SHA comment before).
  debug_assert!(
    modules
      .iter()
      .filter_map(|idx| link_output.module_table[*idx].as_normal().map(|m| m.exec_order))
      .is_sorted(),
    "modules must be in ascending exec_order for deconfliction"
  );

  modules
    .iter()
    .copied()
    // Starts with entry module
    .rev()
    .filter_map(|id| link_output.module_table[id].as_normal())
    .for_each(|module| {
      if let Some(hmr_hot_ref) = module.hmr_hot_ref {
        renamer.add_symbol_in_root_scope(hmr_hot_ref, RootBindingKind::Synthesized);
      }
      // A CJS-wrapped module's top-level statements are printed inside its `__commonJS` closure, so
      // the bindings they declare are inner bindings, not chunk-level ones: they keep their names
      // unless they would capture a reference (`rename_shadowing_symbols_in_nested_scopes`).
      let meta = &link_output.metas[module.idx];
      let is_cjs_wrapped_module = matches!(meta.wrap_kind(), WrapKind::Cjs);
      // A parameter of the CJS closure, named like a top-level binding so that no inner binding
      // and no other top-level binding uses its name.
      if let Some(this_ref) = module.cjs_this_ref
        && is_cjs_wrapped_module
      {
        renamer.add_symbol_in_root_scope(this_ref, RootBindingKind::Synthesized);
      }

      link_output.stmt_infos[module.idx]
        .iter_enumerated()
        // A runtime statement tree-shaking excluded but order wrapping force-includes is rendered
        // and symbol-assigned, so it must reach the renamer too. Mirror the overlay-aware inclusion
        // test the other two consumers already use (`compute_cross_chunk_links` and the module
        // finalizer's `remove_unused_top_level_stmt`); without it a user top-level binding named
        // `__esmMin`/`__esm` co-hosted with the runtime collides with the forced helper declaration.
        .filter(|(idx, stmt_info)| {
          (meta.stmt_info_included.has_bit(*idx)
            || order_wrap_state.forces_runtime_stmt(&link_output.runtime, module.idx, stmt_info))
            && !stmt_info.import_records.iter().any(|rec_idx| {
              order_wrap_state.has_order_cjs_carrier(
                crate::stages::generate_stage::order_wrap_state::OrderCjsCarrierKey {
                  importer: module.idx,
                  record: *rec_idx,
                },
              )
            })
        })
        .for_each(|(_, stmt_info)| {
          for declared_symbol in stmt_info.declared_symbols.iter().filter(|item| item.is_normal()) {
            let symbol_ref = declared_symbol.inner();
            let canonical_ref = link_output.symbol_db.canonical_ref_for(symbol_ref);
            // Import statement declared some symbols that come from other module, those symbol should be skipped
            if canonical_ref.owner != module.idx {
              continue;
            }
            // In a CJS-wrapped module only facade symbols (the `require_foo` wrapper, namespace
            // objects, ...) are printed at the chunk's top level. Since we merge external module
            // symbols, an external import binding declared in a CJS module is a top-level one too.
            // Every other symbol is a closure-local, renamed only if it would capture a reference.
            if is_cjs_wrapped_module
              && !link_output.symbol_db.is_facade_symbol(canonical_ref)
              && !is_external_import_declaration(link_output, module, stmt_info)
            {
              continue;
            }
            renamer
              .add_symbol_in_root_scope(symbol_ref, root_binding_kind(link_output, canonical_ref));
          }
        });
    });

  for synthetic in
    chunks.iter().flat_map(|idx| order_wrap_state.synthetic_statements_for_chunk(*idx))
  {
    for declared_symbol in synthetic.declared_symbols.iter().filter(|item| item.is_normal()) {
      renamer.add_symbol_in_root_scope(declared_symbol.inner(), RootBindingKind::Synthesized);
    }
  }

  // Though, those symbols in `imports_from_other_chunks` doesn't belong to this chunk, but in the final output, they still behave
  // like declared in this chunk. This is because we need to generate import statements in this chunk to import symbols from other
  // statements. Those `import {...} from './other-chunk.js'` will declared these outside symbols in this chunk, so symbols that
  // point to them can be resolved in runtime.
  // So we add them in the deconflict process to generate conflict-less names in this chunk.
  chunk.imports_from_other_chunks.iter().flat_map(|(_, items)| items.iter()).for_each(|item| {
    renamer
      .add_symbol_in_root_scope(item.import_ref, root_binding_kind(link_output, item.import_ref));
  });

  chunk.require_binding_names_for_other_chunks = chunk
    .imports_from_other_chunks
    .iter()
    .map(|(id, _)| {
      (
        *id,
        renamer
          .create_conflictless_name(&legitimize_identifier_name(&concat_string!(
            "require_",
            index_chunk_id_to_name[id]
          )))
          .to_string(),
      )
    })
    .collect();

  // Detect mixed-mode external imports: both ESM (node-mode) and non-ESM importers
  // needing interop on the same external. Create a separate binding name for node-mode.
  if matches!(format, OutputFormat::Iife | OutputFormat::Umd | OutputFormat::Cjs) {
    let mut node_mode_names = FxHashMap::default();
    // Externals the chunk only *references* (their importing module lives in another chunk or was
    // tree-shaken away) carry no `named_imports`, but the inclusion pass still recorded how they
    // are observed — so they can be mixed-mode too. See `chunk_recorded_external_interop`.
    //
    // Only the cjs renderer emits bindings for that indirect list; `render_chunk_external_imports`
    // walks the direct list alone, so a name planned from an indirect external under iife/umd would
    // have no `let` to bind it. Keep the two in step rather than plan a name nothing declares.
    let indirect_externals = matches!(format, OutputFormat::Cjs)
      .then(|| chunk.import_symbol_from_external_modules.iter())
      .into_iter()
      .flatten();
    let externals = chunk
      .direct_imports_from_external_modules
      .iter()
      .map(|(ext_idx, named_imports)| (*ext_idx, Some(named_imports.as_slice())))
      .chain(indirect_externals.map(|ext_idx| (*ext_idx, None)));
    for (ext_idx, named_imports) in externals {
      let ext =
        link_output.module_table[ext_idx].as_external().expect("Should be external module here");
      let Some(modes) = chunk_external_interop_modes(
        link_output,
        chunk_assignments,
        chunk_idx,
        ext.namespace_ref,
        named_imports,
      ) else {
        continue;
      };
      // Both modes are needed *and* some module here will actually read the node one. Without the
      // second test the binding is dead on arrival and only its `__toESM(mod, 1)` call survives DCE.
      if modes.node_esm
        && modes.non_node_esm
        && chunk_has_node_esm_reader(
          link_output,
          chunk_assignments,
          chunk_idx,
          ext.namespace_ref,
          named_imports,
        )
      {
        let canonical_ref = link_output.symbol_db.canonical_ref_for(ext.namespace_ref);
        let original_name = canonical_ref.name(&link_output.symbol_db);
        let node_name = renamer.create_conflictless_name(original_name);
        node_mode_names.insert(canonical_ref, node_name);
      }
    }
    chunk.node_mode_external_ns_names = node_mode_names;
  }

  let inline_names = inline.map(|inline| name_inline_bindings(&mut renamer, inline));

  rename_shadowing_symbols_in_nested_scopes(modules, link_output, format, &mut renamer);

  chunk.canonical_names = renamer.into_canonical_names();
  inline_names
}

/// Reserve the names that no top-level binding can take:
/// - each unresolved reference of the modules of the chunk (for example `console` and `window`),
///   because these references must resolve to globals;
/// - the fixed names that the bodies of the modules print (`fixed_names_in_module_body`), because
///   the body of an ESM module is at the top level;
/// - the fixed parameters of their CJS closures (`cjs_wrapper_fixed_params`), because a parameter
///   would capture a reference to a top-level binding with its name.
fn reserve_fixed_names(
  renamer: &mut Renamer<'_>,
  modules: &[ModuleIdx],
  link_output: &LinkStageOutput,
  format: OutputFormat,
) {
  modules
    .iter()
    .copied()
    .filter_map(|idx| {
      Some(
        link_output.symbol_db[idx]
          .as_ref()?
          .ast_scopes
          .scoping()
          .root_unresolved_references()
          .keys(),
      )
    })
    .flatten()
    .for_each(|name| {
      renamer.reserve(CompactStr::new(name));
    });
  modules.iter().filter_map(|&idx| link_output.module_table[idx].as_normal()).for_each(|module| {
    let wrapper_params = if matches!(link_output.metas[module.idx].wrap_kind(), WrapKind::Cjs) {
      cjs_wrapper_fixed_params(module)
    } else {
      &[]
    };
    for name in fixed_names_in_module_body(module, link_output, format)
      .into_iter()
      .chain(wrapper_params.iter().copied())
    {
      renamer.reserve(CompactStr::new_const(name));
    }
  });
}

/// Whether `stmt_info` is an `import` declaration of an external module. In a CJS-wrapped module,
/// rolldown moves that declaration out of the CJS closure. A `require('external')` initializer
/// stays in the closure, and its binding stays a local of the closure.
fn is_external_import_declaration(
  link_output: &LinkStageOutput,
  module: &NormalModule,
  stmt_info: &StmtInfo,
) -> bool {
  stmt_info.import_records.iter().any(|import_rec_idx| {
    let import_record = &module.import_records[*import_rec_idx];
    import_record.kind == ImportKind::Import
      && import_record
        .resolved_module
        .is_some_and(|module_idx| link_output.module_table[module_idx].is_external())
  })
}

/// Returns the kind of a top-level symbol. A symbol is synthesized if it is a runtime helper, or a
/// facade that a normal module owns (for example a wrapper binding, a namespace object or an
/// interop namespace). The finalizer prints references to these symbols at any location in the
/// body of a module.
///
/// A facade is authored if it represents a binding with a source name: a re-export binding, the
/// default export binding, or a shim for a missing export. Only the references of the source read
/// these facades, and `rename_shadowing_symbols_in_nested_scopes` checks those references at their
/// locations. The facades of an external module are also authored, because their names come from
/// the source that imports them.
fn root_binding_kind(link_output: &LinkStageOutput, symbol_ref: SymbolRef) -> RootBindingKind {
  let canonical_ref = link_output.symbol_db.canonical_ref_for(symbol_ref);
  if canonical_ref.owner == link_output.runtime.id() {
    return RootBindingKind::Synthesized;
  }
  let Some(module) = link_output.module_table[canonical_ref.owner].as_normal() else {
    return RootBindingKind::Authored;
  };
  let is_synthesized = link_output.symbol_db.is_facade_symbol(canonical_ref)
    && !module.named_imports.contains_key(&canonical_ref)
    && module.default_export_ref != canonical_ref
    && !link_output.metas[module.idx]
      .shimmed_missing_exports
      .values()
      .any(|shim| *shim == canonical_ref);
  if is_synthesized { RootBindingKind::Synthesized } else { RootBindingKind::Authored }
}

/// A bridge is read wherever a record symbol was referenced, including inside nested scopes, so
/// its name must be free in every nested scope of the file's modules, like an import binding.
/// The factories' `exports` parameter avoids everything the file declares and reads.
fn name_inline_bindings(renamer: &mut Renamer<'_>, inline: &InlineNamingInput) -> FileInlineNames {
  let bridges = inline
    .file_bridges
    .iter()
    .map(|(record, name)| (*record, bridge_name(renamer, name)))
    .collect();
  let factory_bridges = inline
    .factories
    .iter()
    .map(|(record, reads)| {
      let names = reads.iter().map(|(other, name)| (*other, bridge_name(renamer, name))).collect();
      (*record, names)
    })
    .collect();
  let exports_param = if inline.factories.is_empty() {
    CompactStr::new_const("exports")
  } else {
    renamer.create_conflictless_name("exports")
  };
  FileInlineNames { exports_param, bridges, factory_bridges }
}

/// A bridge is a synthesized binding. Thus, like each synthesized binding, it avoids the names of
/// the inner bindings of the file. These include the root bindings of CJS-wrapped modules, because
/// the finalizer prints them inside their CJS closures.
fn bridge_name(renamer: &mut Renamer<'_>, chunk_name: &str) -> CompactStr {
  let hint_source = concat_string!("share_", chunk_name);
  let hint = legitimize_identifier_name(&hint_source);
  renamer.create_conflictless_name(&hint)
}

/// Rename the inner bindings that would capture a reference to a top-level symbol. Inner bindings
/// are the nested-scope bindings, and the root-scope bindings of CJS-wrapped modules.
///
/// An inner binding keeps its original name unless it would capture a reference. Synthesized
/// top-level bindings never have the name of an inner binding. Thus this function checks only the
/// references that the source contains. See internal-docs/renaming/design.md.
fn rename_shadowing_symbols_in_nested_scopes<'a>(
  modules: &[ModuleIdx],
  link_output: &'a LinkStageOutput,
  output_format: OutputFormat,
  renamer: &mut Renamer<'a>,
) {
  // Same as above, starts with entry module to give entry module symbols naming priority.
  for module_idx in modules.iter().copied().rev() {
    let Some(module) = link_output.module_table[module_idx].as_normal() else {
      continue;
    };
    let Some(db) = &link_output.symbol_db[module_idx] else {
      continue;
    };

    let mut ctx = NestedScopeRenamer {
      module_idx,
      module,
      db,
      scoping: db.ast_scopes.scoping(),
      link_output,
      renamer,
    };

    ctx.rename_bindings_shadowing_star_imports();
    ctx.rename_bindings_shadowing_named_imports();
    ctx.rename_bindings_shadowing_fixed_names(output_format);
  }
}
