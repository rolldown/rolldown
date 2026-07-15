use std::convert::Infallible;

use oxc::semantic::NodeId;
use rolldown_common::{
  EcmaViewMeta, FlatOptions, ImportKind, Module, ModuleIdx, ModuleTable, ModuleType,
  SharedNormalizedBundlerOptions, StmtInfoIdx, SymbolRef, SymbolRefDb,
};
use rolldown_utils::{
  pass::{Pass, PassCtx, RawPassOutput, RunToken},
  rayon::{IntoParallelRefIterator, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  stages::link_stage::cross_module_optimization_runner::{
    ModuleOptimizationInput, ModuleOptimizationMutation, analyze_module,
  },
  type_alias::{IndexEcmaAst, IndexStmtInfos},
};

use super::{
  CrossModuleOptimizationPass, EntryPlanDraft, GlobalConstants, GlobalConstantsDraft,
  MemberExprResolutions, SortedModules,
};

// See internal-docs/linking/cross-module-optimization/implementation.md.

type RelatedDynamicImports = FxHashMap<ModuleIdx, FxHashMap<StmtInfoIdx, FxHashSet<NodeId>>>;

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct CrossModuleOptimizationInput<'a> {
  pub module_table: &'a ModuleTable,
  pub ast_table: &'a IndexEcmaAst,
  pub symbols: &'a SymbolRefDb,
  pub sorted_modules: &'a SortedModules,
  pub entry_plan: &'a EntryPlanDraft,
  pub member_expr_resolutions: &'a MemberExprResolutions,
  pub flat_options: FlatOptions,
  pub options: &'a SharedNormalizedBundlerOptions,
}

pub(in crate::stages::link_stage) struct CrossModuleOptimizationOwned {
  pub stmt_infos: IndexStmtInfos,
  pub global_constants: GlobalConstantsDraft,
}

pub(in crate::stages::link_stage) struct UnreachableDynamicImports {
  nodes: FxHashSet<(ModuleIdx, NodeId)>,
}

impl UnreachableDynamicImports {
  pub(in crate::stages::link_stage) fn contains(
    &self,
    module_idx: ModuleIdx,
    node_id: NodeId,
  ) -> bool {
    self.nodes.contains(&(module_idx, node_id))
  }
}

/// One-call ownership envelope. The driver must destructure this immediately; no pass accepts it.
pub(in crate::stages::link_stage) struct CrossModuleOptimizationOutput {
  pub stmt_infos: IndexStmtInfos,
  pub global_constants: GlobalConstants,
}

#[derive(Clone, Copy)]
struct CrossModuleOptimizationConfig {
  pass: u32,
  inline_const_optimization: bool,
}

fn prepare_config(
  module_table: &ModuleTable,
  options: &SharedNormalizedBundlerOptions,
) -> CrossModuleOptimizationConfig {
  let has_side_effect_free_functions = module_table.modules.iter().any(|module| {
    module
      .as_normal()
      .is_some_and(|module| module.meta.contains(EcmaViewMeta::TopExportedSideEffectsFreeFunction))
  });
  let other_optimization_pass = u32::from(has_side_effect_free_functions);
  // Keep the existing arithmetic exactly. `pass = 0` is a known compatibility issue recorded as
  // I-072 and must be fixed separately from this representation-only extraction.
  let cross_module_inline_const_pass = options.optimization.inline_const_pass() - 1;
  CrossModuleOptimizationConfig {
    pass: cross_module_inline_const_pass.max(other_optimization_pass),
    inline_const_optimization: cross_module_inline_const_pass >= 1,
  }
}

fn assert_layout(
  input: CrossModuleOptimizationInput<'_>,
  stmt_infos: &IndexStmtInfos,
  global_constants: &GlobalConstantsDraft,
) {
  let module_count = input.module_table.modules.len();
  for (domain, actual) in [
    ("AST", input.ast_table.len()),
    ("statement", stmt_infos.len()),
    ("symbol", input.symbols.inner().len()),
    ("member-resolution", input.member_expr_resolutions.module_count()),
  ] {
    std::assert_eq!(
      actual,
      module_count,
      "{domain} layout must match modules before cross-module optimization"
    );
  }

  for (module_idx, module) in input.module_table.modules.iter_enumerated() {
    let valid = match module {
      Module::Normal(module) => {
        let ast = input.ast_table[module_idx].as_ref();
        module.idx == module_idx
          && ast.is_some()
          && input.symbols.inner()[module_idx].is_some()
          && input.member_expr_resolutions.has_normal_slot(module_idx)
          && ast.is_some_and(|ast| {
            let body = &ast.program().body;
            let statement_count = stmt_infos[module_idx].len();
            // Rebuilt object-shaped JSON keeps one statement slot for every executable AST
            // statement, plus the namespace slot. Its final `export { ... }` list has no runtime
            // statement and cannot contain a call or dynamic import for P to mutate.
            statement_count > body.len()
              || (module.module_type == ModuleType::Json
                && statement_count == body.len()
                && body.last().is_some_and(|statement| {
                  let oxc::ast::ast::Statement::ExportNamedDeclaration(export) = statement else {
                    return false;
                  };
                  export.declaration.is_none() && export.source.is_none()
                }))
          })
      }
      Module::External(module) => {
        module.idx == module_idx
          && input.ast_table[module_idx].is_none()
          && input.symbols.inner()[module_idx].is_some()
          && !input.member_expr_resolutions.has_normal_slot(module_idx)
          && stmt_infos[module_idx].len() == 1
      }
    };
    std::assert!(valid, "cross-module optimization slot shape must match {module_idx:?}");
  }

  let mut sorted_modules = FxHashSet::default();
  for module_idx in input.sorted_modules.as_slice() {
    let valid = input
      .module_table
      .modules
      .get(*module_idx)
      .is_some_and(|module| module.as_normal().is_some())
      && input.ast_table.get(*module_idx).is_some_and(Option::is_some)
      && input.member_expr_resolutions.has_normal_slot(*module_idx);
    std::assert!(valid, "sorted module {module_idx:?} must be an in-range normal module");
    std::assert!(
      sorted_modules.insert(*module_idx),
      "sorted module {module_idx:?} must occur exactly once"
    );
  }

  for owner in global_constants.identity_owners() {
    let valid =
      input.module_table.modules.get(owner).is_some_and(|module| module.as_normal().is_some())
        && input.symbols.inner().get(owner).is_some_and(Option::is_some);
    std::assert!(valid, "global constant owner {owner:?} must be an in-range normal module");
  }

  for (root, entry_idx, importer_idx, stmt_idx, node_id, record_idx) in
    input.entry_plan.related_dynamic_imports()
  {
    let root_is_exact = root == entry_idx
      && input.module_table.modules.get(root).is_some_and(|module| module.as_normal().is_some());
    std::assert!(root_is_exact, "related dynamic import must target its grouped normal entry");

    let importer = input.module_table.modules.get(importer_idx).and_then(Module::as_normal);
    std::assert!(
      importer.is_some(),
      "related dynamic import must have an in-range normal importer"
    );
    let Some(importer) = importer else {
      continue;
    };
    let statement_is_exact = stmt_idx.index() > 0
      && input
        .ast_table
        .get(importer_idx)
        .and_then(Option::as_ref)
        .is_some_and(|ast| stmt_idx.index() <= ast.program().body.len())
      && stmt_infos
        .get(importer_idx)
        .and_then(|statements| statements.infos.get(stmt_idx))
        .is_some_and(|statement| statement.import_records.contains(&record_idx));
    std::assert!(
      statement_is_exact,
      "related dynamic import statement identity must match its importer"
    );
    let record_matches = |candidate_idx: rolldown_common::ImportRecordIdx| {
      importer.import_records.get(candidate_idx).is_some_and(|record| {
        std::matches!(record.kind, ImportKind::DynamicImport)
          && record.resolved_module == Some(root)
          && record
            .dynamic_import_expr_info
            .as_deref()
            .is_some_and(|info| info.stmt_info_idx == stmt_idx && info.node_id == node_id)
      })
    };
    let mapped_record_idx = importer.imports.get(&node_id).copied();
    let record_is_exact = record_matches(record_idx)
      && mapped_record_idx.is_some_and(|mapped_record_idx| {
        record_matches(mapped_record_idx)
          && stmt_infos
            .get(importer_idx)
            .and_then(|statements| statements.infos.get(stmt_idx))
            .is_some_and(|statement| statement.import_records.contains(&mapped_record_idx))
      });
    std::assert!(
      record_is_exact,
      "related dynamic import record and NodeId-mapped record must both match the statement, node, kind, and target: related={record_idx:?}, mapped={mapped_record_idx:?}"
    );
  }
}

fn collect_related_dynamic_imports(entry_plan: &EntryPlanDraft) -> RelatedDynamicImports {
  let mut related = FxHashMap::default();
  for (_, _, importer_idx, stmt_idx, node_id, _) in entry_plan.related_dynamic_imports() {
    related
      .entry(importer_idx)
      .or_insert_with(FxHashMap::default)
      .entry(stmt_idx)
      .or_insert_with(FxHashSet::default)
      .insert(node_id);
  }
  related
}

fn find_modules_referencing_constants(
  module_table: &ModuleTable,
  symbols: &SymbolRefDb,
  new_constant_refs: &FxHashSet<SymbolRef>,
) -> FxHashSet<ModuleIdx> {
  if new_constant_refs.is_empty() {
    return FxHashSet::default();
  }
  module_table
    .modules
    .iter()
    .filter_map(|module| {
      let normal_module = module.as_normal()?;
      normal_module
        .named_imports
        .keys()
        .any(|local_symbol_ref| {
          new_constant_refs.contains(&symbols.canonical_ref_for(*local_symbol_ref))
        })
        .then_some(normal_module.idx)
    })
    .collect()
}

fn analyze_round(
  input: CrossModuleOptimizationInput<'_>,
  config: CrossModuleOptimizationConfig,
  global_constants: &GlobalConstantsDraft,
  related_dynamic_imports: &RelatedDynamicImports,
  modules_to_process: Option<&FxHashSet<ModuleIdx>>,
) -> Vec<Option<ModuleOptimizationMutation>> {
  input
    .sorted_modules
    .as_slice()
    .par_iter()
    .map(|module_idx| {
      if modules_to_process.is_some_and(|modules| !modules.contains(module_idx)) {
        return None;
      }
      let module = input.module_table.modules[*module_idx].as_normal();
      std::assert!(module.is_some(), "validated sorted modules must remain normal");
      let module = module?;
      let ast = input.ast_table[*module_idx].as_ref();
      std::assert!(ast.is_some(), "validated sorted modules must retain their AST");
      let ast = ast?;
      let resolved_member_expr_refs = input.member_expr_resolutions.get(*module_idx);
      std::assert!(
        resolved_member_expr_refs.is_some(),
        "validated sorted modules must retain member resolutions"
      );
      let resolved_member_expr_refs = resolved_member_expr_refs?;
      analyze_module(ModuleOptimizationInput {
        module,
        ast,
        modules: &input.module_table.modules,
        symbols: input.symbols,
        global_constants,
        flat_options: input.flat_options,
        options: input.options,
        inline_const_optimization: config.inline_const_optimization,
        stmt_idx_to_dynamic_import_expr_node_ids: related_dynamic_imports.get(module_idx),
        resolved_member_expr_refs,
      })
    })
    .collect()
}

fn assert_mutation_layout(
  sorted_modules: &SortedModules,
  stmt_infos: &IndexStmtInfos,
  mutations: &[Option<ModuleOptimizationMutation>],
) {
  std::assert_eq!(
    mutations.len(),
    sorted_modules.as_slice().len(),
    "cross-module mutation batches must preserve sorted-module layout"
  );
  for (module_idx, mutation) in sorted_modules.as_slice().iter().copied().zip(mutations) {
    let Some(mutation) = mutation else { continue };
    for stmt_info_idx in mutation.eval_flags.keys() {
      let valid = stmt_infos
        .get(module_idx)
        .and_then(|statements| statements.infos.get(*stmt_info_idx))
        .is_some();
      std::assert!(
        valid,
        "cross-module mutation statement {stmt_info_idx:?} must exist in {module_idx:?}"
      );
    }
  }
}

impl Pass for CrossModuleOptimizationPass {
  type InputRead<'a> = CrossModuleOptimizationInput<'a>;
  type InputOwned = CrossModuleOptimizationOwned;
  type OutputRead = UnreachableDynamicImports;
  type OutputOwned = CrossModuleOptimizationOutput;
  type Error = Infallible;

  fn run(
    self,
    token: RunToken<'_, Self>,
    _cx: &mut PassCtx<'_>,
    input: Self::InputRead<'_>,
    owned: Self::InputOwned,
  ) -> Result<RawPassOutput<Self::OutputRead, Self::OutputOwned>, Self::Error> {
    let CrossModuleOptimizationOwned { mut stmt_infos, mut global_constants } = owned;
    let config = prepare_config(input.module_table, input.options);
    if config.pass < 1 {
      return Ok(token.finish(
        UnreachableDynamicImports { nodes: FxHashSet::default() },
        CrossModuleOptimizationOutput { stmt_infos, global_constants: global_constants.finalize() },
      ));
    }

    assert_layout(input, &stmt_infos, &global_constants);
    let related_dynamic_imports = collect_related_dynamic_imports(input.entry_plan);
    let mut unreachable_nodes = FxHashSet::default();
    let mut remaining_passes = config.pass;
    let mut changed = true;
    let mut modules_to_process = None;
    while remaining_passes > 0 && changed {
      remaining_passes -= 1;
      changed = false;
      let mutations = analyze_round(
        input,
        config,
        &global_constants,
        &related_dynamic_imports,
        modules_to_process.as_ref(),
      );
      // Validate the complete parallel batch before the first serial write so a malformed result
      // cannot leave statements or constants partially committed.
      assert_mutation_layout(input.sorted_modules, &stmt_infos, &mutations);
      let mut new_constant_refs = FxHashSet::default();
      for (module_idx, mutation) in input.sorted_modules.as_slice().iter().copied().zip(mutations) {
        let Some(mutation) = mutation else { continue };
        for (stmt_info_idx, eval_flags) in mutation.eval_flags {
          stmt_infos[module_idx][stmt_info_idx].eval_flags = eval_flags;
        }
        if !mutation.constants.is_empty() {
          changed = true;
          new_constant_refs.extend(mutation.constants.keys().copied());
          global_constants.extend(mutation.constants);
        }
        unreachable_nodes
          .extend(mutation.unreachable_nodes.into_iter().map(|node_id| (module_idx, node_id)));
      }
      if !changed {
        break;
      }
      modules_to_process = Some(find_modules_referencing_constants(
        input.module_table,
        input.symbols,
        &new_constant_refs,
      ));
    }

    Ok(token.finish(
      UnreachableDynamicImports { nodes: unreachable_nodes },
      CrossModuleOptimizationOutput { stmt_infos, global_constants: global_constants.finalize() },
    ))
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use oxc::{
    ast_visit::{Visit, walk},
    semantic::{NodeId, Scoping, SymbolId},
    span::SourceType,
  };
  use oxc_index::IndexVec;
  use rolldown_common::{
    ConstExportMeta, ConstantValue, DynamicImportExprInfo, EcmaViewMeta, EntryPointKind,
    FlatOptions, GetLocalDb, GetLocalDbMut, ImportKind, ImportRecordIdx, InlineConstConfig,
    InlineConstMode, InlineConstOption, MemberExprRefResolution, Module, ModuleIdx, ModuleTable,
    ModuleType, NamedImport, NormalizedBundlerOptions, OptimizationOption, Specifier,
    StmtEvalFlags, StmtInfo, StmtInfoIdx, StmtInfos, SymbolRef, SymbolRefDb, SymbolRefDbForModule,
    SymbolRefFlags, normalize_optimization_option,
  };
  use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
  use rolldown_utils::pass::{PassPipelineCtx, Sealed, run_infallible_pass};
  use rustc_hash::FxHashMap;

  use crate::type_alias::{IndexEcmaAst, IndexStmtInfos};

  use super::super::{
    CanonicalizeEntriesPass, CrossModuleOptimizationInput, CrossModuleOptimizationOutput,
    CrossModuleOptimizationOwned, CrossModuleOptimizationPass, EntryPlanDraft, GlobalConstants,
    MemberExprResolutions, SortedModules, UnreachableDynamicImports,
    compute_module_execution_order::test_support::sorted_modules,
    extract_global_constants::test_support::global_constants,
    resolve_member_expressions::test_support::member_expr_resolutions,
    test_utils::{entry_point, external_module, module_idx, module_table, normal_module},
  };
  use super::prepare_config;

  struct Fixture {
    modules: ModuleTable,
    asts: IndexEcmaAst,
    symbols: SymbolRefDb,
    statements: IndexStmtInfos,
    resolutions: MemberExprResolutions,
    sorted: SortedModules,
    entries: EntryPlanDraft,
  }

  fn options_with_pass(pass: Option<u32>) -> Arc<NormalizedBundlerOptions> {
    let mut options = NormalizedBundlerOptions::default();
    options.optimization = normalize_optimization_option(
      Some(OptimizationOption {
        inline_const: Some(match pass {
          Some(pass) => {
            InlineConstOption::Config(InlineConstConfig { mode: Some(InlineConstMode::All), pass })
          }
          None => InlineConstOption::Bool(false),
        }),
        ..OptimizationOption::default()
      }),
      options.platform,
    );
    Arc::new(options)
  }

  fn fixture(sources: &[&str], imports: &[Vec<(ImportKind, Option<usize>)>]) -> Fixture {
    let mut modules = Vec::new();
    let mut asts = IndexVec::new();
    let mut symbols = SymbolRefDb::new();
    let mut statements = IndexVec::new();
    for (index, source) in sources.iter().enumerate() {
      let filename = format!("m{index}.js");
      let ast = EcmaCompiler::parse(&filename, *source, SourceType::default().with_module(true))
        .expect("test source parses");
      let scoping = EcmaAst::make_semantic(ast.program()).into_scoping();
      let symbol_count = scoping.symbols_len();
      let root_scope_id = scoping.root_scope_id();
      let idx = module_idx(index);
      symbols.store_local_db(idx, SymbolRefDbForModule::new(scoping, idx, root_scope_id));
      for symbol_index in 0..symbol_count {
        symbols
          .local_db_mut(idx)
          .flags
          .entry(SymbolId::new(symbol_index))
          .or_default()
          .insert(SymbolRefFlags::IsNotReassigned);
      }
      let namespace_ref = symbols.create_facade_root_symbol_ref(idx, "namespace");
      let default_export_ref = symbols.create_facade_root_symbol_ref(idx, "default");
      let test_imports = imports[index]
        .iter()
        .enumerate()
        .map(|(record, (kind, target))| {
          (
            *kind,
            *target,
            oxc::span::Span::new(
              u32::try_from(record).expect("test record fits u32"),
              u32::try_from(record + 1).expect("test record fits u32"),
            ),
          )
        })
        .collect();
      let mut module = normal_module(index, false, test_imports);
      let normal = module.as_normal_mut().expect("normal test module");
      normal.source = (*source).into();
      normal.namespace_object_ref = namespace_ref;
      normal.default_export_ref = default_export_ref;
      let mut stmt_infos = StmtInfos::new();
      for _ in &ast.program().body {
        stmt_infos.add_stmt_info(StmtInfo::default());
      }
      modules.push(module);
      asts.push(Some(ast));
      statements.push(stmt_infos);
    }
    let modules = module_table(modules);
    let resolutions = member_expr_resolutions(sources.iter().map(|_| Some(FxHashMap::default())));
    let sorted = sorted_modules((0..sources.len()).map(module_idx));
    let mut pipeline = PassPipelineCtx::new();
    let (_, entries) =
      run_infallible_pass(CanonicalizeEntriesPass, &mut pipeline, &modules, Vec::new());
    assert!(pipeline.into_diagnostics().is_empty());
    Fixture { modules, asts, symbols, statements, resolutions, sorted, entries }
  }

  fn symbol(fixture: &Fixture, module: usize, name: &str) -> SymbolRef {
    let module_idx = module_idx(module);
    let local = fixture.symbols.local_db(module_idx);
    let symbol = local
      .ast_scopes
      .scoping()
      .get_binding(local.ast_scopes.scoping().root_scope_id(), name.into())
      .expect("test binding");
    (module_idx, symbol).into()
  }

  #[derive(Default)]
  struct AstNodes {
    imports: Vec<NodeId>,
    member_callees: Vec<NodeId>,
  }

  impl<'ast> Visit<'ast> for AstNodes {
    fn visit_import_expression(&mut self, expression: &oxc::ast::ast::ImportExpression<'ast>) {
      self.imports.push(expression.node_id());
      walk::walk_import_expression(self, expression);
    }

    fn visit_call_expression(&mut self, expression: &oxc::ast::ast::CallExpression<'ast>) {
      if let Some(member) = expression.callee.as_member_expression() {
        self.member_callees.push(member.node_id());
      }
      walk::walk_call_expression(self, expression);
    }
  }

  fn ast_nodes(ast: &EcmaAst) -> AstNodes {
    ast.program.with_dependent(|_, dependent| {
      let mut nodes = AstNodes::default();
      nodes.visit_program(&dependent.program);
      nodes
    })
  }

  fn install_related_dynamic_import(
    fixture: &mut Fixture,
    importer: usize,
    target: usize,
    stmt: usize,
    node_id: NodeId,
    record_idx: usize,
  ) {
    let importer_idx = module_idx(importer);
    let target_idx = module_idx(target);
    let stmt_idx = StmtInfoIdx::new(stmt);
    let record_idx = ImportRecordIdx::from_usize(record_idx);
    let importer = fixture.modules[importer_idx].as_normal_mut().expect("normal importer");
    let record = &mut importer.import_records[record_idx];
    record.dynamic_import_expr_info =
      Some(Box::new(DynamicImportExprInfo { stmt_info_idx: stmt_idx, node_id }));
    let resolved_module = record.resolved_module;
    importer.imports.insert(node_id, record_idx);
    fixture.statements[importer_idx][stmt_idx].import_records.push(record_idx);
    let mut entry = entry_point(target, EntryPointKind::DynamicImport);
    entry.related_stmt_infos.push((importer_idx, stmt_idx, node_id, record_idx));
    let mut pipeline = PassPipelineCtx::new();
    let (_, entries) =
      run_infallible_pass(CanonicalizeEntriesPass, &mut pipeline, &fixture.modules, vec![entry]);
    assert!(pipeline.into_diagnostics().is_empty());
    fixture.entries = entries;
    assert_eq!(resolved_module, Some(target_idx));
  }

  fn append_external(fixture: &mut Fixture, index: usize, id: &str) -> SymbolRef {
    let external = external_module(index, id);
    let external_namespace = external.as_external().expect("external module").namespace_ref;
    fixture.modules.modules.push(external);
    fixture.asts.push(None);
    fixture.statements.push(StmtInfos::new());
    let scoping = Scoping::default();
    let root_scope_id = scoping.root_scope_id();
    fixture.symbols.store_local_db(
      module_idx(index),
      SymbolRefDbForModule::new(scoping, module_idx(index), root_scope_id),
    );
    assert_eq!(
      fixture.symbols.create_facade_root_symbol_ref(module_idx(index), id),
      external_namespace
    );
    let mut resolutions = (0..index).map(|_| Some(FxHashMap::default())).collect::<Vec<_>>();
    resolutions.push(None);
    fixture.resolutions = member_expr_resolutions(resolutions);
    external_namespace
  }

  fn dynamic_call_fixture(annotation_only: bool) -> (Fixture, NodeId) {
    let mut fixture = fixture(
      &["function empty(callback) {} empty(() => import('./target'));", ""],
      &[vec![(ImportKind::DynamicImport, Some(1))], Vec::new()],
    );
    let empty = symbol(&fixture, 0, "empty");
    let mut flags = SymbolRefFlags::SideEffectsFreeFunction | SymbolRefFlags::IsNotReassigned;
    flags.set(SymbolRefFlags::PureAnnotationOnly, annotation_only);
    *empty.flags_mut(&mut fixture.symbols) = flags;
    fixture.modules[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .meta
      .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    let nodes = ast_nodes(fixture.asts[module_idx(0)].as_ref().expect("module AST"));
    let node_id = nodes.imports[0];
    install_related_dynamic_import(&mut fixture, 0, 1, 2, node_id, 0);
    fixture.statements[module_idx(0)][StmtInfoIdx::new(2)].eval_flags =
      StmtEvalFlags::PureCjs | StmtEvalFlags::UnknownSideEffect;
    (fixture, node_id)
  }

  fn run(
    fixture: Fixture,
    options: &Arc<NormalizedBundlerOptions>,
    constants: impl IntoIterator<Item = (SymbolRef, ConstExportMeta)>,
  ) -> (Sealed<UnreachableDynamicImports>, IndexStmtInfos, GlobalConstants) {
    let mut pipeline = PassPipelineCtx::new();
    let (unreachable, output) = run_infallible_pass(
      CrossModuleOptimizationPass,
      &mut pipeline,
      CrossModuleOptimizationInput {
        module_table: &fixture.modules,
        ast_table: &fixture.asts,
        symbols: &fixture.symbols,
        sorted_modules: &fixture.sorted,
        entry_plan: &fixture.entries,
        member_expr_resolutions: &fixture.resolutions,
        flat_options: FlatOptions::from_shared_options(options),
        options,
      },
      CrossModuleOptimizationOwned {
        stmt_infos: fixture.statements,
        global_constants: global_constants(constants),
      },
    );
    assert!(pipeline.into_diagnostics().is_empty());
    let CrossModuleOptimizationOutput { stmt_infos, global_constants } = output;
    (unreachable, stmt_infos, global_constants)
  }

  #[test]
  fn computes_bounded_pass_count_and_forces_one_round_for_side_effect_free_functions() {
    let mut fixture = fixture(&[""], &[Vec::new()]);
    let options = options_with_pass(Some(3));
    let config = prepare_config(&fixture.modules, &options);
    assert_eq!(config.pass, 2);
    assert!(config.inline_const_optimization);

    let options = options_with_pass(None);
    let config = prepare_config(&fixture.modules, &options);
    assert_eq!(config.pass, 0);
    assert!(!config.inline_const_optimization);
    fixture.modules[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .meta
      .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    let config = prepare_config(&fixture.modules, &options);
    assert_eq!(config.pass, 1);
    assert!(!config.inline_const_optimization);
  }

  #[test]
  #[should_panic(expected = "attempt to subtract with overflow")]
  fn preserves_inline_const_pass_zero_debug_underflow() {
    let fixture = fixture(&[""], &[Vec::new()]);
    let options = options_with_pass(Some(0));
    let _ = prepare_config(&fixture.modules, &options);
  }

  fn constant_chain(pass: u32) -> (FxHashMap<SymbolRef, ConstExportMeta>, SymbolRef, SymbolRef) {
    let mut fixture = fixture(
      &["export const A = 1;", "import { A } from './a'; export const B = A;"],
      &[Vec::new(), vec![(ImportKind::Import, Some(0))]],
    );
    let export_a = symbol(&fixture, 0, "A");
    let import_a = symbol(&fixture, 1, "A");
    let export_b = symbol(&fixture, 1, "B");
    fixture.symbols.link(import_a, export_a);
    fixture.modules[module_idx(1)].as_normal_mut().expect("normal importer").named_imports.insert(
      import_a,
      NamedImport {
        imported: Specifier::from("A"),
        span_imported: oxc::span::Span::new(0, 1),
        imported_as: import_a,
        record_idx: ImportRecordIdx::from_usize(0),
      },
    );
    let options = options_with_pass(Some(pass));
    let (_, _, constants) = run(fixture, &options, []);
    (constants.into_legacy(), export_a, export_b)
  }

  #[test]
  fn isolates_same_round_constants_and_filters_the_next_round_by_named_imports() {
    let (constants, export_a, export_b) = constant_chain(2);
    assert!(std::matches!(
      constants.get(&export_a).map(|meta| &meta.value),
      Some(ConstantValue::Number(value)) if value.to_bits() == 1.0_f64.to_bits()
    ));
    assert!(!constants.contains_key(&export_b), "same-round constants must stay invisible");

    let (constants, export_a, export_b) = constant_chain(3);
    assert!(constants.contains_key(&export_a));
    assert!(std::matches!(
      constants.get(&export_b).map(|meta| &meta.value),
      Some(ConstantValue::Number(value)) if value.to_bits() == 1.0_f64.to_bits()
    ));
  }

  #[test]
  fn retains_first_round_unreachable_nodes_across_a_later_filtered_round() {
    let mut fixture = fixture(
      &[
        "export const A = 1; function empty(callback) {} empty(() => import('./target'));",
        "import { A } from './source'; export const B = A;",
        "",
      ],
      &[
        vec![(ImportKind::DynamicImport, Some(2))],
        vec![(ImportKind::Import, Some(0))],
        Vec::new(),
      ],
    );
    let empty = symbol(&fixture, 0, "empty");
    *empty.flags_mut(&mut fixture.symbols) =
      SymbolRefFlags::SideEffectsFreeFunction | SymbolRefFlags::IsNotReassigned;
    fixture.modules[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .meta
      .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    let export_a = symbol(&fixture, 0, "A");
    let import_a = symbol(&fixture, 1, "A");
    let export_b = symbol(&fixture, 1, "B");
    fixture.symbols.link(import_a, export_a);
    fixture.modules[module_idx(1)].as_normal_mut().expect("normal importer").named_imports.insert(
      import_a,
      NamedImport {
        imported: Specifier::from("A"),
        span_imported: oxc::span::Span::new(0, 1),
        imported_as: import_a,
        record_idx: ImportRecordIdx::from_usize(0),
      },
    );
    let nodes = ast_nodes(fixture.asts[module_idx(0)].as_ref().expect("module AST"));
    let node_id = nodes.imports[0];
    install_related_dynamic_import(&mut fixture, 0, 2, 3, node_id, 0);

    let options = options_with_pass(Some(3));
    let (unreachable, _, constants) = run(fixture, &options, []);

    assert!(
      constants.into_legacy().contains_key(&export_b),
      "the named importer must run in the second filtered round"
    );
    assert!(
      unreachable.contains(module_idx(0), node_id),
      "the first-round node must survive when its module is filtered from round two"
    );
  }

  #[test]
  fn preserves_existing_commonjs_constants_when_finalizing_the_read_only_output() {
    let fixture = fixture(&[""], &[Vec::new()]);
    let existing = fixture.modules[module_idx(0)].namespace_object_ref();
    let existing = existing.expect("normal namespace");
    let options = options_with_pass(None);
    let (_, _, constants) =
      run(fixture, &options, [(existing, ConstExportMeta::new(ConstantValue::Number(7.0), true))]);
    let constants = constants.into_legacy();
    let constant = constants.get(&existing).expect("preserved constant");
    assert!(constant.commonjs_export);
    assert!(std::matches!(constant.value, ConstantValue::Number(7.0)));
  }

  #[test]
  fn disabled_fast_path_does_not_observe_malformed_dense_inputs() {
    let mut fixture = fixture(&[""], &[Vec::new()]);
    fixture.asts.clear();
    fixture.statements.clear();
    fixture.resolutions = member_expr_resolutions(std::iter::empty::<
      Option<rolldown_common::MemberExprRefResolutionMap>,
    >());
    let options = options_with_pass(None);
    let (_, statements, constants) = run(fixture, &options, []);
    assert!(statements.is_empty());
    assert!(constants.into_legacy().is_empty());
  }

  #[test]
  fn rejects_dense_and_sorted_layout_mismatches() {
    let mut fixture = fixture(&["export const A = 1;"], &[Vec::new()]);
    fixture.modules[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .meta
      .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    fixture.sorted = sorted_modules([module_idx(0), module_idx(0)]);
    let options = options_with_pass(None);
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(fixture, &options, [])));
    assert!(result.is_err());
  }

  #[test]
  fn accepts_rebuilt_json_export_list_without_a_runtime_statement_slot() {
    let mut fixture =
      fixture(&["const value = 1; export default { value }; export { value };"], &[Vec::new()]);
    let module = fixture.modules[module_idx(0)].as_normal_mut().expect("normal JSON module");
    module.module_type = ModuleType::Json;
    module.meta.insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    let body_len = fixture.asts[module_idx(0)].as_ref().expect("JSON AST").program().body.len();
    assert_eq!(fixture.statements[module_idx(0)].len(), body_len + 1);
    fixture.statements[module_idx(0)].infos.pop();
    assert_eq!(fixture.statements[module_idx(0)].len(), body_len);

    let options = options_with_pass(None);
    let (_, statements, _) = run(fixture, &options, []);

    assert_eq!(statements[module_idx(0)].len(), body_len);
  }

  #[test]
  fn rejects_global_constants_owned_by_external_or_missing_modules() {
    let options = options_with_pass(None);
    for (mut fixture, invalid) in {
      let missing = fixture(&[""], &[Vec::new()]);
      let missing_ref = SymbolRef { owner: ModuleIdx::from_usize(4), symbol: SymbolId::new(0) };
      let mut external = fixture(&[""], &[Vec::new()]);
      let external_ref = append_external(&mut external, 1, "external");
      [(missing, missing_ref), (external, external_ref)]
    } {
      fixture.modules[module_idx(0)]
        .as_normal_mut()
        .expect("normal module")
        .meta
        .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
      let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(
          fixture,
          &options,
          [(invalid, ConstExportMeta::new(ConstantValue::Boolean(true), false))],
        )
      }));
      assert!(result.is_err());
    }
  }

  #[test]
  fn replaces_eval_flags_and_distinguishes_empty_from_annotation_only_callbacks() {
    let (fixture, node_id) = dynamic_call_fixture(false);
    let options = options_with_pass(None);
    let (unreachable, statements, _) = run(fixture, &options, []);
    assert!(unreachable.contains(module_idx(0), node_id));
    assert_eq!(statements[module_idx(0)][StmtInfoIdx::new(2)].eval_flags, StmtEvalFlags::empty());

    let (fixture, node_id) = dynamic_call_fixture(true);
    let (unreachable, statements, _) = run(fixture, &options, []);
    assert!(!unreachable.contains(module_idx(0), node_id));
    assert_eq!(statements[module_idx(0)][StmtInfoIdx::new(2)].eval_flags, StmtEvalFlags::empty());
  }

  #[test]
  fn rejects_external_related_entries_outside_the_producer_invariant() {
    let mut fixture = fixture(
      &["function empty(callback) {} empty(() => import('external'));"],
      &[vec![(ImportKind::DynamicImport, Some(1))]],
    );
    append_external(&mut fixture, 1, "external");
    let empty = symbol(&fixture, 0, "empty");
    *empty.flags_mut(&mut fixture.symbols) =
      SymbolRefFlags::SideEffectsFreeFunction | SymbolRefFlags::IsNotReassigned;
    fixture.modules[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .meta
      .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    let nodes = ast_nodes(fixture.asts[module_idx(0)].as_ref().expect("module AST"));
    let node_id = nodes.imports[0];
    install_related_dynamic_import(&mut fixture, 0, 1, 2, node_id, 0);

    let options = options_with_pass(None);
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(fixture, &options, [])));
    assert!(result.is_err());
  }

  #[test]
  fn accepts_equivalent_related_and_node_mapped_dynamic_import_records() {
    let (mut fixture, node_id) = dynamic_call_fixture(false);
    let importer_idx = module_idx(0);
    let importer = fixture.modules[importer_idx].as_normal_mut().expect("normal importer");
    let duplicate = importer.import_records[ImportRecordIdx::from_usize(0)].clone();
    let duplicate_idx = importer.import_records.push(duplicate);
    importer.imports.insert(node_id, duplicate_idx);
    fixture.statements[importer_idx][StmtInfoIdx::new(2)].import_records.push(duplicate_idx);

    let options = options_with_pass(None);
    let (unreachable, _, _) = run(fixture, &options, []);

    assert!(unreachable.contains(importer_idx, node_id));
  }

  #[test]
  fn resolves_namespace_member_calls_to_side_effect_free_exports() {
    let mut fixture = fixture(
      &["import * as ns from './lib'; ns.empty();", "export function empty() {}"],
      &[vec![(ImportKind::Import, Some(1))], Vec::new()],
    );
    let namespace = symbol(&fixture, 0, "ns");
    let empty = symbol(&fixture, 1, "empty");
    *empty.flags_mut(&mut fixture.symbols) =
      SymbolRefFlags::SideEffectsFreeFunction | SymbolRefFlags::IsNotReassigned;
    fixture.modules[module_idx(1)]
      .as_normal_mut()
      .expect("normal export owner")
      .meta
      .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    fixture.modules[module_idx(0)].as_normal_mut().expect("normal importer").named_imports.insert(
      namespace,
      NamedImport {
        imported: Specifier::Star,
        span_imported: oxc::span::Span::new(0, 1),
        imported_as: namespace,
        record_idx: ImportRecordIdx::from_usize(0),
      },
    );
    let nodes = ast_nodes(fixture.asts[module_idx(0)].as_ref().expect("module AST"));
    let member_node = nodes.member_callees[0];
    let mut importer_resolutions = FxHashMap::default();
    importer_resolutions.insert(
      member_node,
      MemberExprRefResolution {
        resolved: Some(empty),
        prop_and_related_span_list: Vec::new(),
        depended_refs: Vec::new(),
        target_commonjs_exported_symbol: None,
        reference_id: None,
      },
    );
    fixture.resolutions =
      member_expr_resolutions([Some(importer_resolutions), Some(FxHashMap::default())]);
    fixture.statements[module_idx(0)][StmtInfoIdx::new(2)].eval_flags =
      StmtEvalFlags::UnknownSideEffect;
    let options = options_with_pass(None);
    let (_, statements, _) = run(fixture, &options, []);
    assert_eq!(statements[module_idx(0)][StmtInfoIdx::new(2)].eval_flags, StmtEvalFlags::empty());
  }

  #[test]
  fn preserves_default_literal_extraction_while_inline_is_disabled_but_imports_are_tracked() {
    let mut fixture = fixture(
      &["export default 1; import('./target');", ""],
      &[vec![(ImportKind::DynamicImport, Some(1))], Vec::new()],
    );
    fixture.modules[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .meta
      .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    let default_export =
      fixture.modules[module_idx(0)].as_normal().expect("normal module").default_export_ref;
    let nodes = ast_nodes(fixture.asts[module_idx(0)].as_ref().expect("module AST"));
    install_related_dynamic_import(&mut fixture, 0, 1, 2, nodes.imports[0], 0);
    let options = options_with_pass(None);
    let (_, _, constants) = run(fixture, &options, []);
    let constants = constants.into_legacy();
    assert!(std::matches!(
      constants.get(&default_export).map(|meta| &meta.value),
      Some(ConstantValue::Number(value)) if value.to_bits() == 1.0_f64.to_bits()
    ));
  }

  fn assert_layout_rejected(mutate: impl FnOnce(&mut Fixture)) {
    let mut fixture = fixture(&["export const A = 1;"], &[Vec::new()]);
    fixture.modules[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .meta
      .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    mutate(&mut fixture);
    let options = options_with_pass(None);
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(fixture, &options, [])));
    assert!(result.is_err());
  }

  #[test]
  fn rejects_every_dense_normal_slot_and_sorted_identity_mismatch() {
    assert_layout_rejected(|fixture| fixture.asts.clear());
    assert_layout_rejected(|fixture| fixture.asts[module_idx(0)] = None);
    assert_layout_rejected(|fixture| fixture.statements.clear());
    assert_layout_rejected(|fixture| {
      fixture.statements[module_idx(0)].infos.pop();
    });
    assert_layout_rejected(|fixture| {
      fixture.modules[module_idx(0)].as_normal_mut().expect("normal module").module_type =
        ModuleType::Json;
      fixture.statements[module_idx(0)].infos.pop();
    });
    assert_layout_rejected(|fixture| fixture.symbols = SymbolRefDb::new());
    assert_layout_rejected(|fixture| fixture.symbols[module_idx(0)] = None);
    assert_layout_rejected(|fixture| {
      fixture.resolutions = member_expr_resolutions(std::iter::empty::<
        Option<rolldown_common::MemberExprRefResolutionMap>,
      >());
    });
    assert_layout_rejected(|fixture| {
      fixture.resolutions = member_expr_resolutions([None]);
    });
    assert_layout_rejected(|fixture| {
      fixture.modules[module_idx(0)].as_normal_mut().expect("normal module").idx = module_idx(3);
    });
    assert_layout_rejected(|fixture| fixture.sorted = sorted_modules([module_idx(3)]));
    assert_layout_rejected(|fixture| {
      fixture.sorted = sorted_modules([module_idx(0), module_idx(0)]);
    });
  }

  #[test]
  fn rejects_external_slot_shape_and_embedded_identity_mismatches() {
    fn fixture_with_external() -> Fixture {
      let mut fixture = fixture(&[""], &[Vec::new()]);
      fixture.modules[module_idx(0)]
        .as_normal_mut()
        .expect("normal module")
        .meta
        .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
      append_external(&mut fixture, 1, "external");
      fixture
    }

    let mut fixture = fixture_with_external();
    let Module::External(external) = &mut fixture.modules[module_idx(1)] else {
      panic!("external test module")
    };
    external.idx = module_idx(4);
    let options = options_with_pass(None);
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(fixture, &options, [])));
    assert!(result.is_err());

    let mut fixture = fixture_with_external();
    fixture.asts[module_idx(1)] = Some(EcmaAst::default());
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(fixture, &options, [])));
    assert!(result.is_err());

    let mut fixture = fixture_with_external();
    fixture.symbols[module_idx(1)] = None;
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(fixture, &options, [])));
    assert!(result.is_err());

    let mut fixture = fixture_with_external();
    fixture.resolutions =
      member_expr_resolutions([Some(FxHashMap::default()), Some(FxHashMap::default())]);
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(fixture, &options, [])));
    assert!(result.is_err());

    let mut fixture = fixture_with_external();
    fixture.statements[module_idx(1)].add_stmt_info(StmtInfo::default());
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(fixture, &options, [])));
    assert!(result.is_err());
  }

  fn assert_related_identity_rejected(mutate: impl FnOnce(&mut Fixture, NodeId)) {
    let (mut fixture, node_id) = dynamic_call_fixture(false);
    mutate(&mut fixture, node_id);
    let options = options_with_pass(None);
    let result =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(fixture, &options, [])));
    assert!(result.is_err());
  }

  #[test]
  fn rejects_every_related_dynamic_import_identity_mismatch() {
    assert_related_identity_rejected(|fixture, node_id| {
      let mut entry = entry_point(4, EntryPointKind::UserDefined);
      entry.related_stmt_infos.push((
        module_idx(0),
        StmtInfoIdx::new(2),
        node_id,
        ImportRecordIdx::from_usize(0),
      ));
      let mut pipeline = PassPipelineCtx::new();
      let (_, entries) =
        run_infallible_pass(CanonicalizeEntriesPass, &mut pipeline, &fixture.modules, vec![entry]);
      fixture.entries = entries;
    });
    assert_related_identity_rejected(|fixture, node_id| {
      let mut entry = entry_point(1, EntryPointKind::DynamicImport);
      entry.related_stmt_infos.push((
        module_idx(4),
        StmtInfoIdx::new(2),
        node_id,
        ImportRecordIdx::from_usize(0),
      ));
      let mut pipeline = PassPipelineCtx::new();
      let (_, entries) =
        run_infallible_pass(CanonicalizeEntriesPass, &mut pipeline, &fixture.modules, vec![entry]);
      fixture.entries = entries;
    });
    assert_related_identity_rejected(|fixture, _| {
      fixture.modules[module_idx(0)].as_normal_mut().expect("normal importer").import_records
        [ImportRecordIdx::from_usize(0)]
      .kind = ImportKind::Require;
    });
    assert_related_identity_rejected(|fixture, _| {
      fixture.modules[module_idx(0)].as_normal_mut().expect("normal importer").import_records
        [ImportRecordIdx::from_usize(0)]
      .state
      .resolved_module = Some(module_idx(0));
    });
    assert_related_identity_rejected(|fixture, _| {
      fixture.modules[module_idx(0)].as_normal_mut().expect("normal importer").import_records
        [ImportRecordIdx::from_usize(0)]
      .dynamic_import_expr_info = None;
    });
    assert_related_identity_rejected(|fixture, node_id| {
      fixture.modules[module_idx(0)]
        .as_normal_mut()
        .expect("normal importer")
        .imports
        .remove(&node_id);
    });
    assert_related_identity_rejected(|fixture, node_id| {
      fixture.modules[module_idx(0)]
        .as_normal_mut()
        .expect("normal importer")
        .imports
        .insert(node_id, ImportRecordIdx::from_usize(4));
    });
    assert_related_identity_rejected(|fixture, _| {
      fixture.statements[module_idx(0)][StmtInfoIdx::new(2)].import_records.clear();
    });
    assert_related_identity_rejected(|fixture, node_id| {
      let mut entry = entry_point(1, EntryPointKind::DynamicImport);
      entry.related_stmt_infos.push((
        module_idx(0),
        StmtInfoIdx::new(1),
        node_id,
        ImportRecordIdx::from_usize(0),
      ));
      let mut pipeline = PassPipelineCtx::new();
      let (_, entries) =
        run_infallible_pass(CanonicalizeEntriesPass, &mut pipeline, &fixture.modules, vec![entry]);
      fixture.entries = entries;
    });
  }

  #[test]
  fn seals_unreachable_dynamic_imports() {
    fn assert_sealed(_: &Sealed<UnreachableDynamicImports>) {}
    let mut fixture = fixture(&[""], &[Vec::new()]);
    fixture.modules[module_idx(0)]
      .as_normal_mut()
      .expect("normal module")
      .meta
      .insert(EcmaViewMeta::TopExportedSideEffectsFreeFunction);
    let options = options_with_pass(None);
    let (unreachable, _, _) = run(fixture, &options, []);
    assert_sealed(&unreachable);
  }
}
