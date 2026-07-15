use std::convert::Infallible;

use arcstr::ArcStr;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaModuleAstUsage, MemberExprObjectReferencedType, MemberExprRefResolution,
  MemberExprRefResolutionMap, Module, ModuleIdx, ModuleTable, ModuleType, NormalModule,
  StmtInfoMeta, StmtInfos, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb, SymbolRefFlags,
};
use rolldown_error::BuildDiagnostic;
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
  rayon::{IntoParallelRefIterator, ParallelIterator},
};
use rustc_hash::FxHashSet;

use crate::type_alias::IndexStmtInfos;

use super::super::non_splittable_json_defaults::NonSplittableJsonDefaults;
use super::{
  CjsRoutingDraft, CjsRoutingFinal, DynamicExports, GlobalConstantsDraft, ModuleDependenciesDraft,
  ModuleSideEffects, NormalExportChains, ResolveMemberExpressionsPass, ResolvedExports,
};

// See internal-docs/pass-based-pipeline/implementation.md.

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ResolveMemberExpressionsInput<'a> {
  pub module_table: &'a ModuleTable,
  pub stmt_infos: &'a IndexStmtInfos,
  pub symbols: &'a SymbolRefDb,
  pub resolved_exports: &'a ResolvedExports,
  pub normal_export_chains: &'a NormalExportChains,
  pub module_side_effects: &'a ModuleSideEffects,
  pub dynamic_exports: &'a DynamicExports,
}

pub(in crate::stages::link_stage) struct ResolveMemberExpressionsOwned {
  pub cjs_routing: CjsRoutingDraft,
  pub non_splittable_json_defaults: NonSplittableJsonDefaults,
  pub global_constants: GlobalConstantsDraft,
  pub dependencies: ModuleDependenciesDraft,
}

pub(in crate::stages::link_stage) struct MemberExprResolutions {
  slots: IndexVec<ModuleIdx, Option<MemberExprRefResolutionMap>>,
}

impl MemberExprResolutions {
  pub(in crate::stages::link_stage) fn module_count(&self) -> usize {
    self.slots.len()
  }

  pub(in crate::stages::link_stage) fn into_slots(
    self,
  ) -> IndexVec<ModuleIdx, Option<MemberExprRefResolutionMap>> {
    self.slots
  }
}

/// One-call ownership envelope. The driver must destructure this immediately; no pass accepts it.
pub(in crate::stages::link_stage) struct ResolveMemberExpressionsOutput {
  pub resolutions: MemberExprResolutions,
  pub cjs_routing: CjsRoutingFinal,
  pub global_constants: GlobalConstantsDraft,
  pub dependencies: ModuleDependenciesDraft,
}

/// Exact read-only domain for one M invocation. This helper never crosses the pass boundary.
struct MemberResolutionFacts<'a> {
  module_table: &'a ModuleTable,
  symbols: &'a SymbolRefDb,
  resolved_exports: &'a ResolvedExports,
  normal_export_chains: &'a NormalExportChains,
  cjs_routing: &'a CjsRoutingDraft,
  dynamic_exports: &'a DynamicExports,
  side_effects_modules: &'a FxHashSet<ModuleIdx>,
  non_splittable_json_defaults: &'a FxHashSet<SymbolRef>,
}

struct ModuleMemberResolution {
  resolutions: MemberExprRefResolutionMap,
  side_effect_dependencies: Vec<ModuleIdx>,
  written_cjs_exports: Vec<SymbolRef>,
}

fn json_default_canonical_ref(
  input: ResolveMemberExpressionsInput<'_>,
  symbol_ref: SymbolRef,
) -> Option<SymbolRef> {
  let canonical_ref = input.symbols.canonical_ref_for(symbol_ref);
  let is_json_default = input.module_table[canonical_ref.owner].as_normal().is_some_and(|module| {
    std::matches!(module.module_type, ModuleType::Json)
      && input.resolved_exports.get(canonical_ref.owner, "default").is_some_and(|default_export| {
        input.symbols.canonical_ref_for(default_export.symbol_ref) == canonical_ref
      })
  });
  is_json_default.then_some(canonical_ref)
}

fn json_default_made_non_splittable_by(
  input: ResolveMemberExpressionsInput<'_>,
  reference: &SymbolOrMemberExprRef,
) -> Option<SymbolRef> {
  match reference {
    SymbolOrMemberExprRef::Symbol(symbol_ref) => json_default_canonical_ref(input, *symbol_ref),
    SymbolOrMemberExprRef::MemberExpr(member_expr_ref) => {
      if !member_expr_ref.is_write {
        return None;
      }
      match member_expr_ref.object_ref_type {
        MemberExprObjectReferencedType::Default | MemberExprObjectReferencedType::Named => {
          json_default_canonical_ref(input, member_expr_ref.object_ref)
        }
        MemberExprObjectReferencedType::Namespace
          if member_expr_ref
            .prop_and_span_list
            .first()
            .is_some_and(|prop| prop.name.as_str() == "default") =>
        {
          let canonical_ref = input.symbols.canonical_ref_for(member_expr_ref.object_ref);
          let is_json_namespace =
            input.module_table[canonical_ref.owner].as_normal().is_some_and(|module| {
              std::matches!(module.module_type, ModuleType::Json)
                && module.namespace_object_ref == canonical_ref
            });
          if !is_json_namespace {
            return None;
          }
          input
            .resolved_exports
            .get(canonical_ref.owner, "default")
            .map(|export| input.symbols.canonical_ref_for(export.symbol_ref))
        }
        MemberExprObjectReferencedType::Namespace => None,
      }
    }
  }
}

fn collect_non_splittable_json_defaults(
  input: ResolveMemberExpressionsInput<'_>,
  stmt_infos: &StmtInfos,
) -> FxHashSet<SymbolRef> {
  let mut non_splittable = FxHashSet::default();
  for stmt_info in stmt_infos.iter() {
    if stmt_info.meta.contains(StmtInfoMeta::LazyJsonExportInitializer) {
      continue;
    }
    for reference in &stmt_info.referenced_symbols {
      if let Some(json_default) = json_default_made_non_splittable_by(input, reference) {
        non_splittable.insert(json_default);
      }
    }
  }
  non_splittable
}

fn resolve_normal_module(
  facts: &MemberResolutionFacts<'_>,
  module: &NormalModule,
  stmt_infos: &StmtInfos,
  warnings: &append_only_vec::AppendOnlyVec<BuildDiagnostic>,
) -> ModuleMemberResolution {
  let mut resolutions = MemberExprRefResolutionMap::default();
  let mut side_effect_dependencies = Vec::new();
  let mut written_cjs_exports = Vec::new();

  for stmt_info in stmt_infos.iter() {
    'next_reference: for symbol_ref in &stmt_info.referenced_symbols {
      let SymbolOrMemberExprRef::MemberExpr(member_expr_ref) = symbol_ref else { continue };
      let mut depended_refs = Vec::new();
      let mut canonical_ref = facts.symbols.canonical_ref_for(member_expr_ref.object_ref);
      let mut canonical_ref_owner = match &facts.module_table[canonical_ref.owner] {
        Module::Normal(module) => module.as_ref(),
        Module::External(_) => continue,
      };
      // A default-imported JSON object may use the same namespace walk as `import *` only for
      // static reads. Writes and graph-wide mutation/escape facts must keep the live object access;
      // `.default` is also excluded because resolving it would silently erase that property.
      let is_json_import_ns = std::matches!(canonical_ref_owner.module_type, ModuleType::Json)
        && member_expr_ref.object_ref_type == MemberExprObjectReferencedType::Default
        && !member_expr_ref.is_write
        && !facts.non_splittable_json_defaults.contains(&canonical_ref)
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
        let export_symbol =
          facts.resolved_exports.get(canonical_ref_owner.idx, name).and_then(|resolved_export| {
            (!resolved_export.came_from_commonjs).then_some(resolved_export)
          });
        let Some(export_symbol) = export_symbol else {
          if !facts.dynamic_exports.contains(canonical_ref_owner.idx) {
            resolutions.insert(
              member_expr_ref.node_id,
              MemberExprRefResolution {
                resolved: if is_json_import_ns { Some(canonical_ref) } else { None },
                prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..].to_vec(),
                depended_refs: Vec::new(),
                target_commonjs_exported_symbol: None,
                reference_id: member_expr_ref.reference_id,
              },
            );
          }
          if !facts.dynamic_exports.contains(canonical_ref_owner.idx) && !is_json_import_ns {
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
        if !facts.resolved_exports.contains_canonical_name(canonical_ref_owner.idx, name) {
          resolutions.insert(
            member_expr_ref.node_id,
            MemberExprRefResolution {
              resolved: None,
              prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..].to_vec(),
              depended_refs: Vec::new(),
              target_commonjs_exported_symbol: None,
              reference_id: member_expr_ref.reference_id,
            },
          );
          continue 'next_reference;
        }

        depended_refs.push(export_symbol.symbol_ref);
        if let Some(chains) = facts.normal_export_chains.get(&export_symbol.symbol_ref) {
          depended_refs.extend(chains);
          for item in chains {
            if facts.side_effects_modules.contains(&item.owner) {
              side_effect_dependencies.push(item.owner);
            }
          }
        }
        canonical_ref = facts.symbols.canonical_ref_for(export_symbol.symbol_ref);
        // External symbols end static namespace traversal. Remaining properties stay as runtime
        // accesses, exactly as in the legacy resolver.
        let Some(normal_module) = facts.module_table[canonical_ref.owner].as_normal() else {
          cursor += 1;
          break;
        };
        canonical_ref_owner = normal_module;
        cursor += 1;
        is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
      }

      let mut target_commonjs_exported_symbol = None;
      if cursor < member_expr_ref.prop_and_span_list.len() {
        let maybe_namespace = depended_refs
          .last()
          .copied()
          .unwrap_or(facts.symbols.canonical_ref_for(member_expr_ref.object_ref));
        let maybe_namespace_symbol = facts.symbols.get(maybe_namespace);
        // An imported CJS binding can keep resolving only when the namespace alias represents the
        // whole default object. Named aliases already point at a property and must stop here.
        let continue_resolve = maybe_namespace_symbol
          .namespace_alias
          .as_ref()
          .is_none_or(|namespace| namespace.property_name.as_str() == "default");
        let cjs_module_idx = continue_resolve.then(|| {
          facts
            .cjs_routing
            .named_target(maybe_namespace.owner, maybe_namespace)
            .or_else(|| facts.cjs_routing.namespace_target(maybe_namespace.owner, maybe_namespace))
            .or_else(|| {
              facts.dynamic_exports.contains(maybe_namespace.owner).then_some(maybe_namespace.owner)
            })
        });
        if let Some(cjs_idx) = cjs_module_idx.flatten()
          && let Some(cjs_export) = facts
            .resolved_exports
            .get(cjs_idx, &member_expr_ref.prop_and_span_list[cursor].name)
            .and_then(|resolved_export| {
              resolved_export.came_from_commonjs.then_some(resolved_export)
            })
        {
          let is_default = member_expr_ref.prop_and_span_list[cursor].name == "default";
          // In Node ESM mode, or without an `__esModule` flag, `__toESM` makes `.default` the whole
          // `module.exports` object. In that shape the next property, not `default` itself, is the
          // candidate CJS export.
          let default_is_module_exports = is_default && {
            let is_node_esm = module.should_consider_node_esm_spec_for_static_import();
            let importee_has_es_module_flag =
              facts.module_table[cjs_idx].as_normal().is_some_and(|importee| {
                importee.ecma_view.ast_usage.contains(EcmaModuleAstUsage::EsModuleFlag)
              });
            is_node_esm || !importee_has_es_module_flag
          };

          if default_is_module_exports
            && let Some(next_prop) = member_expr_ref.prop_and_span_list.get(cursor + 1)
          {
            if let Some(property) =
              facts.resolved_exports.get(cjs_idx, &next_prop.name).and_then(|resolved_export| {
                resolved_export.came_from_commonjs.then_some(resolved_export)
              })
            {
              let is_next_default = next_prop.name == "default";
              // `import * as ns; ns.default.default` cannot be collapsed: `__copyProps` skips the
              // already-created default property, so the second `.default` is only reachable from
              // the live `module.exports` object. Namespace aliases with an explicit `.default`
              // base do not have this shape.
              if !is_next_default || maybe_namespace_symbol.namespace_alias.is_some() {
                cursor += 1;
                target_commonjs_exported_symbol = Some((property.symbol_ref, is_next_default));
                depended_refs.push(property.symbol_ref);
                if member_expr_ref.is_write {
                  written_cjs_exports.push(property.symbol_ref);
                }
              }
            }
          } else if !default_is_module_exports {
            target_commonjs_exported_symbol = Some((cjs_export.symbol_ref, is_default));
          }
          depended_refs.push(cjs_export.symbol_ref);
          if member_expr_ref.is_write {
            written_cjs_exports.push(cjs_export.symbol_ref);
          }
        }
      }

      if cursor > 0 {
        depended_refs.push(member_expr_ref.object_ref);
        if let Some(refs) = facts.normal_export_chains.get(&member_expr_ref.object_ref) {
          depended_refs.extend(refs);
        }
      }

      if cursor > 0 || target_commonjs_exported_symbol.is_some() {
        resolutions.insert(
          member_expr_ref.node_id,
          MemberExprRefResolution {
            resolved: Some(canonical_ref),
            prop_and_related_span_list: member_expr_ref.prop_and_span_list[cursor..].to_vec(),
            depended_refs,
            target_commonjs_exported_symbol,
            reference_id: member_expr_ref.reference_id,
          },
        );
      }
    }
  }

  ModuleMemberResolution { resolutions, side_effect_dependencies, written_cjs_exports }
}

fn side_effect_modules(module_side_effects: &ModuleSideEffects) -> FxHashSet<ModuleIdx> {
  (0..module_side_effects.module_count())
    .map(ModuleIdx::new)
    .filter(|module_idx| module_side_effects.get(*module_idx).has_side_effects())
    .collect()
}

impl Pass for ResolveMemberExpressionsPass {
  type InputRead<'a> = ResolveMemberExpressionsInput<'a>;
  type InputOwned = ResolveMemberExpressionsOwned;
  type OutputRead = ();
  type OutputOwned = ResolveMemberExpressionsOutput;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let ResolveMemberExpressionsOwned {
      cjs_routing,
      non_splittable_json_defaults,
      mut global_constants,
      mut dependencies,
    } = owned;
    let module_count = input.module_table.modules.len();
    std::assert_eq!(
      input.stmt_infos.len(),
      module_count,
      "statement-info layout must match modules"
    );
    std::assert_eq!(
      input.module_side_effects.module_count(),
      module_count,
      "module-side-effect layout must match modules"
    );
    std::assert_eq!(
      cjs_routing.module_count(),
      module_count,
      "CJS-routing layout must match modules"
    );
    std::assert_eq!(
      input.resolved_exports.module_count(),
      module_count,
      "resolved-export layout must match modules before member resolution"
    );
    std::assert_eq!(
      input.dynamic_exports.module_count(),
      module_count,
      "dynamic-export layout must match modules before member resolution"
    );
    std::assert_eq!(
      dependencies.module_count(),
      module_count,
      "dependency layout must match modules before member resolution"
    );

    let side_effects_modules = side_effect_modules(input.module_side_effects);
    // A JSON default whose `data.key` reads are rewritten to split named exports becomes
    // non-splittable when that object is mutated or escapes anywhere in the graph. Complete the
    // graph-wide union before resolving any reads; otherwise parallel module order could change
    // whether the optimization is applied.
    let mut non_splittable = non_splittable_json_defaults
      .iter()
      .map(|symbol_ref| input.symbols.canonical_ref_for(symbol_ref))
      .collect::<FxHashSet<_>>();
    let has_json_module = input.module_table.modules.iter().any(|module| {
      module.as_normal().is_some_and(|module| std::matches!(module.module_type, ModuleType::Json))
    });
    if has_json_module {
      non_splittable.extend(
        input
          .module_table
          .modules
          .par_iter()
          .zip(input.stmt_infos.par_iter())
          .map(|(module, stmt_infos)| match module {
            Module::Normal(_) => collect_non_splittable_json_defaults(input, stmt_infos),
            Module::External(_) => FxHashSet::default(),
          })
          .collect::<Vec<_>>()
          .into_iter()
          .flatten(),
      );
    }

    let warnings = append_only_vec::AppendOnlyVec::new();
    let facts = MemberResolutionFacts {
      module_table: input.module_table,
      symbols: input.symbols,
      resolved_exports: input.resolved_exports,
      normal_export_chains: input.normal_export_chains,
      cjs_routing: &cjs_routing,
      dynamic_exports: input.dynamic_exports,
      side_effects_modules: &side_effects_modules,
      non_splittable_json_defaults: &non_splittable,
    };
    let module_results = IndexVec::from_vec(
      input
        .module_table
        .modules
        .par_iter()
        .zip(input.stmt_infos.par_iter())
        .map(|(module, stmt_infos)| match module {
          Module::Normal(module) => {
            Some(resolve_normal_module(&facts, module, stmt_infos, &warnings))
          }
          Module::External(_) => None,
        })
        .collect(),
    );
    cx.extend(warnings);

    // Remove written CJS exports from the constant table. Static writes are collected during
    // resolution; `HasComputedMemberWrite` invalidates every known CJS export reached through
    // either named-import or namespace-record routing because the written property is unknown.
    let mut written_cjs_export_symbols = Vec::new();
    for (module_idx, result) in module_results.iter_enumerated() {
      if let Some(result) = result {
        written_cjs_export_symbols.extend(&result.written_cjs_exports);
      }
      for (import_symbol, cjs_module_idx) in cjs_routing.routes_for(module_idx) {
        if import_symbol
          .flags(input.symbols)
          .is_some_and(|flags| flags.contains(SymbolRefFlags::HasComputedMemberWrite))
        {
          written_cjs_export_symbols.extend(
            input
              .resolved_exports
              .iter(*cjs_module_idx)
              .map(|(_, resolved_export)| resolved_export)
              .filter(|resolved_export| resolved_export.came_from_commonjs)
              .map(|resolved_export| resolved_export.symbol_ref),
          );
        }
      }
    }
    for symbol_ref in &written_cjs_export_symbols {
      global_constants.remove(symbol_ref);
    }

    let mut resolutions = IndexVec::with_capacity(module_count);
    for (module_idx, result) in module_results.into_iter_enumerated() {
      if let Some(result) = result {
        dependencies.extend(module_idx, result.side_effect_dependencies);
        resolutions.push(Some(result.resolutions));
      } else {
        resolutions.push(None);
      }
    }

    Ok(token.finish(
      (),
      ResolveMemberExpressionsOutput {
        resolutions: MemberExprResolutions { slots: resolutions },
        cjs_routing: cjs_routing.finalize(),
        global_constants,
        dependencies,
      },
    ))
  }
}

#[cfg(test)]
mod tests {
  use oxc::{semantic::Scoping, span::Span};
  use oxc_index::IndexVec;
  use rolldown_common::{
    ConstExportMeta, ConstantValue, ExportsKind, ImportKind, ImportRecordIdx, LocalExport, Module,
    ModuleTable, NamedImport, Specifier, StmtInfos, SymbolRefDb, SymbolRefDbForModule,
    SymbolRefFlags, side_effects::DeterminedSideEffects,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::super::non_splittable_json_defaults::NonSplittableJsonDefaults;
  use super::super::{
    CollectInitialDependenciesPass, CollectResolvedExportsPass, ComputeCjsRoutingInput,
    ComputeCjsRoutingPass, ConstantExtractionInput, ExtractGlobalConstantsPass,
    FinalizeResolvedExportsPass, ResolveMemberExpressionsInput, ResolveMemberExpressionsOwned,
    ResolveMemberExpressionsPass,
    bind_imports::test_support::empty_normal_export_chains,
    compute_dynamic_exports::test_support::dynamic_exports,
    determine_module_formats::test_support::module_formats,
    determine_module_side_effects::test_support::module_side_effects,
    test_utils::{external_module, module_idx, module_table, normal_module},
  };

  fn symbols_for(modules: &ModuleTable) -> SymbolRefDb {
    let mut symbols = SymbolRefDb::new();
    for (module_idx, module) in modules.modules.iter_enumerated() {
      let scoping = Scoping::default();
      let root_scope_id = scoping.root_scope_id();
      symbols
        .store_local_db(module_idx, SymbolRefDbForModule::new(scoping, module_idx, root_scope_id));
      let namespace_ref = symbols.create_facade_root_symbol_ref(module_idx, "namespace");
      let expected = match module {
        Module::Normal(module) => module.namespace_object_ref,
        Module::External(module) => module.namespace_ref,
      };
      assert_eq!(namespace_ref, expected);
    }
    symbols
  }

  #[test]
  fn consumes_named_routes_and_json_guards_but_retains_namespace_routes() {
    let mut modules = module_table(vec![
      normal_module(0, false, vec![(ImportKind::Import, Some(1), Span::new(1, 2))]),
      normal_module(1, false, Vec::new()),
      external_module(2, "external"),
    ]);
    let mut symbols = symbols_for(&modules);
    let named_import_ref = symbols.create_facade_root_symbol_ref(module_idx(0), "named_import");
    let cjs_export = symbols.create_facade_root_symbol_ref(module_idx(1), "cjs_export");
    modules[module_idx(1)].as_normal_mut().expect("normal CJS module").named_exports.insert(
      "value".into(),
      LocalExport { span: Span::new(10, 11), referenced: cjs_export, came_from_commonjs: true },
    );
    modules[module_idx(1)]
      .as_normal_mut()
      .expect("normal CJS module")
      .constant_export_map
      .insert(cjs_export.symbol, ConstExportMeta::new(ConstantValue::Number(1.0), true));
    let namespace_ref = modules[module_idx(0)].as_normal().expect("normal importer").import_records
      [ImportRecordIdx::from_usize(0)]
    .namespace_ref;
    modules[module_idx(0)].as_normal_mut().expect("normal importer").named_imports.insert(
      named_import_ref,
      NamedImport {
        imported: Specifier::from("value"),
        span_imported: Span::new(1, 2),
        imported_as: named_import_ref,
        record_idx: ImportRecordIdx::from_usize(0),
      },
    );
    named_import_ref.flags_mut(&mut symbols).insert(SymbolRefFlags::HasComputedMemberWrite);

    let mut pipeline = PassPipelineCtx::new();
    let (_, (modules, global_constants)) = run_infallible_pass(
      ExtractGlobalConstantsPass,
      &mut pipeline,
      ConstantExtractionInput { enabled: true },
      modules,
    );
    let (_, dependencies) =
      run_infallible_pass(CollectInitialDependenciesPass, &mut pipeline, &modules, ());
    let (_, resolved_draft) =
      run_infallible_pass(CollectResolvedExportsPass, &mut pipeline, &modules, ());
    let (_, resolved_exports) =
      run_infallible_pass(FinalizeResolvedExportsPass, &mut pipeline, &symbols, resolved_draft);
    let formats = module_formats(&[Some(ExportsKind::Esm), Some(ExportsKind::CommonJs), None]);
    let dynamic_exports = dynamic_exports(3, []);
    let (_, cjs_routing) = run_infallible_pass(
      ComputeCjsRoutingPass,
      &mut pipeline,
      ComputeCjsRoutingInput {
        module_table: &modules,
        module_formats: &formats,
        dynamic_exports: &dynamic_exports,
      },
      (),
    );
    assert_eq!(cjs_routing.named_target(module_idx(0), named_import_ref), Some(module_idx(1)));
    assert_eq!(cjs_routing.namespace_target(module_idx(0), namespace_ref), Some(module_idx(1)));
    let stmt_infos = modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();
    let side_effects = module_side_effects(&[
      DeterminedSideEffects::Analyzed(false),
      DeterminedSideEffects::Analyzed(false),
      DeterminedSideEffects::Analyzed(false),
    ]);
    let normal_export_chains = empty_normal_export_chains();

    let (_, output) = run_infallible_pass(
      ResolveMemberExpressionsPass,
      &mut pipeline,
      ResolveMemberExpressionsInput {
        module_table: &modules,
        stmt_infos: &stmt_infos,
        symbols: &symbols,
        resolved_exports: &resolved_exports,
        normal_export_chains: &normal_export_chains,
        module_side_effects: &side_effects,
        dynamic_exports: &dynamic_exports,
      },
      ResolveMemberExpressionsOwned {
        cjs_routing,
        non_splittable_json_defaults: NonSplittableJsonDefaults::default(),
        global_constants,
        dependencies,
      },
    );

    let resolution_slots = output.resolutions.into_slots();
    assert!(resolution_slots[module_idx(0)].as_ref().expect("normal slot").is_empty());
    assert!(resolution_slots[module_idx(1)].as_ref().expect("normal slot").is_empty());
    assert!(resolution_slots[module_idx(2)].is_none());
    let final_routes = output.cjs_routing.into_importers();
    assert_eq!(final_routes[&module_idx(0)].get(&namespace_ref), Some(&module_idx(1)));
    assert_eq!(final_routes.len(), 1);
    assert!(!output.global_constants.into_legacy().contains_key(&cjs_export));
    assert_eq!(
      output.dependencies.into_inner()[module_idx(0)].iter().copied().collect::<Vec<_>>(),
      [module_idx(1)]
    );
    assert!(pipeline.into_diagnostics().is_empty());
  }
}
