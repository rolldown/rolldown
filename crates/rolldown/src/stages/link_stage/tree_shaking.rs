use index_vec::IndexVec;
use rolldown_common::{NormalModule, NormalModuleId, StmtInfoId, SymbolRef};

use crate::types::{module_table::NormalModuleVec, symbols::Symbols};

use super::LinkStage;

struct Context<'a> {
  modules: &'a NormalModuleVec,
  symbols: &'a Symbols,
  is_included_vec: &'a mut IndexVec<NormalModuleId, IndexVec<StmtInfoId, bool>>,
  is_module_included_vec: &'a mut IndexVec<NormalModuleId, bool>,
  tree_shaking: bool,
  runtime_id: NormalModuleId,
}

fn include_module(ctx: &mut Context, module: &NormalModule) {
  let is_included = ctx.is_module_included_vec[module.id];
  if is_included {
    return;
  }

  ctx.is_module_included_vec[module.id] = true;

  if ctx.tree_shaking || module.id == ctx.runtime_id {
    module.stmt_infos.iter_enumerated().for_each(|(stmt_info_id, stmt_info)| {
      if stmt_info.side_effect {
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

  // include the statement itself
  *is_included = true;

  let stmt_info = module.stmt_infos.get(stmt_info_id);

  // include statements that are referenced by this statement
  stmt_info.declared_symbols.iter().chain(stmt_info.referenced_symbols.iter()).for_each(
    |symbol_ref| {
      include_symbol(ctx, *symbol_ref);
    },
  );
}

impl LinkStage<'_> {
  pub fn include_statements(&mut self) {
    use rayon::prelude::*;

    let mut is_included_vec: IndexVec<NormalModuleId, IndexVec<StmtInfoId, bool>> = self
      .module_table
      .normal_modules
      .iter()
      .map(|m| m.stmt_infos.iter().map(|_| false).collect::<IndexVec<StmtInfoId, _>>())
      .collect::<IndexVec<NormalModuleId, _>>();

    let mut is_module_included_vec: IndexVec<NormalModuleId, bool> =
      index_vec::index_vec![false; self.module_table.normal_modules.len()];

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
}
