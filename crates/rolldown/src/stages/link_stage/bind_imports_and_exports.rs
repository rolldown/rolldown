use arcstr::ArcStr;
use rolldown_common::{
  EcmaModuleAstUsage, MemberExprObjectReferencedType, MemberExprRefResolution, Module, ModuleIdx,
  ModuleType, NormalModule, StmtInfoMeta, StmtInfos, SymbolOrMemberExprRef, SymbolRef,
  SymbolRefFlags,
};
use rolldown_error::BuildDiagnostic;
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  index_vec_ext::IndexVecRefExt,
  rayon::{IntoParallelRefIterator, ParallelIterator},
};

use rustc_hash::{FxHashMap, FxHashSet};

use super::{LinkStage, non_splittable_json_defaults::NonSplittableJsonDefaults};

pub enum RelationWithCommonjs {
  Commonjs,
  IndirectDependOnCommonjs,
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn finish_binding(
    &mut self,
    resolved_exports: &super::passes::ResolvedExports,
    normal_symbol_exports_chain_map: FxHashMap<SymbolRef, Vec<SymbolRef>>,
    fallback_non_splittable_json_defaults: &NonSplittableJsonDefaults,
  ) {
    let side_effects_modules = self
      .module_table
      .modules
      .iter_enumerated()
      .filter(|(_, item)| item.side_effects().has_side_effects())
      .map(|(idx, _)| idx)
      .collect::<FxHashSet<ModuleIdx>>();
    self.update_cjs_module_meta();
    self.resolve_member_expr_refs(
      resolved_exports,
      &side_effects_modules,
      &normal_symbol_exports_chain_map,
      fallback_non_splittable_json_defaults,
    );
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
    resolved_exports: &super::passes::ResolvedExports,
    side_effects_modules: &FxHashSet<ModuleIdx>,
    normal_symbol_exports_chain_map: &FxHashMap<SymbolRef, Vec<SymbolRef>>,
    fallback_non_splittable_json_defaults: &NonSplittableJsonDefaults,
  ) {
    let warnings = append_only_vec::AppendOnlyVec::new();
    // A JSON default object whose `data.key` accesses get rewritten to the statically split
    // per-key exports must stay un-split once it is mutated or escapes (see
    // `collect_non_splittable_json_defaults`). Such a mutation/escape in one module invalidates
    // the optimization for reads of the same JSON default in *any* module (e.g. when the default
    // is mutated through a re-exporting wrapper), so the non-splittable set must be unioned across
    // the whole graph before any read is resolved below. Only pay for the scan when JSON exists.
    let has_json_module = self.module_table.modules.iter().any(|module| {
      module.as_normal().is_some_and(|module| matches!(module.module_type, ModuleType::Json))
    });
    let mut non_splittable_json_defaults: FxHashSet<SymbolRef> =
      fallback_non_splittable_json_defaults
        .iter()
        .map(|symbol_ref| self.symbols.canonical_ref_for(symbol_ref))
        .collect();
    if has_json_module {
      non_splittable_json_defaults.extend(
        self
          .module_table
          .modules
          .par_iter()
          .zip(self.stmt_infos.par_iter())
          .map(|(module, stmt_infos)| match module {
            Module::Normal(_) => {
              self.collect_non_splittable_json_defaults(resolved_exports, stmt_infos)
            }
            Module::External(_) => FxHashSet::default(),
          })
          .collect::<Vec<_>>()
          .into_iter()
          .flatten()
          .collect::<FxHashSet<_>>(),
      );
    }
    let resolved_meta_data = self
      .module_table
      .modules
      .par_iter()
      .zip(self.stmt_infos.par_iter())
      .map(|(module, stmt_infos)| match module {
        Module::Normal(module) => {
          let mut resolved_map = FxHashMap::default();
          let mut side_effects_dependency = vec![];
          let mut written_cjs_exports: Vec<SymbolRef> = vec![];
          stmt_infos.iter().for_each(|stmt_info| {
            stmt_info.referenced_symbols.iter().for_each(|symbol_ref| {
              // `depended_refs` is used to store necessary symbols that must be included once the resolved symbol gets included
              let mut depended_refs: Vec<SymbolRef> = vec![];

              if let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = symbol_ref {
                // First get the canonical ref of `foo_ns`, then we get the `NormalModule#namespace_object_ref` of `foo.js`.
                let mut canonical_ref = self.symbols.canonical_ref_for(member_expr_ref.object_ref);
                let mut canonical_ref_owner: &NormalModule =
                  match &self.module_table[canonical_ref.owner] {
                    Module::Normal(module) => module,
                    Module::External(_) => return,
                  };
                // Treat `import data from './x.json'; data.foo` as a namespace access so
                // it can be optimized to the underlying `foo` export. Skip this for writes
                // (`data.foo = ...`) since rewriting the write target to a bare identifier
                // (or worse, an inlined constant) is unsound and would crash the finalizer.
                // Also skip all reads from a JSON default object that is non-splittable, i.e. it
                // is mutated or escapes anywhere in the graph (`collect_non_splittable_json_defaults`),
                // because then the split `foo` export no longer reflects the live `data.foo`.
                // Finally, skip when the first prop is `"default"`: `data` is already the
                // default export value, and the JSON module's `"default"` named export points
                // to that same symbol, so the optimization would resolve `.default` to `data`
                // itself and silently drop the access. Since JSON resolution is single-level,
                // gating at setup is sufficient — the loop never re-enters for a JSON module
                // after the first iteration.
                let is_json_import_ns = matches!(canonical_ref_owner.module_type, ModuleType::Json)
                  && member_expr_ref.object_ref_type == MemberExprObjectReferencedType::Default
                  && !member_expr_ref.is_write
                  && !non_splittable_json_defaults.contains(&canonical_ref)
                  && member_expr_ref
                    .prop_and_span_list
                    .first()
                    .is_none_or(|prop| prop.name.as_str() != "default");
                let mut is_namespace_ref =
                  canonical_ref_owner.namespace_object_ref == canonical_ref || is_json_import_ns;
                let mut cursor = 0;
                while cursor < member_expr_ref.prop_and_span_list.len() && is_namespace_ref {
                  let prop = &member_expr_ref.prop_and_span_list[cursor];
                  let name = &prop.name;
                  let export_symbol = resolved_exports.get(canonical_ref_owner.idx, name).and_then(
                    |resolved_export| {
                      (!resolved_export.came_from_commonjs).then_some(resolved_export)
                    },
                  );
                  let Some(export_symbol) = export_symbol else {
                    // when we try to resolve `a.b.c`, and found that `b` is not exported by module
                    // that `a` pointed to, convert the `a.b.c` into `void 0` if module `a` do not
                    // have any dynamic exports.
                    if !self.metas[canonical_ref_owner.idx].has_dynamic_exports {
                      resolved_map.insert(
                        member_expr_ref.node_id,
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
                  if !resolved_exports.contains_canonical_name(canonical_ref_owner.idx, name) {
                    resolved_map.insert(
                      member_expr_ref.node_id,
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
                  canonical_ref = self.symbols.canonical_ref_for(export_symbol.symbol_ref);
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
                    .unwrap_or(self.symbols.canonical_ref_for(member_expr_ref.object_ref));
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
                  let cjs_module_idx = continue_resolve
                    .then(|| {
                      self.metas[maybe_namespace.owner]
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
                        .copied()
                    })
                    .flatten();
                  if let Some(cjs_idx) = cjs_module_idx
                    && let Some(m) = resolved_exports
                      .get(cjs_idx, &member_expr_ref.prop_and_span_list[cursor].name)
                      .and_then(|resolved_export| {
                        resolved_export.came_from_commonjs.then_some(resolved_export)
                      })
                  {
                    let is_default = member_expr_ref.prop_and_span_list[cursor].name == "default";
                    // When the accessed property is `default`, check if `.default` represents
                    // the whole `module.exports` (rather than `exports.default`). This is true when:
                    // - Node ESM mode: __toESM always ignores __esModule flag
                    // - Non-node mode without __esModule: __toESM sets .default = module.exports
                    // In these cases, skip resolving `.default` to a specific CJS export.
                    let default_is_module_exports = is_default && {
                      let is_node_esm = module.should_consider_node_esm_spec_for_static_import();
                      let importee_has_es_module_flag =
                        self.module_table[cjs_idx].as_normal().is_some_and(|importee| {
                          importee.ecma_view.ast_usage.contains(EcmaModuleAstUsage::EsModuleFlag)
                        });
                      is_node_esm || !importee_has_es_module_flag
                    };

                    // If the current property is `default` and it represents the whole `module.exports`,
                    // try to resolve the next property as a CJS export.
                    if default_is_module_exports
                      && let Some(next_prop) = member_expr_ref.prop_and_span_list.get(cursor + 1)
                    {
                      if let Some(property) =
                        resolved_exports.get(cjs_idx, &next_prop.name).and_then(|resolved_export| {
                          resolved_export.came_from_commonjs.then_some(resolved_export)
                        })
                      {
                        let is_next_default = next_prop.name == "default";
                        if is_next_default && maybe_namespace_symbol.namespace_alias.is_none() {
                          // import * as ns; ns.default.default — can't optimize.
                          //
                          // __toESM sets import_ns.default = module.exports and __copyProps
                          // skips "default" (already set), so exports.default is only
                          // reachable via import_ns.default.default (two levels).
                          // If we advance cursor, props becomes ["default"] and the finalizer
                          // base is import_ns (#LOCAL_NAMESPACE has no namespace_alias to
                          // append .default), so the result is import_ns.default which is
                          // module.exports — not module.exports.default.
                          //
                          // Other non-"default" properties (e.g. ns.default.foo) work fine
                          // because __copyProps copies them onto the __toESM target, so
                          // import_ns.foo = module.exports.foo.
                        } else {
                          cursor += 1;
                          target_commonjs_exported_symbol =
                            Some((property.symbol_ref, is_next_default));
                          depended_refs.push(property.symbol_ref);

                          if member_expr_ref.is_write {
                            written_cjs_exports.push(property.symbol_ref);
                          }
                        }
                      }
                    } else if default_is_module_exports {
                      // `.default` represents the whole `module.exports` with no further
                      // property access. Leave target_commonjs_exported_symbol as None so
                      // that include_statements runs the CJS bailout check and keeps all
                      // exports for this opaque usage.
                    } else {
                      target_commonjs_exported_symbol = Some((m.symbol_ref, is_default));
                    }
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
                    member_expr_ref.node_id,
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
    self.diagnostics.extend(warnings);
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
            resolved_exports
              .iter(*cjs_module_idx)
              .map(|(_, resolved_export)| resolved_export)
              .filter(|e| e.came_from_commonjs)
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

  /// Collect the JSON-module default exports in `stmt_infos` that must NOT be split into their
  /// per-key named exports, because the live default object is observably mutated or escapes.
  ///
  /// The `data.foo` → split-`foo`-export rewrite (see `resolve_member_expr_refs`) is only sound
  /// while every reference to the default object is a plain `data.<key>` read. Once the object is
  /// mutated (`data.foo = ...`) or escapes — aliased (`const o = data`), passed to a function
  /// (`f(data)`), indexed dynamically (`data[k]`), returned, ... — the split exports may diverge
  /// from the live object, so the whole default has to stay materialized and the access left as-is.
  ///
  /// This scans a single module; the caller unions the results across the whole graph, since a
  /// mutation/escape in one module invalidates the optimization for reads of the same JSON default
  /// in any other module (e.g. through a re-exporting wrapper).
  fn collect_non_splittable_json_defaults(
    &self,
    resolved_exports: &super::passes::ResolvedExports,
    stmt_infos: &StmtInfos,
  ) -> FxHashSet<SymbolRef> {
    let mut non_splittable = FxHashSet::default();
    for stmt_info in stmt_infos.iter() {
      if stmt_info.meta.contains(StmtInfoMeta::LazyJsonExportInitializer) {
        continue;
      }
      for reference in &stmt_info.referenced_symbols {
        if let Some(json_default) =
          self.json_default_made_non_splittable_by(resolved_exports, reference)
        {
          non_splittable.insert(json_default);
        }
      }
    }
    non_splittable
  }

  /// If `reference` makes a JSON default non-splittable (mutation or escape, see
  /// [`Self::collect_non_splittable_json_defaults`]), return that default's canonical `SymbolRef`.
  fn json_default_made_non_splittable_by(
    &self,
    resolved_exports: &super::passes::ResolvedExports,
    reference: &SymbolOrMemberExprRef,
  ) -> Option<SymbolRef> {
    match reference {
      // A bare reference to the binding (anything other than a static `x.key` access) means the
      // default object flows somewhere we can't track: an alias (`const o = data`), a call
      // argument (`f(data)`), a dynamic index (`data[k]`, which also covers computed writes
      // `data[k] = ...`), a `return`, etc. Any of these may mutate or alias it, so bail. Static
      // `data.key` accesses are recorded as `MemberExpr` (below), never as a bare `Symbol`, so
      // this does not disable the optimization for the case it targets.
      SymbolOrMemberExprRef::Symbol(symbol_ref) => {
        self.json_default_canonical_ref(resolved_exports, *symbol_ref)
      }
      SymbolOrMemberExprRef::MemberExpr(member_expr_ref) => {
        // Reads (`data.key`) are exactly the optimizable use — never bail on them.
        if !member_expr_ref.is_write {
          return None;
        }
        match member_expr_ref.object_ref_type {
          // `data.key = ...` where `data` is a default-imported JSON object, possibly reached
          // through a named re-export (`export { default as data } from './x.json'`).
          MemberExprObjectReferencedType::Default | MemberExprObjectReferencedType::Named => {
            self.json_default_canonical_ref(resolved_exports, member_expr_ref.object_ref)
          }
          // `ns.default.key = ...` — the mutated object is the namespace's default export.
          MemberExprObjectReferencedType::Namespace
            if member_expr_ref
              .prop_and_span_list
              .first()
              .is_some_and(|prop| prop.name.as_str() == "default") =>
          {
            let canonical_ref = self.symbols.canonical_ref_for(member_expr_ref.object_ref);
            let is_json_namespace =
              self.module_table[canonical_ref.owner].as_normal().is_some_and(|module| {
                matches!(module.module_type, ModuleType::Json)
                  && module.namespace_object_ref == canonical_ref
              });
            if !is_json_namespace {
              return None;
            }
            resolved_exports
              .get(canonical_ref.owner, "default")
              .map(|export| self.symbols.canonical_ref_for(export.symbol_ref))
          }
          MemberExprObjectReferencedType::Namespace => None,
        }
      }
    }
  }

  /// Canonicalize `symbol_ref` and return it iff it resolves to a JSON module's `default` export.
  fn json_default_canonical_ref(
    &self,
    resolved_exports: &super::passes::ResolvedExports,
    symbol_ref: SymbolRef,
  ) -> Option<SymbolRef> {
    let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
    let is_json_default =
      self.module_table[canonical_ref.owner].as_normal().is_some_and(|module| {
        matches!(module.module_type, ModuleType::Json)
          && resolved_exports.get(canonical_ref.owner, "default").is_some_and(|default_export| {
            self.symbols.canonical_ref_for(default_export.symbol_ref) == canonical_ref
          })
      });
    is_json_default.then_some(canonical_ref)
  }
}
