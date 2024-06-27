use crate::types::linking_metadata::LinkingMetadataVec;
use crate::types::symbols::Symbols;
use crate::types::tree_shake::{UsedExportsInfo, UsedInfo};
use oxc::index::IndexVec;
// use crate::utils::extract_member_chain::extract_canonical_symbol_info;
use oxc::span::CompactStr;
use rolldown_common::side_effects::DeterminedSideEffects;
use rolldown_common::{
  NormalModule, NormalModuleId, NormalModuleVec, StmtInfoId, SymbolOrMemberExprRef, SymbolRef,
};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use super::LinkStage;

struct Context<'a> {
  modules: &'a NormalModuleVec,
  symbols: &'a Symbols,
  is_included_vec: &'a mut IndexVec<NormalModuleId, IndexVec<StmtInfoId, bool>>,
  is_module_included_vec: &'a mut IndexVec<NormalModuleId, bool>,
  used_exports_info_vec: &'a mut IndexVec<NormalModuleId, UsedExportsInfo>,
  tree_shaking: bool,
  runtime_id: NormalModuleId,
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
fn include_module(ctx: &mut Context, module: &NormalModule) {
  fn forcefully_include_all_statements(ctx: &mut Context, module: &NormalModule) {
    module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, _stmt_info)| {
      // Skip the first statement, which is the namespace object. It should be included only if it is used no matter
      // tree shaking is enabled or not.
      if stmt_info_id.index() == 0 {
        return;
      }
      include_statement(ctx, module, stmt_info_id);
    });
  }

  let is_included = ctx.is_module_included_vec[module.id];
  let used_info = ctx.used_exports_info_vec[module.id].used_info;
  if used_info.contains(UsedInfo::USED_AS_NAMESPACE)
    && !used_info.contains(UsedInfo::INCLUDED_AS_NAMESPACE)
  {
    ctx.used_exports_info_vec[module.id].used_info |= UsedInfo::INCLUDED_AS_NAMESPACE;
    include_module_as_namespace(ctx, module);
  }
  if is_included {
    return;
  }
  ctx.is_module_included_vec[module.id] = true;

  if module.id == ctx.runtime_id {
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

  // Include imported modules for its side effects
  module.import_records.iter().for_each(|import_record| match import_record.resolved_module {
    rolldown_common::ModuleId::Normal(importee_id) => {
      let importee = &ctx.modules[importee_id];
      let bailout_side_effect = matches!(import_record.kind, rolldown_common::ImportKind::Require)
        || importee.def_format.is_commonjs();
      if bailout_side_effect {
        ctx.used_exports_info_vec[importee_id].used_info |= UsedInfo::USED_AS_NAMESPACE;
      }
      if !ctx.tree_shaking || importee.side_effects.has_side_effects() || bailout_side_effect {
        include_module(ctx, importee);
      }
    }
    rolldown_common::ModuleId::External(_) => {}
  });
}

// TODO(hyf0): suspicious namespace should be include normally
fn include_module_as_namespace(ctx: &mut Context, module: &NormalModule) {
  // Collect all the canonical export to avoid violating rustc borrow rules.
  let canonical_export_list = ctx.metas[module.id]
    .canonical_exports()
    .map(|(key, export)| (key.clone(), export.symbol_ref))
    .collect::<Vec<_>>();
  canonical_export_list.into_iter().for_each(|(key, symbol_ref)| {
    ctx.used_exports_info_vec[module.id].used_exports.insert(key);
    include_symbol(ctx, symbol_ref);
  });
}

fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
  let mut canonical_ref = ctx.symbols.par_canonical_ref_for(symbol_ref);
  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  let canonical_ref_owner = &ctx.modules[canonical_ref.owner];
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    canonical_ref = namespace_alias.namespace_ref;
  }

  // TODO(hyf0): suspicious why need `USED_AS_NAMESPACE`
  let is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
  if is_namespace_ref {
    ctx.used_exports_info_vec[canonical_ref_owner.id].used_info |= UsedInfo::USED_AS_NAMESPACE;
  }

  // TODO(hyf0): why we need `used_symbol_refs` to make if the symbol is used?
  ctx.used_symbol_refs.insert(canonical_ref);

  // ---

  include_module(ctx, canonical_ref_owner);
  canonical_ref_owner.stmt_infos.declared_stmts_by_symbol(&canonical_ref).iter().copied().for_each(
    |stmt_info_id| {
      include_statement(ctx, canonical_ref_owner, stmt_info_id);
    },
  );
}

fn include_member_expr_ref(ctx: &mut Context, symbol_ref: SymbolRef, props: &[CompactStr]) {
  // Try to find the final pointed `SymbolRef` of the member expression.
  // ```js
  // // index.js
  // import * as foo_ns from './foo';
  // foo_ns.bar_ns.c;
  // // foo.js
  // export * as bar_ns from './bar';
  // // bar.js
  // export const c = 1;
  // ```
  // The final pointed `SymbolRef` of `foo_ns.bar_ns.c` is the `c` in `bar.js`.

  let mut cursor = 0;

  // First get the canonical ref of `foo_ns`, then we get the `NormalModule#namespace_object_ref` of `foo.js`.
  let mut canonical_ref = ctx.symbols.par_canonical_ref_for(symbol_ref);
  let mut canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  let mut canonical_ref_owner = &ctx.modules[canonical_ref.owner];
  let is_same_ref = canonical_ref == symbol_ref;
  let is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
  let mut ns_symbol_list = vec![];
  let mut has_ambiguous_symbol = false;

  while cursor < props.len() && is_namespace_ref {
    let name = &props[cursor];
    let export_symbol = ctx.metas[canonical_ref_owner.id].resolved_exports.get(&name.to_rstr());
    let Some(export_symbol) = export_symbol else { break };
    // TODO(hyf0): suspicious
    has_ambiguous_symbol |= export_symbol.potentially_ambiguous_symbol_refs.is_some();
    // TODO(hyf0): suspicious cjs might just fallback to dynamic lookup?
    if !ctx.modules[export_symbol.symbol_ref.owner].exports_kind.is_esm() {
      break;
    }
    ns_symbol_list.push((canonical_ref, name.to_rstr()));
    canonical_ref = ctx.symbols.par_canonical_ref_for(export_symbol.symbol_ref);
    canonical_ref_symbol = ctx.symbols.get(canonical_ref);
    canonical_ref_owner = &ctx.modules[canonical_ref.owner];
    cursor += 1;
    // TODO(hyf0): suspicious `is_namespace_ref` doesn't get updated
  }

  let (export_name, namespace_property_name) =
    if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
      let name = canonical_ref_symbol.name.clone();
      canonical_ref = namespace_alias.namespace_ref;
      canonical_ref_owner = &ctx.modules[canonical_ref.owner];
      (name, Some(namespace_alias.property_name.clone()))
    } else {
      (canonical_ref_symbol.name.clone(), None)
    };

  let export_name = export_name.to_rstr();
  let is_namespace_ref = canonical_ref_owner.namespace_object_ref == canonical_ref;
  // TODO(hyf0): suspicious what does `is_same_ref` do?
  if is_namespace_ref && !is_same_ref {
    ctx.used_exports_info_vec[canonical_ref_owner.id].used_info |= UsedInfo::USED_AS_NAMESPACE;
  }

  let id = ns_symbol_list.last().map_or(symbol_ref.owner, |(symbol, _)| symbol.owner);
  ctx.used_exports_info_vec[id].used_exports.insert(export_name);
  // Only cache the top level member expr resolved result, if it consume at least one chain element.
  if cursor > 0 {
    let map = ctx.top_level_member_expr_resolved_cache.entry(symbol_ref).or_default();
    let chains = props.to_vec();
    // If the last namespace object is a namespace alias, we should add the property name postfix
    // to the final access chains.
    map.insert(chains.into_boxed_slice(), (canonical_ref, cursor, namespace_property_name));
  }
  // TODO(hyf0): suspicious
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

fn include_statement(ctx: &mut Context, module: &NormalModule, stmt_info_id: StmtInfoId) {
  let is_included = &mut ctx.is_included_vec[module.id][stmt_info_id];

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

    let mut is_included_vec: IndexVec<NormalModuleId, IndexVec<StmtInfoId, bool>> = self
      .module_table
      .normal_modules
      .iter()
      .map(|m| m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoId, _>>())
      .collect::<IndexVec<NormalModuleId, _>>();

    let mut is_module_included_vec: IndexVec<NormalModuleId, bool> =
      oxc::index::index_vec![false; self.module_table.normal_modules.len()];

    let mut used_exports_info_vec: IndexVec<NormalModuleId, UsedExportsInfo> =
      oxc::index::index_vec![UsedExportsInfo::default(); self.module_table.normal_modules.len()];
    let mut top_level_member_expr_resolved_cache = FxHashMap::default();
    let context = &mut Context {
      modules: &self.module_table.normal_modules,
      symbols: &self.symbols,
      is_included_vec: &mut is_included_vec,
      is_module_included_vec: &mut is_module_included_vec,
      tree_shaking: self.input_options.treeshake.enabled(),
      runtime_id: self.runtime.id(),
      used_exports_info_vec: &mut used_exports_info_vec,
      metas: &self.metas,
      used_symbol_refs: &mut self.used_symbol_refs,
      top_level_member_expr_resolved_cache: &mut top_level_member_expr_resolved_cache,
    };

    self.entries.iter().for_each(|entry| {
      let module = &self.module_table.normal_modules[entry.id];
      let meta = &self.metas[entry.id];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
        include_symbol(context, *symbol_ref);
      });
      module.named_exports.iter().for_each(|(name, _)| {
        context.used_exports_info_vec[entry.id].used_exports.insert(name.clone());
      });
      include_module(context, module);
    });

    self.module_table.normal_modules.iter_mut().par_bridge().for_each(|module| {
      module.is_included = is_module_included_vec[module.id];
      is_included_vec[module.id].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
        module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
      });
    });

    self.module_table.normal_modules.iter_mut().for_each(|module| {
      self.metas[module.id].used_exports_info =
        std::mem::take(&mut used_exports_info_vec[module.id]);
    });

    self.top_level_member_expr_resolved_cache = top_level_member_expr_resolved_cache;

    tracing::trace!(
      "included statements {:#?}",
      self
        .module_table
        .normal_modules
        .iter()
        .map(NormalModule::to_debug_normal_module_for_tree_shaking)
        .collect::<Vec<_>>()
    );
  }

  fn determine_side_effects(&mut self) {
    type IndexVisited = IndexVec<NormalModuleId, bool>;
    type IndexSideEffectsCache = IndexVec<NormalModuleId, Option<DeterminedSideEffects>>;

    fn determine_side_effects_for_module(
      visited: &mut IndexVisited,
      cache: &mut IndexSideEffectsCache,
      module_id: NormalModuleId,
      normal_modules: &NormalModuleVec,
    ) -> DeterminedSideEffects {
      let module = &normal_modules[module_id];

      let is_visited = &mut visited[module_id];

      if *is_visited {
        return module.side_effects;
      }

      *is_visited = true;

      if let Some(ret) = cache[module_id] {
        return ret;
      }

      let ret = match module.side_effects {
        // should keep as is if the side effects is derived from package.json, it is already
        // true or `no-treeshake`
        DeterminedSideEffects::UserDefined(_) | DeterminedSideEffects::NoTreeshake => {
          module.side_effects
        }
        DeterminedSideEffects::Analyzed(v) if v => module.side_effects,
        // this branch means the side effects of the module is analyzed `false`
        DeterminedSideEffects::Analyzed(_) => {
          let has_side_effects_in_dep =
            module.import_records.iter().any(|import_record| match import_record.resolved_module {
              rolldown_common::ModuleId::Normal(importee_id) => {
                determine_side_effects_for_module(visited, cache, importee_id, normal_modules)
                  .has_side_effects()
              }
              rolldown_common::ModuleId::External(_) => {
                // External module is currently treated as always having side effects, but
                // it's ensured by `render_chunk_imports`. So here we consider it as no side effects.
                DeterminedSideEffects::Analyzed(false).has_side_effects()
              }
            });
          DeterminedSideEffects::Analyzed(has_side_effects_in_dep)
        }
      };

      cache[module_id] = Some(ret);

      ret
    }

    let mut index_side_effects_cache =
      oxc::index::index_vec![None; self.module_table.normal_modules.len()];
    let index_module_side_effects = self
      .module_table
      .normal_modules
      .iter()
      .map(|module| {
        let mut visited: IndexVisited =
          oxc::index::index_vec![false; self.module_table.normal_modules.len()];
        determine_side_effects_for_module(
          &mut visited,
          &mut index_side_effects_cache,
          module.id,
          &self.module_table.normal_modules,
        )
      })
      .collect::<Vec<_>>();

    self.module_table.normal_modules.iter_mut().zip(index_module_side_effects).for_each(
      |(module, side_effects)| {
        module.side_effects = side_effects;
      },
    );
  }
}
