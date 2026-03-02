use std::borrow::Cow;

use arcstr::ArcStr;
use indexmap::IndexSet;
use oxc::span::CompactStr;
use oxc_module_graph::types::MatchImportKind as OxcMatchImportKind;
use oxc_module_graph::{ImportHooks, ImportResolutionContext, LinkConfig};
use rolldown_common::{
  IndexModules, MemberExprObjectReferencedType, MemberExprRefResolution, Module, ModuleIdx,
  ModuleType, NamespaceAlias, NormalModule, OutputFormat, ResolvedExport, Specifier,
  SymbolOrMemberExprRef, SymbolRef, SymbolRefFlags,
};
use rolldown_error::{AmbiguousExternalNamespaceModule, BuildDiagnostic};
use rolldown_utils::{
  ecmascript::{is_validate_identifier_name, legitimize_identifier_name},
  index_vec_ext::IndexVecRefExt,
  indexmap::{FxIndexMap, FxIndexSet},
  rayon::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator},
};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{SharedOptions, types::linking_metadata::LinkingMetadataVec};

use super::LinkStage;
use super::oxc_conversions::{from_oxc_module_idx, from_oxc_symbol_ref, to_oxc_symbol_ref};

pub enum RelationWithCommonjs {
  Commonjs,
  IndirectDependOnCommonjs,
}

impl LinkStage<'_> {
  /// Notices:
  /// - For external import like
  /// ```js
  /// // main.js
  /// import { a } from 'external';
  ///
  /// // foo.js
  /// import { a } from 'external';
  /// export { a }
  /// ```
  ///
  /// Unlike import from normal modules, the imported variable deosn't have a place that declared the variable. So we consider `import { a } from 'external'` in `foo.js` as the declaration statement of `a`.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn bind_imports_and_exports(&mut self) {
    // Build resolved exports once using shared algorithm from oxc_module_graph.
    // This initializes local exports and propagates star re-exports (including CJS reexports)
    // with proper shadowing and ambiguity detection.
    // Uses the push-callback API to read exports directly from module_table,
    // avoiding the cost of copying named_exports into graph storage.
    let module_table = &self.module_table;
    let oxc_resolved = oxc_module_graph::build_resolved_exports_with_fn(
      &self.link_kernel.graph,
      &|oxc_idx, cb| {
        let rd_idx = from_oxc_module_idx(oxc_idx);
        if let Some(m) = module_table[rd_idx].as_normal() {
          for (name, export) in &m.named_exports {
            cb(name.as_str(), to_oxc_symbol_ref(export.referenced));
          }
        }
      },
    );
    for (oxc_idx, exports) in &oxc_resolved {
      let module_idx = from_oxc_module_idx(*oxc_idx);
      self.metas[module_idx].resolved_exports = exports
        .iter()
        .map(|(name, export)| {
          (
            CompactStr::from(name.as_str()),
            ResolvedExport {
              symbol_ref: from_oxc_symbol_ref(export.symbol_ref),
              potentially_ambiguous_symbol_refs: export.potentially_ambiguous.as_ref().map(
                |symbols| {
                  symbols.iter().copied().map(from_oxc_symbol_ref).collect::<Vec<_>>()
                },
              ),
              came_from_cjs: export.came_from_cjs,
            },
          )
        })
        .collect();
    }
    let side_effects_modules = self
      .module_table
      .modules
      .iter_enumerated()
      .filter(|(_, item)| item.side_effects().has_side_effects())
      .map(|(idx, _)| idx)
      .collect::<FxHashSet<ModuleIdx>>();
    let mut normal_symbol_exports_chain_map = FxHashMap::default();

    // Pre-processing: collect ESM external import bindings for later merging.
    let is_esm = matches!(self.options.format, OutputFormat::Esm);
    let mut external_import_binding_merger: FxHashMap<
      ModuleIdx,
      FxHashMap<CompactStr, IndexSet<SymbolRef>>,
    > = FxHashMap::default();
    let mut external_import_namespace_merger: FxHashMap<ModuleIdx, FxIndexSet<SymbolRef>> =
      FxHashMap::default();

    for module in &self.module_table.modules {
      let Module::Normal(module) = module else { continue };
      for (imported_as_ref, named_import) in &module.named_imports {
        let rec = &module.import_records[named_import.record_idx];
        let Some(resolved_module_idx) = rec.resolved_module else { continue };
        let is_external =
          matches!(self.module_table.modules[resolved_module_idx], Module::External(_));
        if is_esm && is_external {
          match named_import.imported {
            Specifier::Star => {
              external_import_namespace_merger
                .entry(resolved_module_idx)
                .or_default()
                .insert(*imported_as_ref);
            }
            Specifier::Literal(ref name)
              if self.metas[module.idx]
                .resolved_exports
                .iter()
                .all(|(_, resolved_export)| resolved_export.symbol_ref != *imported_as_ref) =>
            {
              external_import_binding_merger
                .entry(resolved_module_idx)
                .or_default()
                .entry(name.clone())
                .or_default()
                .insert(*imported_as_ref);
            }
            Specifier::Literal(_) => {}
          }
        }
      }
    }

    // Phase 2: Match imports to resolved exports using oxc_module_graph's
    // concrete graph + hooks, while Rolldown still owns the final symbol
    // links for its CJS/external special cases.
    let mut matcher = RolldownImportHooks {
      index_modules: &self.module_table.modules,
      metas: &mut self.metas,
      options: self.options,
      side_effects_modules: &side_effects_modules,
      normal_symbol_exports_chain_map: &mut normal_symbol_exports_chain_map,
      collected_links: Vec::new(),
      namespace_alias_results: Vec::new(),
      shim_requests: Vec::new(),
      errors: Vec::new(),
      warnings: Vec::new(),
    };
    let mut config =
      LinkConfig { cjs_interop: false, import_hooks: Some(&mut matcher), ..Default::default() };
    let module_table = &self.module_table;
    let (_binding_errors, _generated_links) =
      oxc_module_graph::match_imports_collect_with_fn(
        &self.link_kernel.graph,
        &mut config,
        Some(&oxc_resolved),
        &|oxc_idx, cb| {
          let rd_idx = from_oxc_module_idx(oxc_idx);
          if let Some(m) = module_table[rd_idx].as_normal() {
            for ni in m.named_imports.values() {
              let imported_name = match &ni.imported {
                Specifier::Star => "*",
                Specifier::Literal(s) => s.as_str(),
              };
              let is_ns = matches!(&ni.imported, Specifier::Star);
              cb(to_oxc_symbol_ref(ni.imported_as), imported_name, ni.record_idx.index(), is_ns);
            }
          }
        },
        &|oxc_module_idx, oxc_symbol| {
          let rd_idx = from_oxc_module_idx(oxc_module_idx);
          let rd_symbol = from_oxc_symbol_ref(oxc_symbol);
          let m = module_table[rd_idx].as_normal()?;
          let ni = m.named_imports.get(&rd_symbol)?;
          let imported_name = match &ni.imported {
            Specifier::Star => oxc_module_graph::CompactString::from("*"),
            Specifier::Literal(s) => oxc_module_graph::CompactString::from(s.as_str()),
          };
          let is_ns = matches!(&ni.imported, Specifier::Star);
          Some((imported_name, ni.record_idx.index(), is_ns))
        },
      );

    // Extract results from the matcher (releases &mut self.metas borrow).
    let collected_links = std::mem::take(&mut matcher.collected_links);
    let namespace_alias_results = std::mem::take(&mut matcher.namespace_alias_results);
    let shim_requests = std::mem::take(&mut matcher.shim_requests);
    let errors = std::mem::take(&mut matcher.errors);
    let warnings = std::mem::take(&mut matcher.warnings);
    drop(matcher);

    for (from, to) in collected_links {
      self.symbols.link(from, to);
    }

    // Apply namespace alias results (NormalAndNamespace).
    for (symbol, ns_ref, alias) in &namespace_alias_results {
      self.symbols.get_mut(*symbol).namespace_alias =
        Some(NamespaceAlias { property_name: alias.clone(), namespace_ref: *ns_ref });
    }

    // Process shim_missing_exports: create shimmed symbols and link.
    // Facade symbols use countdown IDs from u32::MAX and cannot be stored in
    // the graph's dense SymbolRefDb.  The `safe_canonical` / `graph_canonical_ref`
    // fallback handles them via Rolldown's DB.
    for (_importer_idx, local_symbol, target_idx, imported) in &shim_requests {
      let shimmed_symbol_ref =
        self.metas[*target_idx].shimmed_missing_exports.entry(imported.clone()).or_insert_with(
          || {
            self.symbols.create_facade_root_symbol_ref(*target_idx, imported.as_str())
          },
        );
      self.symbols.link(*local_symbol, *shimmed_symbol_ref);
    }

    self.errors.extend(errors);
    self.warnings.extend(warnings);

    self.external_import_namespace_merger = external_import_namespace_merger;

    // Facade symbols use countdown IDs from u32::MAX — skip graph sync.
    for (module_idx, map) in &external_import_binding_merger {
      for (key, symbol_set) in map {
        let name = if key.as_str() == "default" {
          let key = symbol_set
            .first()
            .map_or_else(|| key.clone(), |sym_ref| sym_ref.name(&self.symbols).into());
          Cow::Owned(key)
        } else if is_validate_identifier_name(key.as_str()) {
          Cow::Borrowed(key)
        } else {
          let legal_name = legitimize_identifier_name(key);
          Cow::Owned(legal_name.as_ref().into())
        };
        let target_symbol = self.symbols.create_facade_root_symbol_ref(*module_idx, &name);
        for symbol_ref in symbol_set {
          self.symbols.link(*symbol_ref, target_symbol);
        }
      }
    }

    let symbols = &self.symbols;
    self.metas.par_iter_mut().for_each(|meta| {
      let safe_canonical = |sym: SymbolRef| -> SymbolRef {
        symbols.canonical_ref_for(sym)
      };
      let mut sorted_and_non_ambiguous_resolved_exports = vec![];
      'next_export: for (exported_name, resolved_export) in &meta.resolved_exports {
        if let Some(potentially_ambiguous_symbol_refs) =
          &resolved_export.potentially_ambiguous_symbol_refs
        {
          let main_ref = safe_canonical(resolved_export.symbol_ref);

          for ambiguous_ref in potentially_ambiguous_symbol_refs {
            let ambiguous_ref = safe_canonical(*ambiguous_ref);
            if main_ref != ambiguous_ref {
              continue 'next_export;
            }
          }
        }
        sorted_and_non_ambiguous_resolved_exports
          .push((exported_name.clone(), resolved_export.came_from_cjs));
      }
      sorted_and_non_ambiguous_resolved_exports.sort_unstable();
      meta.sorted_and_non_ambiguous_resolved_exports =
        FxIndexMap::from_iter(sorted_and_non_ambiguous_resolved_exports);
    });
    self.update_cjs_module_meta();
    self.resolve_member_expr_refs(&side_effects_modules, &normal_symbol_exports_chain_map);
    self.normal_symbol_exports_chain_map = normal_symbol_exports_chain_map;
  }

  /// Update the metadata of CommonJS modules.
  /// - Safe to eliminate interop default export
  ///   e.g.
  /// ```js
  /// // index.js
  /// import a from './a'
  /// a.something // this property could safely rewrite to `a.something` rather than `a.default.something`
  ///
  /// // a.js
  /// module.exports = require('./mod.js')
  ///
  /// // mod.js
  /// exports.something = 1
  /// ```
  fn update_cjs_module_meta(&mut self) {
    let relation_with_commonjs_map = self
      .module_table
      .modules
      .iter()
      .filter_map(|module| {
        let module = module.as_normal()?;
        if module.exports_kind.is_commonjs() {
          Some((module.idx, RelationWithCommonjs::Commonjs))
        } else if self.metas[module.idx].has_dynamic_exports {
          Some((module.idx, RelationWithCommonjs::IndirectDependOnCommonjs))
        } else {
          None
        }
      })
      .collect::<FxHashMap<ModuleIdx, RelationWithCommonjs>>();

    let idx_to_symbol_ref_to_module_idx_map = self
      .module_table
      .par_iter_enumerated()
      .filter_map(|(idx, module)| {
        // a cjs module could only be referenced by normal modules.
        let module = module.as_normal()?;
        let mut named_import_to_cjs_module = FxHashMap::default();
        let mut import_record_ns_to_cjs_module = FxHashMap::default();
        module.named_imports.iter().for_each(|(_, named_import)| {
          let rec = &module.import_records[named_import.record_idx];
          if let Some(module_idx) = rec.resolved_module && relation_with_commonjs_map.contains_key(&module_idx) {
            named_import_to_cjs_module.insert(named_import.imported_as, module_idx);
          }
        });
        module.import_records.iter().for_each(|item| {
          if let Some(module_idx) = item.resolved_module && let Some(RelationWithCommonjs::Commonjs) = relation_with_commonjs_map.get(&module_idx) {
            import_record_ns_to_cjs_module.insert(item.namespace_ref, module_idx);
          }
        });
        (!named_import_to_cjs_module.is_empty() || !import_record_ns_to_cjs_module.is_empty()).then_some((idx, (named_import_to_cjs_module, import_record_ns_to_cjs_module)))
      })
      .collect::<FxHashMap<ModuleIdx, (FxHashMap<SymbolRef, ModuleIdx>, FxHashMap<SymbolRef, ModuleIdx>)>>();
    for (k, (named_import_to_cjs_module, import_record_ns_to_cjs_module)) in
      idx_to_symbol_ref_to_module_idx_map
    {
      let meta = &mut self.metas[k];
      meta.named_import_to_cjs_module = named_import_to_cjs_module;
      meta.import_record_ns_to_cjs_module = import_record_ns_to_cjs_module;
    }
  }

  /// Try to find the final pointed `SymbolRef` of the member expression.
  /// ```js
  /// // index.js
  /// import * as foo_ns from './foo';
  /// foo_ns.bar_ns.c;
  /// // foo.js
  /// export * as bar_ns from './bar';
  /// // bar.js
  /// export const c = 1;
  /// ```
  /// The final pointed `SymbolRef` of `foo_ns.bar_ns.c` is the `c` in `bar.js`.
  #[expect(clippy::too_many_lines)]
  fn resolve_member_expr_refs(
    &mut self,
    side_effects_modules: &FxHashSet<ModuleIdx>,
    normal_symbol_exports_chain_map: &FxHashMap<SymbolRef, Vec<SymbolRef>>,
  ) {
    let warnings = append_only_vec::AppendOnlyVec::new();
    let symbols_db = &self.symbols;
    let safe_canonical = |sym: SymbolRef| -> SymbolRef {
      symbols_db.canonical_ref_for(sym)
    };
    let resolved_meta_data = self
      .module_table
      .modules
      .par_iter()
      .map(|module| match module {
        Module::Normal(module) => {
          let mut resolved_map = FxHashMap::default();
          let mut side_effects_dependency = vec![];
          let mut written_cjs_exports: Vec<SymbolRef> = vec![];
          module.stmt_infos.iter().for_each(|stmt_info| {
            stmt_info.referenced_symbols.iter().for_each(|symbol_ref| {
              // `depended_refs` is used to store necessary symbols that must be included once the resolved symbol gets included
              let mut depended_refs: Vec<SymbolRef> = vec![];

              if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = symbol_ref {
                // First get the canonical ref of `foo_ns`, then we get the `NormalModule#namespace_object_ref` of `foo.js`.
                let mut canonical_ref = safe_canonical(member_expr_ref.object_ref);
                let mut canonical_ref_owner: &NormalModule =
                  match &self.module_table[canonical_ref.owner] {
                    Module::Normal(module) => module,
                    Module::External(_) => return,
                  };
                let is_json_import_ns =
                  (matches!(canonical_ref_owner.module_type, ModuleType::Json)
                    && member_expr_ref.object_ref_type == MemberExprObjectReferencedType::Default);
                let mut is_namespace_ref =
                  canonical_ref_owner.namespace_object_ref == canonical_ref || is_json_import_ns;
                let mut cursor = 0;
                while cursor < member_expr_ref.prop_and_span_list.len() && is_namespace_ref {
                  let (name, _related_span) = &member_expr_ref.prop_and_span_list[cursor];
                  let meta = &self.metas[canonical_ref_owner.idx];
                  let export_symbol = meta.resolved_exports.get(name).and_then(|resolved_export| {
                    (!resolved_export.came_from_cjs).then_some(resolved_export)
                  });
                  let Some(export_symbol) = export_symbol else {
                    // when we try to resolve `a.b.c`, and found that `b` is not exported by module
                    // that `a` pointed to, convert the `a.b.c` into `void 0` if module `a` do not
                    // have any dynamic exports.
                    if !self.metas[canonical_ref_owner.idx].has_dynamic_exports {
                      resolved_map.insert(
                        member_expr_ref.span,
                        MemberExprRefResolution {
                          resolved: if is_json_import_ns { Some(canonical_ref) } else { None },
                          prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..]
                            .to_vec(),
                          depended_refs: vec![],
                          target_commonjs_exported_symbol: None,
                          reference_id: member_expr_ref.reference_id,
                        },
                      );
                    }
                    if !self.metas[canonical_ref_owner.idx].has_dynamic_exports
                      && !is_json_import_ns
                    {
                      warnings.push(
                        BuildDiagnostic::import_is_undefined(
                          module.id.as_arc_str().clone(),
                          module.source.clone(),
                          member_expr_ref.span,
                          ArcStr::from(name.as_str()),
                          canonical_ref_owner.stable_id.to_string(),
                        )
                        .with_severity_warning(),
                      );
                    }
                    break;
                  };
                  if !meta
                    .sorted_and_non_ambiguous_resolved_exports
                    .contains_key(&CompactStr::new(name))
                  {
                    resolved_map.insert(
                      member_expr_ref.span,
                      MemberExprRefResolution {
                        resolved: None,
                        prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..]
                          .to_vec(),
                        depended_refs: vec![],
                        target_commonjs_exported_symbol: None,
                        reference_id: member_expr_ref.reference_id,
                      },
                    );
                    return;
                  }

                  depended_refs.push(export_symbol.symbol_ref);
                  if let Some(chains) =
                    normal_symbol_exports_chain_map.get(&export_symbol.symbol_ref)
                  {
                    depended_refs.extend(chains);
                    for item in chains {
                      if side_effects_modules.contains(&item.owner) {
                        side_effects_dependency.push(item.owner);
                      }
                    }
                  }
                  canonical_ref = safe_canonical(export_symbol.symbol_ref);
                  // If the canonical ref points to an external module, we can't resolve
                  // further properties statically. Break out of the loop and let the
                  // remaining properties be accessed at runtime.
                  let Some(normal_module) = self.module_table[canonical_ref.owner].as_normal()
                  else {
                    cursor += 1;
                    break;
                  };
                  canonical_ref_owner = normal_module;
                  cursor += 1;
                  is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
                }
                let mut target_commonjs_exported_symbol = None;
                // Although the last one may not be a namespace ref, but it may reference a cjs
                // import record namespace, which may reference a export in commonjs module.
                // TODO: we could record if a module could potential reference a cjs symbol
                // so that we could skip this step.
                if cursor < member_expr_ref.prop_and_span_list.len() {
                  let maybe_namespace = depended_refs
                    .last()
                    .copied()
                    .unwrap_or(safe_canonical(member_expr_ref.object_ref));
                  let maybe_namespace_symbol = self.symbols.get(maybe_namespace);
                  let continue_resolve =
                    if let Some(ref ns) = maybe_namespace_symbol.namespace_alias {
                      // If the property_name is not "default", it means the symbol reference a imported binding
                      // rather than a namespace object.
                      // e.g. import { foo } from './cjs'
                      ns.property_name.as_str() == "default"
                    } else {
                      true
                    };
                  // corresponding to cases in:
                  // https://github.com/rolldown/rolldown/blob/30a5a2fc8fa6785821153922e21dc0273cc00c7a/crates/rolldown/tests/rolldown/tree_shaking/commonjs/main.js?plain=1#L3-L10
                  if continue_resolve
                    && let Some(m) = self.metas[maybe_namespace.owner]
                      .named_import_to_cjs_module
                      .get(&maybe_namespace)
                      .or_else(|| {
                        self.metas[maybe_namespace.owner]
                          .import_record_ns_to_cjs_module
                          .get(&maybe_namespace)
                      })
                      .or_else(|| {
                        (self.metas[maybe_namespace.owner].has_dynamic_exports)
                          .then_some(&maybe_namespace.owner)
                      })
                      .and_then(|idx| {
                        self.metas[*idx]
                          .resolved_exports
                          .get(&member_expr_ref.prop_and_span_list[cursor].0)
                          .and_then(|resolved_export| {
                            resolved_export.came_from_cjs.then_some(resolved_export)
                          })
                      })
                  {
                    let is_default = member_expr_ref.prop_and_span_list[cursor].0 == "default";
                    target_commonjs_exported_symbol = Some((m.symbol_ref, is_default));
                    depended_refs.push(m.symbol_ref);
                    // If this member expression is a write (e.g. `cjs.c = 'abcd'`), the
                    // CJS exported symbol should not be inlined as a constant since its
                    // value may change at runtime.
                    if member_expr_ref.is_write {
                      written_cjs_exports.push(m.symbol_ref);
                    }
                  }
                }

                if cursor > 0 {
                  // The module namespace might be created in the other module get imported via named import instead of `import * as`.
                  // We need to include the possible export chain.
                  depended_refs.push(member_expr_ref.object_ref);
                  normal_symbol_exports_chain_map.get(&member_expr_ref.object_ref).inspect(
                    |refs| {
                      depended_refs.extend(*refs);
                    },
                  );
                }

                if cursor > 0 || target_commonjs_exported_symbol.is_some() {
                  resolved_map.insert(
                    member_expr_ref.span,
                    MemberExprRefResolution {
                      resolved: Some(canonical_ref),
                      prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..]
                        .to_vec(),
                      depended_refs,
                      target_commonjs_exported_symbol,
                      reference_id: member_expr_ref.reference_id,
                    },
                  );
                }
              }
            });
          });

          (resolved_map, side_effects_dependency, written_cjs_exports)
        }
        Module::External(_) => (FxHashMap::default(), vec![], vec![]),
      })
      .collect::<Vec<_>>();

    debug_assert_eq!(self.metas.len(), resolved_meta_data.len());
    self.warnings.extend(warnings);
    // Remove CJS exported symbols that are written to by importers from the constant map
    // to prevent incorrect inlining of mutated values.
    // First, collect statically-known written symbols gathered during resolution above.
    // Then, for imports flagged with `HasComputedMemberWrite` (dynamic computed writes like
    // `cjs[name] = value`, or writes through `ns.default`), bail out all CJS exports of the
    // target module since we can't determine which specific property is affected.
    let mut written_cjs_export_symbols: Vec<SymbolRef> = Vec::new();
    for (meta, (_, _, written_cjs_exports)) in self.metas.iter().zip(resolved_meta_data.iter()) {
      written_cjs_export_symbols.extend(written_cjs_exports);
      for (import_symbol, cjs_module_idx) in
        meta.named_import_to_cjs_module.iter().chain(meta.import_record_ns_to_cjs_module.iter())
      {
        if import_symbol
          .flags(&self.symbols)
          .is_some_and(|f| f.contains(SymbolRefFlags::HasComputedMemberWrite))
        {
          written_cjs_export_symbols.extend(
            self.metas[*cjs_module_idx]
              .resolved_exports
              .values()
              .filter(|e| e.came_from_cjs)
              .map(|e| e.symbol_ref),
          );
        }
      }
    }
    for symbol_ref in &written_cjs_export_symbols {
      self.global_constant_symbol_map.remove(symbol_ref);
    }
    self.metas.iter_mut().zip(resolved_meta_data).for_each(
      |(meta, (resolved_map, side_effects_dependency, _))| {
        meta.resolved_member_expr_refs = resolved_map;
        meta.dependencies.extend(side_effects_dependency);
      },
    );
  }
}

/// Rolldown-specific import hooks that bridge `oxc_module_graph::match_imports_collect`
/// with Rolldown's CJS interop, external module handling, and diagnostics.
struct RolldownImportHooks<'a> {
  index_modules: &'a IndexModules,
  metas: &'a mut LinkingMetadataVec,
  options: &'a SharedOptions,
  side_effects_modules: &'a FxHashSet<ModuleIdx>,
  normal_symbol_exports_chain_map: &'a mut FxHashMap<SymbolRef, Vec<SymbolRef>>,
  /// Collected links to apply to Rolldown's persistent `SymbolRefDb`.
  collected_links: Vec<(SymbolRef, SymbolRef)>,
  /// Collected (local_symbol, namespace_ref, alias) for NormalAndNamespace results.
  namespace_alias_results: Vec<(SymbolRef, SymbolRef, CompactStr)>,
  /// Collected (importer_idx, local_symbol, target_idx, import_name) for NoMatch shimming.
  shim_requests: Vec<(ModuleIdx, SymbolRef, ModuleIdx, CompactStr)>,
  errors: Vec<BuildDiagnostic>,
  warnings: Vec<BuildDiagnostic>,
}

impl ImportHooks for RolldownImportHooks<'_> {
  fn on_resolved(&mut self, ctx: &ImportResolutionContext) {
    let importer_idx = from_oxc_module_idx(ctx.importer);
    let local_symbol = from_oxc_symbol_ref(ctx.local_symbol);
    let resolved = ctx.result;
    let reexport_chain = ctx.reexport_chain;

    match resolved {
      OxcMatchImportKind::Normal { .. }
      | OxcMatchImportKind::Namespace { .. }
      | OxcMatchImportKind::NormalAndNamespace { .. } => {
        let Some(importer) = self.index_modules[importer_idx].as_normal() else {
          return;
        };
        let Some(named_import) = importer.named_imports.get(&local_symbol) else {
          return;
        };
        let rec = &importer.import_records[named_import.record_idx];
        let Some(target_idx) = rec.resolved_module else {
          return;
        };

        let mut final_link = match resolved {
          OxcMatchImportKind::Normal { symbol_ref } => {
            Some((local_symbol, from_oxc_symbol_ref(*symbol_ref)))
          }
          OxcMatchImportKind::Namespace { namespace_ref } => {
            Some((local_symbol, from_oxc_symbol_ref(*namespace_ref)))
          }
          // Keep aliased imports self-canonical so downstream consumers still
          // observe `namespace_alias` on the canonical symbol.
          OxcMatchImportKind::NormalAndNamespace { .. } => Some((local_symbol, local_symbol)),
          OxcMatchImportKind::Ambiguous { .. }
          | OxcMatchImportKind::Cycle
          | OxcMatchImportKind::NoMatch => None,
        };
        let mut namespace_alias = match resolved {
          OxcMatchImportKind::NormalAndNamespace { namespace_ref, alias } => {
            Some((from_oxc_symbol_ref(*namespace_ref), CompactStr::new(alias.as_str())))
          }
          _ => None,
        };

        let is_namespace_import = matches!(&named_import.imported, Specifier::Star);

        match &self.index_modules[target_idx] {
          Module::External(_) if self.options.format.keep_esm_import_export_syntax() => {
            // ESM output preserves external imports as-is. Keep the local symbol canonical.
            final_link = Some((local_symbol, local_symbol));
            namespace_alias = None;
          }
          Module::Normal(target) if target.exports_kind.is_commonjs() => {
            // Rolldown uses the importer's synthetic namespace symbol for CJS interop.
            if let Specifier::Literal(imported) = &named_import.imported {
              if let Some(symbol_ref) = self.metas[target_idx]
                .resolved_exports
                .get(imported)
                .map(|resolved_export| resolved_export.symbol_ref)
              {
                self.metas[target_idx].included_commonjs_export_symbol.insert(symbol_ref);
              }
              namespace_alias = Some((rec.namespace_ref, imported.clone()));
              final_link = Some((local_symbol, local_symbol));
            } else {
              namespace_alias = None;
              final_link = Some((local_symbol, rec.namespace_ref));
            }
          }
          Module::Normal(target) if !is_namespace_import => {
            if let Specifier::Literal(imported) = &named_import.imported
              && let Some((symbol_ref, came_from_cjs)) = self.metas[target_idx]
                .resolved_exports
                .get(imported)
                .map(|resolved_export| (resolved_export.symbol_ref, resolved_export.came_from_cjs))
              && came_from_cjs
            {
              self.metas[target_idx].included_commonjs_export_symbol.insert(symbol_ref);
              namespace_alias = Some((target.namespace_object_ref, imported.clone()));
              final_link = Some((local_symbol, local_symbol));
            }
          }
          Module::Normal(_) | Module::External(_) => {}
        }

        if let Some((_, target_symbol)) = final_link {
          self.collected_links.push((local_symbol, target_symbol));
        }
        if let Some((namespace_ref, alias)) = namespace_alias {
          self.namespace_alias_results.push((local_symbol, namespace_ref, alias));
        }

        let converted_chain =
          reexport_chain.iter().copied().map(from_oxc_symbol_ref).collect::<Vec<_>>();
        for symbol_ref in &converted_chain {
          if self.side_effects_modules.contains(&symbol_ref.owner) {
            self.metas[importer_idx].dependencies.insert(symbol_ref.owner);
          }
        }
        if !converted_chain.is_empty() {
          self.normal_symbol_exports_chain_map.insert(local_symbol, converted_chain);
        }
      }
      OxcMatchImportKind::NoMatch => {
        // Look up import info to generate diagnostics or collect shim requests.
        let Some(importer) = self.index_modules[importer_idx].as_normal() else { return };
        let Some(named_import) = importer.named_imports.get(&local_symbol) else { return };
        let rec = &importer.import_records[named_import.record_idx];
        let Some(resolved_module_idx) = rec.resolved_module else { return };
        let importee = &self.index_modules[resolved_module_idx];

        // Check shim_missing_exports.
        if let Module::Normal(importee_normal) = importee {
          if self.options.shim_missing_exports
            || matches!(importee_normal.module_type, ModuleType::Empty)
          {
            if let Specifier::Literal(ref imported) = named_import.imported {
              self.shim_requests.push((
                importer_idx,
                local_symbol,
                resolved_module_idx,
                imported.clone(),
              ));
              return;
            }
          }
        }

        // Generate missing_export diagnostic.
        let is_ts_like_importing_ts_like =
          matches!(
            importee.as_normal().map(|m| &m.module_type),
            Some(ModuleType::Ts | ModuleType::Tsx)
          ) && matches!(importer.module_type, ModuleType::Ts | ModuleType::Tsx);
        let mut diagnostic = BuildDiagnostic::missing_export(
          importer.id.to_string(),
          importer.stable_id.to_string(),
          importee.id().to_string(),
          importee.stable_id().to_string(),
          importer.source.clone(),
          named_import.imported.to_string(),
          named_import.span_imported,
          is_ts_like_importing_ts_like.then(|| {
            format!(
              "If you meant to import a type rather than a value, make sure to add the `type` modifier (e.g. `import {{ type Foo }} from '{}'`).",
              rec.module_request
            )
          }),
        );
        if is_ts_like_importing_ts_like {
          diagnostic = diagnostic.with_severity_warning();
          self.warnings.push(diagnostic);
        } else {
          self.errors.push(diagnostic);
        }
      }
      OxcMatchImportKind::Ambiguous { candidates } => {
        let Some(importer) = self.index_modules[importer_idx].as_normal() else { return };
        let Some(named_import) = importer.named_imports.get(&local_symbol) else { return };
        let rec = &importer.import_records[named_import.record_idx];
        let Some(resolved_module_idx) = rec.resolved_module else { return };
        let importee_id = self.index_modules[resolved_module_idx].stable_id().to_string();

        let mut exporter = Vec::new();
        for candidate in candidates {
          let candidate = from_oxc_symbol_ref(*candidate);
          if let Some(owner) = self.index_modules[candidate.owner].as_normal() {
            if let Specifier::Literal(name) = &named_import.imported {
              if let Some(named_export) = owner.named_exports.get(name) {
                exporter.push(AmbiguousExternalNamespaceModule {
                  source: owner.source.clone(),
                  module_id: owner.id.to_string(),
                  stable_id: owner.stable_id.to_string(),
                  span_of_identifier: named_export.span,
                });
              }
            }
          }
        }

        self.errors.push(BuildDiagnostic::ambiguous_external_namespace(
          named_import.imported.to_string(),
          importee_id,
          AmbiguousExternalNamespaceModule {
            source: importer.source.clone(),
            module_id: importer.id.to_string(),
            stable_id: importer.stable_id.to_string(),
            span_of_identifier: named_import.span_imported,
          },
          exporter,
        ));
      }
      OxcMatchImportKind::Cycle => {
        let Some(importer) = self.index_modules[importer_idx].as_normal() else { return };
        let Some(named_import) = importer.named_imports.get(&local_symbol) else { return };
        let rec = &importer.import_records[named_import.record_idx];
        let Some(resolved_module_idx) = rec.resolved_module else { return };
        let importee = &self.index_modules[resolved_module_idx];
        self.errors.push(BuildDiagnostic::circular_reexport(
          importee.id().to_string(),
          named_import.imported.to_string(),
        ));
      }
    }
  }
}
