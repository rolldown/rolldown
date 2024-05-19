use crate::types::symbols::Symbols;
use oxc_index::IndexVec;
use rolldown_common::{NormalModule, NormalModuleId, NormalModuleVec, StmtInfoId, SymbolRef};
use rolldown_utils::rayon::{ParallelBridge, ParallelIterator};

use super::LinkStage;

struct Context<'a> {
  modules: &'a NormalModuleVec,
  symbols: &'a Symbols,
  is_included_vec: &'a mut IndexVec<NormalModuleId, IndexVec<StmtInfoId, bool>>,
  is_module_included_vec: &'a mut IndexVec<NormalModuleId, bool>,
  has_export_used: &'a mut IndexVec<NormalModuleId, bool>,
  module_side_effects: &'a mut IndexVec<NormalModuleId, bool>,
  tree_shaking: bool,
  runtime_id: NormalModuleId,
}

/// if no export is used, and the module has no side effects, the module should not be included
fn include_module(ctx: &mut Context, module: &NormalModule) {
  let is_included = ctx.is_module_included_vec[module.id];
  if is_included {
    return;
  }

  // should derived from analyze
  ctx.is_module_included_vec[module.id] = true;
  if ctx.tree_shaking || module.id == ctx.runtime_id {
    module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, stmt_info)| {
      if stmt_info.side_effect {
        ctx.module_side_effects[module.id] = true;
        include_statement(ctx, module, stmt_info_id);
      }
    });
  } else {
    module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, _stmt_info)| {
      if stmt_info_id.index() == 0 {
        return;
      }
      include_statement(ctx, module, stmt_info_id);
    });
  }

  module.import_records.iter().for_each(|import_record| match import_record.resolved_module {
    rolldown_common::ModuleId::Normal(importee_id) => {
      let importee = &ctx.modules[importee_id];
      include_module(ctx, importee);
    }
    rolldown_common::ModuleId::External(_) => {}
  });
}

fn include_symbol(ctx: &mut Context, symbol_ref: SymbolRef) {
  let mut canonical_ref = ctx.symbols.par_canonical_ref_for(symbol_ref);
  let canonical_ref_module = &ctx.modules[canonical_ref.owner];
  let canonical_ref_symbol = ctx.symbols.get(canonical_ref);
  ctx.has_export_used[canonical_ref_module.id] = true;
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
    let mut is_included_vec: IndexVec<NormalModuleId, IndexVec<StmtInfoId, bool>> = self
      .module_table
      .normal_modules
      .iter()
      .map(|m| m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoId, _>>())
      .collect::<IndexVec<NormalModuleId, _>>();

    let mut is_module_included_vec: IndexVec<NormalModuleId, bool> =
      oxc_index::index_vec![false; self.module_table.normal_modules.len()];

    let mut has_export_used: IndexVec<NormalModuleId, bool> =
      oxc_index::index_vec![false; self.module_table.normal_modules.len()];

    let mut module_side_effects: IndexVec<NormalModuleId, bool> =
      oxc_index::index_vec![false; self.module_table.normal_modules.len()];

    let context = &mut Context {
      modules: &self.module_table.normal_modules,
      symbols: &self.symbols,
      is_included_vec: &mut is_included_vec,
      is_module_included_vec: &mut is_module_included_vec,
      tree_shaking: self.input_options.treeshake,
      runtime_id: self.runtime.id(),
      has_export_used: &mut has_export_used,
      module_side_effects: &mut module_side_effects,
    };

    self.entries.iter().for_each(|entry| {
      let module = &self.module_table.normal_modules[entry.id];

      include_module(context, module);
    });

    let enable_tree_shaking = self.input_options.treeshake;
    self.module_table.normal_modules.iter_mut().par_bridge().for_each(|module| {
      module.is_included = !enable_tree_shaking
        || has_export_used[module.id]
        || matches!(module.side_effects, Some(true))
        || (module.side_effects.is_none() && module_side_effects[module.id])
        || module.is_user_defined_entry;
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
}
