use crate::types::symbols::Symbols;
use oxc_index::IndexVec;
use rolldown_common::side_effects::DeterminedSideEffects;
use rolldown_common::{NormalModule, NormalModuleId, NormalModuleVec, StmtInfoId, SymbolRef};
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};

use super::LinkStage;

struct Context<'a> {
  modules: &'a NormalModuleVec,
  symbols: &'a Symbols,
  is_included_vec: &'a mut IndexVec<NormalModuleId, IndexVec<StmtInfoId, bool>>,
  is_module_included_vec: &'a mut IndexVec<NormalModuleId, bool>,
  tree_shaking: bool,
  runtime_id: NormalModuleId,
}

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
  if is_included {
    return;
  }
  ctx.is_module_included_vec[module.id] = true;

  if module.id == ctx.runtime_id {
    // runtime module has no side effects and it's statements should be included
    // by other modules's references.
    return;
  }

  if ctx.tree_shaking {
    let forced_no_treeshake = matches!(module.side_effects, DeterminedSideEffects::NoTreeshake);
    if forced_no_treeshake {
      forcefully_include_all_statements(ctx, module);
    } else {
      module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, stmt_info)| {
        // No need to handle the first statement specially, which is the namespace object, because it doesn't have side effects and will only be included if it is used.
        if stmt_info.side_effect {
          include_statement(ctx, module, stmt_info_id);
        }
      });
    }
  } else {
    forcefully_include_all_statements(ctx, module);
  }

  // Include imported modules for its side effects
  module.import_records.iter().for_each(|import_record| match import_record.resolved_module {
    rolldown_common::ModuleId::Normal(importee_id) => {
      let importee = &ctx.modules[importee_id];
      if !ctx.tree_shaking || importee.side_effects.has_side_effects() {
        include_module(ctx, importee);
      }
    }
    rolldown_common::ModuleId::External(_) => {}
  });
}

fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
  let mut canonical_ref = ctx.symbols.par_canonical_ref_for(symbol_ref);
  let canonical_ref_module = &ctx.modules[canonical_ref.owner];
  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  if let Some(namespace_alias) = &canonical_ref_symbol.namespace_alias {
    canonical_ref = namespace_alias.namespace_ref;
  }
  include_module(ctx, canonical_ref_module);
  canonical_ref_module
    .stmt_infos
    .declared_stmts_by_symbol(&canonical_ref)
    .iter()
    .copied()
    .for_each(|stmt_info_id| {
      include_statement(ctx, canonical_ref_module, stmt_info_id);
    });
}

fn include_statement(ctx: &mut Context, module: &NormalModule, stmt_info_id: StmtInfoId) {
  let is_included = &mut ctx.is_included_vec[module.id][stmt_info_id];

  if *is_included {
    return;
  }

  let stmt_info = module.stmt_infos.get(stmt_info_id);

  // include the statement itself
  *is_included = true;

  // include statements that are referenced by this statement
  stmt_info.declared_symbols.iter().chain(stmt_info.referenced_symbols.iter()).for_each(
    |symbol_ref| {
      // Notice we also include `declared_symbols`. This for case that import statements declare new symbols, but they are not
      // really declared by the module itself. We need to include them where they are really declared.
      include_symbol(ctx, *symbol_ref);
    },
  );
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
      oxc_index::index_vec![false; self.module_table.normal_modules.len()];

    let context = &mut Context {
      modules: &self.module_table.normal_modules,
      symbols: &self.symbols,
      is_included_vec: &mut is_included_vec,
      is_module_included_vec: &mut is_module_included_vec,
      tree_shaking: self.input_options.treeshake,
      runtime_id: self.runtime.id(),
    };

    self.entries.iter().for_each(|entry| {
      let module = &self.module_table.normal_modules[entry.id];

      include_module(context, module);
    });

    self.module_table.normal_modules.iter_mut().par_bridge().for_each(|module| {
      module.is_included = is_module_included_vec[module.id];
      is_included_vec[module.id].iter_enumerated().for_each(|(stmt_info_id, is_included)| {
        module.stmt_infos.get_mut(stmt_info_id).is_included = *is_included;
      });
    });

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
      oxc_index::index_vec![None; self.module_table.normal_modules.len()];
    let index_module_side_effects = self
      .module_table
      .normal_modules
      .iter()
      .map(|module| {
        let mut visited: IndexVisited =
          oxc_index::index_vec![false; self.module_table.normal_modules.len()];
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
