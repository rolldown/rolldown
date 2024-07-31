use crate::types::linking_metadata::LinkingMetadataVec;
use crate::types::symbols::Symbols;
use oxc::index::IndexVec;
// use crate::utils::extract_member_chain::extract_canonical_symbol_info;
use oxc::span::CompactStr;
use rolldown_common::side_effects::DeterminedSideEffects;
use rolldown_common::{
  EcmaModule, IndexModules, Module, ModuleIdx, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef,
};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use super::LinkStage;

struct Context<'a> {
  modules: &'a IndexModules,
  symbols: &'a Symbols,
  is_included_vec: &'a mut IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>>,
  is_module_included_vec: &'a mut IndexVec<ModuleIdx, bool>,
  tree_shaking: bool,
  runtime_id: ModuleIdx,
  metas: &'a LinkingMetadataVec,
  used_symbol_refs: &'a mut FxHashSet<SymbolRef>,
  /// Hash list of string is relatively slow, so we use a two dimensions hashmap to cache the resolved symbol.
  /// The first level only store the top level namespace member expr object identifier symbol ref.
  /// With this method, we could avoid the hash calculation of the whole member expr chains.
  /// for the value, the first element is the resolved symbol ref, the second element is how much
  /// chain element does this member expr consume.
  top_level_member_expr_resolved_cache:
    &'a mut FxHashMap<SymbolRef, MemberChainToResolvedSymbolRef>,
}

pub type MemberChainToResolvedSymbolRef =
  FxHashMap<Box<[CompactStr]>, (SymbolRef, usize, Option<Rstr>)>;

/// if no export is used, and the module has no side effects, the module should not be included
fn include_module(ctx: &mut Context, module: &EcmaModule) {
  fn forcefully_include_all_statements(ctx: &mut Context, module: &EcmaModule) {
    module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, _stmt_info)| {
      // Skip the first statement, which is the namespace object. It should be included only if it is used no matter
      // tree shaking is enabled or not.
      if stmt_info_id.index() == 0 {
        return;
      }
      include_statement(ctx, module, stmt_info_id);
    });
  }

  let is_included = ctx.is_module_included_vec[module.idx];
  if is_included {
    return;
  }
  ctx.is_module_included_vec[module.idx] = true;

  if module.idx == ctx.runtime_id {
    // runtime module has no side effects and it's statements should be included
    // by other modules's references.
    return;
  }

  let forced_no_treeshake = matches!(module.side_effects, DeterminedSideEffects::NoTreeshake);
  if ctx.tree_shaking && !forced_no_treeshake {
    module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, stmt_info)| {
      // No need to handle the first statement specially, which is the namespace object, because it doesn't have side effects and will only be included if it is used.
      if stmt_info.side_effect {
        include_statement(ctx, module, stmt_info_id);
      }
    });
  } else {
    forcefully_include_all_statements(ctx, module);
  }

  let module_meta = &ctx.metas[module.idx];

  // Include imported modules for its side effects
  module_meta.dependencies.iter().copied().for_each(|dependency_idx| {
    match &ctx.modules[dependency_idx] {
      Module::Ecma(importee) => {
        if !ctx.tree_shaking || importee.side_effects.has_side_effects() {
          include_module(ctx, importee);
        }
      }
      Module::External(_) => {}
    }
  });
}

fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
  let mut canonical_ref = ctx.symbols.par_canonical_ref_for(symbol_ref);
  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  let mut canonical_ref_owner = ctx.modules[canonical_ref.owner].as_ecma().unwrap();
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    canonical_ref = namespace_alias.namespace_ref;
    canonical_ref_owner = ctx.modules[canonical_ref.owner].as_ecma().unwrap();
  }

  ctx.used_symbol_refs.insert(canonical_ref);

  include_module(ctx, canonical_ref_owner);
  canonical_ref_owner.stmt_infos.declared_stmts_by_symbol(&canonical_ref).iter().copied().for_each(
    |stmt_info_id| {
      include_statement(ctx, canonical_ref_owner, stmt_info_id);
    },
  );
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
fn include_member_expr_ref(ctx: &mut Context, symbol_ref: SymbolRef, props: &[CompactStr]) {
  let mut cursor = 0;

  // First get the canonical ref of `foo_ns`, then we get the `NormalModule#namespace_object_ref` of `foo.js`.
  let mut canonical_ref = ctx.symbols.par_canonical_ref_for(symbol_ref);
  let mut canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  let mut canonical_ref_owner = ctx.modules[canonical_ref.owner].as_ecma().unwrap();
  let mut is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
  let mut ns_symbol_list = vec![];
  let mut has_ambiguous_symbol = false;

  while cursor < props.len() && is_namespace_ref {
    let name = &props[cursor];
    let meta = &ctx.metas[canonical_ref_owner.idx];
    let export_symbol = meta.resolved_exports.get(&name.to_rstr());
    let Some(export_symbol) = export_symbol else { break };
    has_ambiguous_symbol |=
      !meta.sorted_and_non_ambiguous_resolved_exports.contains(&name.to_rstr());
    // TODO(hyf0): suspicious cjs might just fallback to dynamic lookup?
    if !ctx.modules[export_symbol.symbol_ref.owner].as_ecma().unwrap().exports_kind.is_esm() {
      break;
    }
    ns_symbol_list.push((canonical_ref, name.to_rstr()));
    canonical_ref = ctx.symbols.par_canonical_ref_for(export_symbol.symbol_ref);
    canonical_ref_symbol = ctx.symbols.get(canonical_ref);
    canonical_ref_owner = ctx.modules[canonical_ref.owner].as_ecma().unwrap();
    cursor += 1;
    is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
  }

  let (_export_name, namespace_property_name) =
    if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
      let name = canonical_ref_symbol.name.clone();
      canonical_ref = namespace_alias.namespace_ref;
      canonical_ref_owner = ctx.modules[canonical_ref.owner].as_ecma().unwrap();
      (name, Some(namespace_alias.property_name.clone()))
    } else {
      (canonical_ref_symbol.name.clone(), None)
    };

  // Only cache the top level member expr resolved result, if it consume at least one chain element.
  if cursor > 0 {
    let map = ctx.top_level_member_expr_resolved_cache.entry(symbol_ref).or_default();
    let chains = props.to_vec();
    // If the last namespace object is a namespace alias, we should add the property name postfix
    // to the final access chains.
    map.insert(chains.into_boxed_slice(), (canonical_ref, cursor, namespace_property_name));
  }
  // https://github.com/rolldown/rolldown/blob/5fb31d0d254128825df9441b23da58e3f6663060/crates/rolldown/tests/esbuild/import_star/import_export_star_ambiguous_warning/entry.js#L2-L2
  if has_ambiguous_symbol {
    return;
  }
  ctx.used_symbol_refs.insert(canonical_ref);
  include_module(ctx, canonical_ref_owner);
  canonical_ref_owner.stmt_infos.declared_stmts_by_symbol(&canonical_ref).iter().copied().for_each(
    |stmt_info_id| {
      include_statement(ctx, canonical_ref_owner, stmt_info_id);
    },
  );
}

fn include_statement(ctx: &mut Context, module: &EcmaModule, stmt_info_id: StmtInfoIdx) {
  let is_included = &mut ctx.is_included_vec[module.idx][stmt_info_id];

  if *is_included {
    return;
  }

  let stmt_info = module.stmt_infos.get(stmt_info_id);

  // include the statement itself
  *is_included = true;

  stmt_info.referenced_symbols.iter().for_each(|reference_ref| match reference_ref {
    SymbolOrMemberExprRef::Symbol(symbol_ref) => {
      include_symbol(ctx, *symbol_ref);
    }
    SymbolOrMemberExprRef::MemberExpr(member_expr) => {
      include_member_expr_ref(ctx, member_expr.object_ref, &member_expr.props);
    }
  });
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn include_statements(&mut self) {
    self.determine_side_effects();

    let mut is_included_vec: IndexVec<ModuleIdx, IndexVec<StmtInfoIdx, bool>> = self
      .module_table
      .modules
      .iter()
      .map(|m| {
        m.as_ecma().map_or(IndexVec::default(), |m| {
          m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoIdx, _>>()
        })
      })
      .collect::<IndexVec<ModuleIdx, _>>();

    let mut is_module_included_vec: IndexVec<ModuleIdx, bool> =
      oxc::index::index_vec![false; self.module_table.modules.len()];

    let mut top_level_member_expr_resolved_cache = FxHashMap::default();
    let context = &mut Context {
      modules: &self.module_table.modules,
      symbols: &self.symbols,
      is_included_vec: &mut is_included_vec,
      is_module_included_vec: &mut is_module_included_vec,
      tree_shaking: self.options.treeshake.enabled(),
      runtime_id: self.runtime.id(),
      // used_exports_info_vec: &mut used_exports_info_vec,
      metas: &self.metas,
      used_symbol_refs: &mut self.used_symbol_refs,
      top_level_member_expr_resolved_cache: &mut top_level_member_expr_resolved_cache,
    };

    self.entries.iter().for_each(|entry| {
      let module = match &self.module_table.modules[entry.id] {
        Module::Ecma(module) => module,
        Module::External(_module) => {
          // Case: import('external').
          return;
        }
      };
      let meta = &self.metas[entry.id];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
        include_symbol(context, *symbol_ref);
      });
      // module.named_exports.iter().for_each(|(name, _)| {
      //   context.used_exports_info_vec[entry.id].used_exports.insert(name.clone());
      // });
      include_module(context, module);
    });

    self.module_table.modules.iter_mut().par_bridge().filter_map(Module::as_ecma_mut).for_each(
      |module| {
        module.is_included = is_module_included_vec[module.idx];
        is_included_vec[module.idx].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
          module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
        });
      },
    );

    self.top_level_member_expr_resolved_cache = top_level_member_expr_resolved_cache;

    tracing::trace!(
      "included statements {:#?}",
      self
        .module_table
        .modules
        .iter()
        .filter_map(Module::as_ecma)
        .map(EcmaModule::to_debug_normal_module_for_tree_shaking)
        .collect::<Vec<_>>()
    );
  }

  fn determine_side_effects(&mut self) {
    #[derive(Debug, Clone, Copy)]
    enum SideEffectCache {
      None,
      Visited,
      Cache(DeterminedSideEffects),
    }
    type IndexSideEffectsCache = IndexVec<ModuleIdx, SideEffectCache>;

    fn determine_side_effects_for_module(
      cache: &mut IndexSideEffectsCache,
      module_id: ModuleIdx,
      normal_modules: &IndexModules,
    ) -> DeterminedSideEffects {
      let module = &normal_modules[module_id];

      match &mut cache[module_id] {
        SideEffectCache::None => {
          cache[module_id] = SideEffectCache::Visited;
        }
        SideEffectCache::Visited => {
          return *module.side_effects();
        }
        SideEffectCache::Cache(v) => {
          return *v;
        }
      }

      let ret = match *module.side_effects() {
        // should keep as is if the side effects is derived from package.json, it is already
        // true or `no-treeshake`
        DeterminedSideEffects::UserDefined(_) | DeterminedSideEffects::NoTreeshake => {
          *module.side_effects()
        }
        DeterminedSideEffects::Analyzed(v) if v => *module.side_effects(),
        // this branch means the side effects of the module is analyzed `false`
        DeterminedSideEffects::Analyzed(_) => match module {
          Module::Ecma(module) => {
            DeterminedSideEffects::Analyzed(module.import_records.iter().any(|import_record| {
              determine_side_effects_for_module(
                cache,
                import_record.resolved_module,
                normal_modules,
              )
              .has_side_effects()
            }))
          }
          Module::External(module) => module.side_effects,
        },
      };

      cache[module_id] = SideEffectCache::Cache(ret);

      ret
    }

    let mut index_side_effects_cache =
      oxc::index::index_vec![SideEffectCache::None; self.module_table.modules.len()];
    let index_module_side_effects = self
      .module_table
      .modules
      .iter()
      .map(|module| {
        determine_side_effects_for_module(
          &mut index_side_effects_cache,
          module.idx(),
          &self.module_table.modules,
        )
      })
      .collect::<Vec<_>>();

    self.module_table.modules.iter_mut().zip(index_module_side_effects).for_each(
      |(module, side_effects)| {
        if let Module::Ecma(module) = module {
          module.side_effects = side_effects;
        }
      },
    );
  }
}
