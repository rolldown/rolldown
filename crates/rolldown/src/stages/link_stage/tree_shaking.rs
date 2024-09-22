use crate::types::linking_metadata::LinkingMetadataVec;
use crate::types::symbols::Symbols;
use oxc::index::IndexVec;
use rolldown_common::side_effects::DeterminedSideEffects;
use rolldown_common::{
  IndexModules, Module, ModuleIdx, ModuleType, NormalModule, StmtInfoIdx, SymbolOrMemberExprRef,
  SymbolRef,
};
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use rustc_hash::FxHashSet;

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
}

/// if no export is used, and the module has no side effects, the module should not be included
fn include_module(ctx: &mut Context, module: &NormalModule) {
  fn forcefully_include_all_statements(ctx: &mut Context, module: &NormalModule) {
    // Skip the first statement, which is the namespace object. It should be included only if it is used no matter
    // tree shaking is enabled or not.
    module.stmt_infos.iter_enumerated().skip(1).for_each(|(stmt_info_id, _stmt_info)| {
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
      let bail_eval =
        module.has_eval && !stmt_info.declared_symbols.is_empty() && stmt_info_id.index() != 0;
      if stmt_info.side_effect || bail_eval {
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
      Module::Normal(importee) => {
        if !ctx.tree_shaking || importee.side_effects.has_side_effects() {
          include_module(ctx, importee);
        }
      }
      Module::External(_) => {}
    }
  });
  if module.has_eval && matches!(module.module_type, ModuleType::Js | ModuleType::Jsx) {
    module.named_imports.keys().for_each(|symbol| {
      include_symbol(ctx, *symbol);
    });
  }
}

fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
  let mut canonical_ref = ctx.symbols.par_canonical_ref_for(symbol_ref);
  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  let mut canonical_ref_owner = ctx.modules[canonical_ref.owner].as_normal().unwrap();
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    canonical_ref = namespace_alias.namespace_ref;
    canonical_ref_owner = ctx.modules[canonical_ref.owner].as_normal().unwrap();
  }

  ctx.used_symbol_refs.insert(canonical_ref);

  include_module(ctx, canonical_ref_owner);
  canonical_ref_owner.stmt_infos.declared_stmts_by_symbol(&canonical_ref).iter().copied().for_each(
    |stmt_info_id| {
      include_statement(ctx, canonical_ref_owner, stmt_info_id);
    },
  );
}

fn include_statement(ctx: &mut Context, module: &NormalModule, stmt_info_id: StmtInfoIdx) {
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
      if let Some(symbol) =
        member_expr.resolved_symbol_ref(&ctx.metas[module.idx].resolved_member_expr_refs)
      {
        include_symbol(ctx, symbol);
      }
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
        m.as_normal().map_or(IndexVec::default(), |m| {
          m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoIdx, _>>()
        })
      })
      .collect::<IndexVec<ModuleIdx, _>>();

    let mut is_module_included_vec: IndexVec<ModuleIdx, bool> =
      oxc::index::index_vec![false; self.module_table.modules.len()];

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
    };

    self.entries.iter().for_each(|entry| {
      let module = match &self.module_table.modules[entry.id] {
        Module::Normal(module) => module,
        Module::External(_module) => {
          // Case: import('external').
          return;
        }
      };
      let meta = &self.metas[entry.id];
      meta.referenced_symbols_by_entry_point_chunk.iter().for_each(|symbol_ref| {
        include_symbol(context, *symbol_ref);
      });
      include_module(context, module);
    });

    self.module_table.modules.par_iter_mut().filter_map(Module::as_normal_mut).for_each(|module| {
      module.is_included = is_module_included_vec[module.idx];
      is_included_vec[module.idx].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
        module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
      });
    });

    tracing::trace!(
      "included statements {:#?}",
      self
        .module_table
        .modules
        .iter()
        .filter_map(Module::as_normal)
        .map(NormalModule::to_debug_normal_module_for_tree_shaking)
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
          Module::Normal(module) => {
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
        if let Module::Normal(module) = module {
          module.side_effects = side_effects;
        }
      },
    );
  }
}
