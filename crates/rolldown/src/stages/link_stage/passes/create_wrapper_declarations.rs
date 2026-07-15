use std::convert::Infallible;

use oxc_index::IndexVec;
use rolldown_common::{
  ModuleIdx, ModuleTable, NormalModule, StmtInfo, StmtInfoIdx, SymbolRef, SymbolRefDb,
  TaggedSymbolRef, WrapKind,
};
use rolldown_utils::pass::{Pass, PassCtx, RawPassOutput, RunToken};

use crate::type_alias::IndexStmtInfos;

use super::{CreateWrapperDeclarationsPass, plan_module_wrapping::WrapperPlan};

// See internal-docs/pass-based-pipeline/implementation.md.

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct CreateWrapperDeclarationsInput<'a> {
  pub module_table: &'a ModuleTable,
  pub commonjs_helper: SymbolRef,
  pub esm_helper: SymbolRef,
}

pub(in crate::stages::link_stage) struct CreateWrapperDeclarationsOwned {
  pub wrapper_plan: WrapperPlan,
  pub symbols: SymbolRefDb,
  pub stmt_infos: IndexStmtInfos,
}

pub(in crate::stages::link_stage) struct CreateWrapperDeclarationsOutput {
  pub wrapper_declarations: WrapperDeclarationsDraft,
  pub symbols: SymbolRefDb,
  pub stmt_infos: IndexStmtInfos,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::stages::link_stage) enum WrapperDeclaration {
  None,
  Cjs { wrapper_ref: SymbolRef, wrapper_stmt_info: StmtInfoIdx },
  Esm { wrapper_ref: SymbolRef, wrapper_stmt_info: StmtInfoIdx },
}

struct WrapperDeclarationSlot {
  declaration: WrapperDeclaration,
  required_by_other_module: bool,
}

pub(in crate::stages::link_stage) struct WrapperDeclarationsDraft {
  slots: IndexVec<ModuleIdx, WrapperDeclarationSlot>,
}

impl WrapperDeclarationsDraft {
  pub(in crate::stages::link_stage) fn declaration(
    &self,
    module_idx: ModuleIdx,
  ) -> WrapperDeclaration {
    self.slots[module_idx].declaration
  }

  pub(in crate::stages::link_stage) fn clear(&mut self, module_idx: ModuleIdx) {
    self.slots[module_idx].declaration = WrapperDeclaration::None;
  }

  pub(super) fn finalize(self) -> ModuleWrappers {
    ModuleWrappers { slots: self.slots }
  }
}

pub(in crate::stages::link_stage) struct ModuleWrappers {
  slots: IndexVec<ModuleIdx, WrapperDeclarationSlot>,
}

impl ModuleWrappers {
  pub(in crate::stages::link_stage) fn wrap_kind(&self, module_idx: ModuleIdx) -> WrapKind {
    match self.slots[module_idx].declaration {
      WrapperDeclaration::None => WrapKind::None,
      WrapperDeclaration::Cjs { .. } => WrapKind::Cjs,
      WrapperDeclaration::Esm { .. } => WrapKind::Esm,
    }
  }

  pub(in crate::stages::link_stage) fn into_modules(
    self,
  ) -> impl Iterator<Item = (ModuleIdx, WrapperDeclaration, bool)> {
    self
      .slots
      .into_iter_enumerated()
      .map(|(module_idx, slot)| (module_idx, slot.declaration, slot.required_by_other_module))
  }
}

#[cfg(test)]
pub(super) mod test_support {
  use oxc::semantic::SymbolId;
  use rolldown_common::{ModuleIdx, StmtInfoIdx, SymbolRef, WrapKind};

  use super::{ModuleWrappers, WrapperDeclaration, WrapperDeclarationSlot};

  pub(in crate::stages::link_stage::passes) fn module_wrappers(
    wrap_kinds: &[WrapKind],
  ) -> ModuleWrappers {
    let slots = wrap_kinds
      .iter()
      .copied()
      .enumerate()
      .map(|(index, wrap_kind)| {
        let module_idx = ModuleIdx::from_usize(index);
        let wrapper_ref = SymbolRef { owner: module_idx, symbol: SymbolId::new(0) };
        let wrapper_stmt_info = StmtInfoIdx::from_usize(0);
        let declaration = match wrap_kind {
          WrapKind::None => WrapperDeclaration::None,
          WrapKind::Cjs => WrapperDeclaration::Cjs { wrapper_ref, wrapper_stmt_info },
          WrapKind::Esm => WrapperDeclaration::Esm { wrapper_ref, wrapper_stmt_info },
        };
        WrapperDeclarationSlot { declaration, required_by_other_module: false }
      })
      .collect();
    ModuleWrappers { slots }
  }
}

impl Pass for CreateWrapperDeclarationsPass {
  type InputRead<'a> = CreateWrapperDeclarationsInput<'a>;
  type InputOwned = CreateWrapperDeclarationsOwned;
  type OutputRead = ();
  type OutputOwned = CreateWrapperDeclarationsOutput;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let CreateWrapperDeclarationsInput { module_table, commonjs_helper, esm_helper } = input;
    let CreateWrapperDeclarationsOwned { wrapper_plan, mut symbols, mut stmt_infos } = owned;
    let plan = wrapper_plan.into_inner();
    let mut slots = IndexVec::with_capacity(plan.len());

    for (module_idx, planned) in plan.into_iter_enumerated() {
      let declaration = match (planned.kind, module_table[module_idx].as_normal()) {
        (Some(WrapKind::Cjs), Some(module)) => allocate_wrapper_declaration(
          module_idx,
          module,
          WrapKind::Cjs,
          commonjs_helper,
          &mut symbols,
          &mut stmt_infos,
        ),
        (Some(WrapKind::Esm), Some(module)) => allocate_wrapper_declaration(
          module_idx,
          module,
          WrapKind::Esm,
          esm_helper,
          &mut symbols,
          &mut stmt_infos,
        ),
        (None | Some(WrapKind::None), _) | (Some(WrapKind::Cjs | WrapKind::Esm), None) => {
          WrapperDeclaration::None
        }
      };
      slots.push(WrapperDeclarationSlot {
        declaration,
        required_by_other_module: planned.required_by_other_module,
      });
    }

    Ok(token.finish(
      (),
      CreateWrapperDeclarationsOutput {
        wrapper_declarations: WrapperDeclarationsDraft { slots },
        symbols,
        stmt_infos,
      },
    ))
  }
}

fn allocate_wrapper_declaration(
  module_idx: ModuleIdx,
  module: &NormalModule,
  kind: WrapKind,
  helper: SymbolRef,
  symbols: &mut SymbolRefDb,
  stmt_infos: &mut IndexStmtInfos,
) -> WrapperDeclaration {
  let prefix = match kind {
    WrapKind::Cjs => "require_",
    WrapKind::Esm => "init_",
    WrapKind::None => return WrapperDeclaration::None,
  };
  let mut name = String::with_capacity(prefix.len() + module.repr_name.len());
  name.push_str(prefix);
  name.push_str(&module.repr_name);
  let wrapper_ref = symbols.create_facade_root_symbol_ref(module_idx, &name);

  let mut stmt_info = StmtInfo::default();
  stmt_info.declared_symbols.push(TaggedSymbolRef::normal(wrapper_ref));
  stmt_info.referenced_symbols.push(helper.into());
  stmt_info.eval_flags = (kind == WrapKind::Esm).into();
  stmt_info.force_tree_shaking = true;
  let wrapper_stmt_info = stmt_infos[module_idx].add_stmt_info(stmt_info);

  match kind {
    WrapKind::Cjs => WrapperDeclaration::Cjs { wrapper_ref, wrapper_stmt_info },
    WrapKind::Esm => WrapperDeclaration::Esm { wrapper_ref, wrapper_stmt_info },
    WrapKind::None => WrapperDeclaration::None,
  }
}

#[cfg(test)]
mod tests {
  use oxc::{
    semantic::{Scoping, SymbolId},
    span::Span,
  };
  use oxc_index::IndexVec;
  use rolldown_common::{
    EntryPointKind, ExportsKind, ImportKind, Module, ModuleTable, OutputFormat, StmtInfos,
    SymbolOrMemberExprRef, SymbolRef, SymbolRefDb, SymbolRefDbForModule,
  };
  use rolldown_utils::pass::{PassPipelineCtx, run_infallible_pass};

  use super::super::{
    CanonicalizeEntriesPass, DetermineModuleFormatsInput, DetermineModuleFormatsPass,
    PlanModuleWrappingInput, PlanModuleWrappingPass,
    test_utils::{entry_point, external_module, module_idx, module_table, normal_module},
  };
  use super::{
    CreateWrapperDeclarationsInput, CreateWrapperDeclarationsOutput,
    CreateWrapperDeclarationsOwned, CreateWrapperDeclarationsPass, WrapperDeclaration,
  };

  struct AllocationResult {
    output: CreateWrapperDeclarationsOutput,
    helper_refs: [SymbolRef; 4],
  }

  fn snapshot_declarations(
    draft: &super::WrapperDeclarationsDraft,
  ) -> Vec<(rolldown_common::ModuleIdx, WrapperDeclaration, bool)> {
    draft
      .slots
      .iter_enumerated()
      .map(|(module_idx, slot)| (module_idx, slot.declaration, slot.required_by_other_module))
      .collect()
  }

  fn set_exports_kind(modules: &mut ModuleTable, index: usize, kind: ExportsKind) {
    modules[module_idx(index)].as_normal_mut().expect("normal module").exports_kind = kind;
  }

  fn representation_modules() -> ModuleTable {
    let mut modules = module_table(vec![
      normal_module(
        0,
        false,
        vec![
          (ImportKind::Require, Some(1), Span::new(1, 2)),
          (ImportKind::DynamicImport, Some(2), Span::new(3, 4)),
        ],
      ),
      normal_module(1, false, vec![(ImportKind::Require, Some(4), Span::new(5, 6))]),
      normal_module(2, false, Vec::new()),
      normal_module(3, false, Vec::new()),
      external_module(4, "external"),
      normal_module(5, false, Vec::new()),
    ]);
    set_exports_kind(&mut modules, 1, ExportsKind::CommonJs);
    set_exports_kind(&mut modules, 2, ExportsKind::Esm);
    set_exports_kind(&mut modules, 3, ExportsKind::None);
    modules
  }

  fn local_symbols(modules: &ModuleTable) -> SymbolRefDb {
    let mut symbols = SymbolRefDb::new();
    for (module_idx, module) in modules.modules.iter_enumerated() {
      if matches!(module, Module::Normal(_)) {
        let scoping = Scoping::default();
        let root_scope_id = scoping.root_scope_id();
        symbols.store_local_db(
          module_idx,
          SymbolRefDbForModule::new(scoping, module_idx, root_scope_id),
        );
      }
    }
    symbols
  }

  fn allocate(modules: &ModuleTable, profiler_names: bool) -> AllocationResult {
    let mut pipeline = PassPipelineCtx::new();
    let (_, entry_plan) = run_infallible_pass(
      CanonicalizeEntriesPass,
      &mut pipeline,
      modules,
      vec![entry_point(0, EntryPointKind::UserDefined)],
    );
    let (_, (formats, seeds)) = run_infallible_pass(
      DetermineModuleFormatsPass,
      &mut pipeline,
      DetermineModuleFormatsInput {
        module_table: modules,
        entry_plan: &entry_plan,
        output_format: OutputFormat::Esm,
        code_splitting_disabled: true,
      },
      (),
    );
    let (_, wrapper_plan) = run_infallible_pass(
      PlanModuleWrappingPass,
      &mut pipeline,
      PlanModuleWrappingInput {
        module_table: modules,
        module_formats: &formats,
        runtime: module_idx(5),
        strict_execution_order: false,
        on_demand_wrapping: false,
      },
      seeds,
    );

    let mut symbols = local_symbols(modules);
    for module_idx in [module_idx(1), module_idx(2)] {
      symbols.create_facade_root_symbol_ref(module_idx, "preexisting");
    }
    let helper_refs = [
      symbols.create_facade_root_symbol_ref(module_idx(5), "__commonJS"),
      symbols.create_facade_root_symbol_ref(module_idx(5), "__commonJSMin"),
      symbols.create_facade_root_symbol_ref(module_idx(5), "__esm"),
      symbols.create_facade_root_symbol_ref(module_idx(5), "__esmMin"),
    ];
    let mut stmt_infos =
      modules.modules.iter().map(|_| StmtInfos::new()).collect::<IndexVec<_, _>>();
    for module_idx in [module_idx(1), module_idx(2)] {
      stmt_infos[module_idx].add_stmt_info(rolldown_common::StmtInfo::default());
    }
    let (commonjs_helper, esm_helper) = if profiler_names {
      (helper_refs[0], helper_refs[2])
    } else {
      (helper_refs[1], helper_refs[3])
    };
    let (_, output) = run_infallible_pass(
      CreateWrapperDeclarationsPass,
      &mut pipeline,
      CreateWrapperDeclarationsInput { module_table: modules, commonjs_helper, esm_helper },
      CreateWrapperDeclarationsOwned { wrapper_plan, symbols, stmt_infos },
    );
    assert!(pipeline.into_diagnostics().is_empty());
    AllocationResult { output, helper_refs }
  }

  fn symbol_reference(stmt_info: &rolldown_common::StmtInfo) -> SymbolRef {
    match &stmt_info.referenced_symbols[0] {
      SymbolOrMemberExprRef::Symbol(symbol_ref) => *symbol_ref,
      SymbolOrMemberExprRef::MemberExpr(_) => panic!("expected a direct symbol reference"),
    }
  }

  #[test]
  fn allocates_representation_safe_declarations_in_module_order() {
    let modules = representation_modules();
    let AllocationResult { mut output, helper_refs } = allocate(&modules, false);
    let declarations = snapshot_declarations(&output.wrapper_declarations);

    assert_eq!(declarations.len(), modules.modules.len());
    assert!(matches!(declarations[0].1, WrapperDeclaration::None));
    let WrapperDeclaration::Cjs { wrapper_ref: cjs_ref, wrapper_stmt_info: cjs_stmt } =
      declarations[1].1
    else {
      panic!("expected CJS declaration");
    };
    let WrapperDeclaration::Esm { wrapper_ref: esm_ref, wrapper_stmt_info: esm_stmt } =
      declarations[2].1
    else {
      panic!("expected ESM declaration");
    };
    assert!(matches!(declarations[3].1, WrapperDeclaration::None));
    assert!(matches!(declarations[4].1, WrapperDeclaration::None));
    assert!(matches!(declarations[5].1, WrapperDeclaration::None));

    assert_eq!(cjs_ref.name(&output.symbols), "require_m1.js");
    assert_eq!(esm_ref.name(&output.symbols), "init_m2.js");
    assert_eq!(
      SymbolRef { owner: module_idx(1), symbol: SymbolId::new(0) }.name(&output.symbols),
      "preexisting"
    );
    assert_eq!(
      SymbolRef { owner: module_idx(2), symbol: SymbolId::new(0) }.name(&output.symbols),
      "preexisting"
    );
    assert_eq!(cjs_ref.symbol, SymbolId::new(1));
    assert_eq!(esm_ref.symbol, SymbolId::new(1));
    assert_eq!(cjs_stmt.index(), 2);
    assert_eq!(esm_stmt.index(), 2);
    assert_eq!(output.stmt_infos[module_idx(1)].len(), 3);
    assert_eq!(output.stmt_infos[module_idx(2)].len(), 3);
    assert!(
      output.stmt_infos[module_idx(1)][rolldown_common::StmtInfoIdx::new(1)]
        .declared_symbols
        .is_empty()
    );
    assert!(
      output.stmt_infos[module_idx(2)][rolldown_common::StmtInfoIdx::new(1)]
        .declared_symbols
        .is_empty()
    );

    let cjs_info = output.stmt_infos[module_idx(1)].get(cjs_stmt);
    assert_eq!(cjs_info.declared_symbols.len(), 1);
    assert_eq!(cjs_info.declared_symbols[0].inner(), cjs_ref);
    assert_eq!(symbol_reference(cjs_info), helper_refs[1]);
    assert!(cjs_info.eval_flags.is_empty());
    assert!(cjs_info.import_records.is_empty());
    assert!(cjs_info.meta.is_empty());
    assert!(cjs_info.force_tree_shaking);

    let esm_info = output.stmt_infos[module_idx(2)].get(esm_stmt);
    assert_eq!(esm_info.declared_symbols.len(), 1);
    assert_eq!(esm_info.declared_symbols[0].inner(), esm_ref);
    assert_eq!(symbol_reference(esm_info), helper_refs[3]);
    assert!(esm_info.eval_flags.has_side_effect_for_tree_shaking());
    assert!(esm_info.import_records.is_empty());
    assert!(esm_info.meta.is_empty());
    assert!(esm_info.force_tree_shaking);

    assert!(declarations[1].2);
    assert!(!declarations[2].2);
    assert!(declarations[4].2);

    output.wrapper_declarations.clear(module_idx(1));
    let cleared = snapshot_declarations(&output.wrapper_declarations)[1];
    assert!(matches!(cleared.1, WrapperDeclaration::None));
    assert!(cleared.2);
  }

  #[test]
  fn preserves_the_selected_profiler_helpers() {
    let modules = representation_modules();
    let AllocationResult { output, helper_refs } = allocate(&modules, true);
    let declarations = snapshot_declarations(&output.wrapper_declarations);
    let WrapperDeclaration::Cjs { wrapper_stmt_info: cjs_stmt, .. } = declarations[1].1 else {
      panic!("expected CJS declaration");
    };
    let WrapperDeclaration::Esm { wrapper_stmt_info: esm_stmt, .. } = declarations[2].1 else {
      panic!("expected ESM declaration");
    };
    assert_eq!(symbol_reference(output.stmt_infos[module_idx(1)].get(cjs_stmt)), helper_refs[0]);
    assert_eq!(symbol_reference(output.stmt_infos[module_idx(2)].get(esm_stmt)), helper_refs[2]);
  }

  #[test]
  fn does_not_mutate_the_module_table() {
    let modules = representation_modules();
    let before = modules
      .modules
      .iter()
      .map(|module| {
        module.as_normal().map(|module| {
          (
            module.repr_name.clone(),
            module.exports_kind,
            module.ast_usage.bits(),
            module.import_records.len(),
          )
        })
      })
      .collect::<Vec<_>>();
    let _ = allocate(&modules, false);
    let after = modules
      .modules
      .iter()
      .map(|module| {
        module.as_normal().map(|module| {
          (
            module.repr_name.clone(),
            module.exports_kind,
            module.ast_usage.bits(),
            module.import_records.len(),
          )
        })
      })
      .collect::<Vec<_>>();
    assert_eq!(after, before);
  }
}
