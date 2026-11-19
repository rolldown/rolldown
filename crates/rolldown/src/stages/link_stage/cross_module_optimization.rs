use std::cell::Cell;

use oxc::{
  ast::{
    AstKind,
    ast::{
      BindingIdentifier, BindingPattern, CallExpression, Declaration, ExportDeclaration,
      ExportDefaultDeclaration, ExportDefaultDeclarationKind, Expression, IdentifierReference,
      MemberExpression, Statement,
    },
    builder::AstBuilder,
  },
  ast_visit::{VisitJs, walk_js},
  semantic::{NodeId, SymbolId},
};
use oxc_ecmascript::ToBoolean;
use rolldown_common::{
  AstScopes, ConstExportMeta, EcmaViewMeta, ExportsKind, FlatOptions, GetLocalDb, IndexModules,
  MemberExprRefResolutionMap, ModuleIdx, SharedNormalizedBundlerOptions, Specifier, StmtEvalFlags,
  StmtInfoIdx, SymbolRef, SymbolRefDb, SymbolRefFlags,
};
use rolldown_ecmascript_utils::ExpressionExt;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_scanner::{
    const_eval::{ConstEvalCtx, eval_can_assign, try_extract_const_literal},
    stmt_eval_analyzer::StmtEvalAnalyzer,
  },
  type_alias::IndexStmtInfos,
};

use super::{
  LinkStage,
  refine_stmt_side_effects::{ImportCycles, LinkedConstantLookup},
};

type MutationResult = (
  Option<(ModuleIdx, FxHashMap<StmtInfoIdx, StmtEvalFlags>)>,
  FxHashMap<SymbolRef, ConstExportMeta>,
  FxHashSet<(ModuleIdx, NodeId)>,
);

#[derive(Default)]
struct CrossModuleOptimizationCtx {
  changed: bool,
  config: CrossModuleOptimizationConfig,
}

impl CrossModuleOptimizationCtx {
  fn new(config: CrossModuleOptimizationConfig) -> Self {
    Self { changed: true, config }
  }
}

#[derive(Default, Clone, Copy, Debug)]
struct CrossModuleOptimizationConfig {
  pass: u32,
  inline_const_optimization: bool,
}

type ModuleIdxAndStmtIdxToDynamicImportExprNodeIdMap =
  FxHashMap<ModuleIdx, FxHashMap<StmtInfoIdx, FxHashSet<NodeId>>>;

impl LinkStage<'_> {
  fn prepare_cross_module_optimization(&self) -> CrossModuleOptimizationConfig {
    let has_side_effect_free_functions = self.module_table.iter().any(|m| {
      m.as_normal()
        .is_some_and(|n| n.meta.contains(EcmaViewMeta::TopExportedSideEffectsFreeFunction))
    });

    #[expect(clippy::bool_to_int_with_if)]
    let other_optimization_pass = if has_side_effect_free_functions { 1 } else { 0 };
    let cross_module_inline_const_pass =
      self.options.optimization.inline_const_pass().saturating_sub(1);
    CrossModuleOptimizationConfig {
      pass: cross_module_inline_const_pass.max(other_optimization_pass),
      inline_const_optimization: cross_module_inline_const_pass >= 1,
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn cross_module_optimization(&mut self) -> FxHashSet<(ModuleIdx, NodeId)> {
    let config = self.prepare_cross_module_optimization();
    if config.pass < 1 {
      return FxHashSet::default();
    }
    // Explain `inline_const.pass`:
    // - if `inline_const.pass` is 1, we don't need the extra visit pass, since we already do it in
    // scan phase. This would already cover most of the cases, and the overhead is minimal.
    // - if `inline_const.pass` is greater than 1, and there is no cycle in module graph,
    // we could just revisit the ast of module in topological order only once.
    // - TODO:
    //  if there is cycle in module graph, and the `inline_const.pass` is greater than `1`, we
    //  should revisit the ast of the module for `inline_const.pass - 1` time.
    //  potential optimization:
    //  - if in one pass there is no new constant export found, we can stop the pass early.
    //  - if all dependencies of a module has no constant export, we don't need to visit ast at all.
    // The extra passes only run when user enable `inline_const` and set `pass` greater than 1.
    let mut ctx = CrossModuleOptimizationCtx::new(config);
    let mut constant_symbol_map = std::mem::take(&mut self.global_constant_symbol_map);
    let mut unreachable_node_ids = FxHashSet::default();
    // collect all modules that has dynamic import record
    // two dimension map module_idx -> stmt_idx -> dynamic_import_expression_node_id
    let mut module_idx_and_stmt_idx_to_dynamic_import_expr_node_id_map = FxHashMap::default();
    if self.options.treeshake.is_some() {
      for entry in self.entries.values().flatten() {
        for &(module_idx, stmt_idx, node_id, _) in &entry.related_stmt_infos {
          module_idx_and_stmt_idx_to_dynamic_import_expr_node_id_map
            .entry(module_idx)
            .or_insert_with(FxHashMap::default)
            .entry(stmt_idx)
            .or_insert_with(FxHashSet::default)
            .insert(node_id);
        }
      }
    }
    // Track modules to process in subsequent passes. None means process all modules (first pass).
    let mut modules_to_process: Option<FxHashSet<ModuleIdx>> = None;
    while ctx.config.pass > 0 && ctx.changed {
      ctx.config.pass -= 1;
      ctx.changed = false;
      let new_constant_refs = self.run(
        &mut ctx,
        &mut constant_symbol_map,
        &module_idx_and_stmt_idx_to_dynamic_import_expr_node_id_map,
        &mut unreachable_node_ids,
        modules_to_process.as_ref(),
      );
      if !ctx.changed {
        break;
      }
      modules_to_process = Some(self.find_modules_referencing_constants(&new_constant_refs));
    }
    self.global_constant_symbol_map = constant_symbol_map;
    // Return all unreachable import expression node IDs instead of add it as a field of LinkStage,
    // Because this set is only used include statement stage.
    unreachable_node_ids
  }

  /// A cyclic importer can read an exported `var` while it is still `undefined`.
  /// See internal-docs/inline-const/design.md.
  pub(super) fn exclude_cyclic_vars_from_inline_const(&mut self) {
    for module in self.module_table.modules.iter().filter_map(|module| module.as_normal()) {
      if module.exports_kind != ExportsKind::Esm
        || !module.importers_idx.iter().any(|&importer| {
          self.import_cycles.same_cycle(&self.module_table.modules, importer, module.idx)
        })
      {
        continue;
      }
      for export in self.metas[module.idx].resolved_exports.values() {
        let symbol_ref = self.symbols.canonical_ref_for(export.symbol_ref);
        if symbol_ref.owner == module.idx
          && self
            .symbols
            .local_db(module.idx)
            .scoping()
            .symbol_flags(symbol_ref.symbol)
            .is_function_scoped_declaration()
        {
          symbol_ref.flags_mut(&mut self.symbols).insert(SymbolRefFlags::ReadBeforeAssigned);
          self.global_constant_symbol_map.remove(&symbol_ref);
        }
      }
    }
  }

  /// A JSX element name can't be a literal, so a constant used as one is not inlined.
  pub(super) fn exclude_jsx_element_names_from_inline_const(&mut self) {
    if !self.symbols.has_module_preserve_jsx() || self.global_constant_symbol_map.is_empty() {
      return;
    }
    self.global_constant_symbol_map.retain(|symbol_ref, _| {
      symbol_ref.flags(&self.symbols).is_none_or(|flags| {
        !flags.intersects(
          SymbolRefFlags::MustStartWithCapitalLetterForJSX
            | SymbolRefFlags::UsedAsJSXMemberExprRoot,
        )
      })
    });
    // An import from a CJS module is not linked to the export, so the flag stays on the import.
    for meta in &self.metas {
      for (import_ref, cjs_module_idx) in &meta.named_import_to_cjs_module {
        let is_jsx_element_name = import_ref.flags(&self.symbols).is_some_and(|flags| {
          flags.intersects(
            SymbolRefFlags::MustStartWithCapitalLetterForJSX
              | SymbolRefFlags::UsedAsJSXMemberExprRoot,
          )
        });
        if is_jsx_element_name
          && let Some(alias) = &self.symbols.get(*import_ref).namespace_alias
          && let Some(export) =
            self.metas[*cjs_module_idx].resolved_exports.get(&alias.property_name)
        {
          self
            .global_constant_symbol_map
            .remove(&self.symbols.canonical_ref_for(export.symbol_ref));
        }
      }
    }
  }

  /// Find all modules that have imports resolving to any of the given constant canonical refs.
  fn find_modules_referencing_constants(
    &self,
    new_constant_refs: &FxHashSet<SymbolRef>,
  ) -> FxHashSet<ModuleIdx> {
    if new_constant_refs.is_empty() {
      return FxHashSet::default();
    }

    self
      .module_table
      .iter()
      .filter_map(|module| {
        let normal_module = module.as_normal()?;
        // Check if any of the module's named imports resolve to a newly discovered constant
        let references_new_constant = normal_module.named_imports.keys().any(|local_symbol_ref| {
          let canonical_ref = self.symbols.canonical_ref_for(*local_symbol_ref);
          new_constant_refs.contains(&canonical_ref)
        });
        references_new_constant.then_some(normal_module.idx)
      })
      .collect()
  }

  fn run(
    &mut self,
    cross_module_inline_const_ctx: &mut CrossModuleOptimizationCtx,
    constant_symbol_map: &mut FxHashMap<SymbolRef, ConstExportMeta>,
    module_idx_and_stmt_idx_to_dynamic_import_expr_node_id_map: &ModuleIdxAndStmtIdxToDynamicImportExprNodeIdMap,
    all_unreachable_node_ids: &mut FxHashSet<(ModuleIdx, NodeId)>,
    modules_to_process: Option<&FxHashSet<ModuleIdx>>,
  ) -> FxHashSet<SymbolRef> {
    let mutation_result: Vec<MutationResult> = self
      .sorted_modules
      .par_iter()
      .filter_map(|item| {
        if modules_to_process.is_some_and(|filter| !filter.contains(item)) {
          return None;
        }
        let module = self.module_table[*item].as_normal()?;
        let module_idx = module.idx;
        let ast =
          self.ast_table[module_idx].as_ref().expect("ast should be set in a normal module");
        // A dummy map to fits the api of `ConstEvalCtx`
        let constant_map = FxHashMap::default();
        ast.program.with_dependent(|owner, dep| {
          let module_symbol_table = self.symbols.local_db(module_idx);
          // `0` is preserved for namespace stmt
          let toplevel_stmt_idx = Cell::new(StmtInfoIdx::from_raw_unchecked(1));
          let eval_ctx = ConstEvalCtx {
            ast: AstBuilder::new(&owner.allocator),
            scope: module_symbol_table.scoping(),
            overrode_get_constant_value_from_reference_id: Some(&|reference_id| {
              let reference = module_symbol_table.scoping().get_reference(reference_id);
              let symbol_id = reference.symbol_id()?;
              let symbol_ref: SymbolRef = (module_idx, symbol_id).into();
              let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
              let value = constant_symbol_map.get(&canonical_ref).map(|meta| &meta.value)?;
              // A binding has no value before it is initialized. `ALIAS` in
              // `export const ALIAS = VALUE; export var VALUE = 1n;` is `undefined`, and an import
              // from a later module of the same cycle is in its TDZ.
              let lookup = LinkedConstantLookup {
                module_idx,
                stmt_idx: toplevel_stmt_idx.get(),
                modules: &self.module_table.modules,
                import_cycles: &self.import_cycles,
                symbols: &self.symbols,
                stmt_infos: &self.stmt_infos,
                constants: constant_symbol_map,
              };
              lookup.is_initialized(canonical_ref).then_some(value)
            }),
            constant_map: &constant_map,
          };
          let default_stmt_idx_to_dynamic_import_expr_node_ids = FxHashMap::default();
          let stmt_idx_to_dynamic_import_expr_node_ids =
            module_idx_and_stmt_idx_to_dynamic_import_expr_node_id_map
              .get(&module_idx)
              .unwrap_or(&default_stmt_idx_to_dynamic_import_expr_node_ids);
          // Local symbols of `import * as ns` bindings. Property reads on ES module namespace
          // objects are side-effect-free, so the `StmtEvalAnalyzer` re-run below must know
          // about them to drop `ns.fn()` calls to `@__NO_SIDE_EFFECTS__` functions. The scanner
          // builds the same set (see `add_star_import`), but it isn't persisted, so reconstruct
          // it here from the persisted named imports.
          let namespace_object_symbol_ids: FxHashSet<SymbolId> = module
            .named_imports
            .iter()
            .filter_map(|(local_ref, named_import)| {
              // A property read on a deferred namespace evaluates the deferred module.
              (matches!(named_import.imported, Specifier::Star)
                && !module.import_records[named_import.record_idx].is_defer_phase())
              .then_some(local_ref.symbol)
            })
            .collect();
          let mut ctx = CrossModuleOptimizationRunnerContext {
            local_constant_symbol_map: FxHashMap::default(),
            tree_shaking_flags_mutations: FxHashMap::default(),
            side_effect_free_call_expr_node_ids: FxHashSet::default(),
            has_separately_exported_call: false,
            immutable_ctx: CrossModuleOptimizationImmutableCtx {
              eval_ctx: &eval_ctx,
              export_default_symbol: module.default_export_ref,
              module_idx,
              config: &cross_module_inline_const_ctx.config,
              modules: &self.module_table.modules,
              import_cycles: &self.import_cycles,
              symbols: &self.symbols,
              flat_options: self.flat_options,
              options: self.options,
              ast_scope: &self.symbols.local_db(module_idx).ast_scopes,
              stmt_idx_to_dynamic_import_expr_node_ids,
              resolved_member_expr_refs: &self.metas[module_idx].resolved_member_expr_refs,
              namespace_object_symbol_ids: &namespace_object_symbol_ids,
              stmt_infos: &self.stmt_infos,
              constant_symbol_map,
            },
            toplevel_stmt_idx: &toplevel_stmt_idx,
            visit_path: vec![],
            latest_side_effect_free_call_expr_node_id: None,
            unreachable_import_expression_node_ids: FxHashSet::default(),
          };
          ctx.visit_program(&dep.program);

          let tree_shaking_flags_mutations = if ctx.tree_shaking_flags_mutations.is_empty() {
            None
          } else {
            Some((module_idx, ctx.tree_shaking_flags_mutations))
          };
          if tree_shaking_flags_mutations.is_none()
            && ctx.local_constant_symbol_map.is_empty()
            && ctx.unreachable_import_expression_node_ids.is_empty()
          {
            return None;
          }
          Some((
            tree_shaking_flags_mutations,
            ctx.local_constant_symbol_map,
            ctx
              .unreachable_import_expression_node_ids
              .into_iter()
              .map(|node_id| (module_idx, node_id))
              .collect(),
          ))
        })
      })
      .collect();

    let mut new_constant_refs = FxHashSet::default();
    for (tree_shaking_flags_mutations, local_constants, unreachable_node_ids) in mutation_result {
      if let Some((module_idx, mutations)) = tree_shaking_flags_mutations
        && self.module_table[module_idx].as_normal().is_some()
      {
        for (stmt_info_idx, tree_shaking_flags) in mutations {
          self.stmt_infos[module_idx][stmt_info_idx].eval_flags = tree_shaking_flags;
        }
        self.relaxed_side_effect_modules.insert(module_idx);
      }

      if !local_constants.is_empty() {
        cross_module_inline_const_ctx.changed = true;
        new_constant_refs.extend(local_constants.keys().copied());
        constant_symbol_map.extend(local_constants);
      }

      // Collect all unreachable import expression node IDs
      all_unreachable_node_ids.extend(unreachable_node_ids);
    }
    new_constant_refs
  }
}

struct CrossModuleOptimizationImmutableCtx<'a, 'ast: 'a> {
  eval_ctx: &'a ConstEvalCtx<'a, 'ast>,
  export_default_symbol: SymbolRef,
  module_idx: ModuleIdx,
  config: &'a CrossModuleOptimizationConfig,
  modules: &'a IndexModules,
  import_cycles: &'a ImportCycles,
  symbols: &'a SymbolRefDb,
  flat_options: FlatOptions,
  options: &'a SharedNormalizedBundlerOptions,
  ast_scope: &'a AstScopes,
  stmt_idx_to_dynamic_import_expr_node_ids: &'a FxHashMap<StmtInfoIdx, FxHashSet<NodeId>>,
  /// Resolution of namespace/named member expressions (`ns.fn`) to their target symbol,
  /// used to recognize `ns.fn()` calls to `@__NO_SIDE_EFFECTS__` functions.
  resolved_member_expr_refs: &'a MemberExprRefResolutionMap,
  /// Local symbols of `import * as ns` bindings, so property reads on them are treated as
  /// side-effect-free when re-detecting side effects of statements.
  namespace_object_symbol_ids: &'a FxHashSet<SymbolId>,
  stmt_infos: &'a IndexStmtInfos,
  /// Constants known so far, for the `StmtEvalAnalyzer` re-run. See `LinkedConstantLookup`.
  constant_symbol_map: &'a FxHashMap<SymbolRef, ConstExportMeta>,
}

struct CrossModuleOptimizationRunnerContext<'a, 'ast: 'a> {
  local_constant_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  tree_shaking_flags_mutations: FxHashMap<StmtInfoIdx, StmtEvalFlags>,
  side_effect_free_call_expr_node_ids: FxHashSet<NodeId>,
  has_separately_exported_call: bool,
  immutable_ctx: CrossModuleOptimizationImmutableCtx<'a, 'ast>,
  /// Shared with the constant lookup of `eval_ctx`.
  toplevel_stmt_idx: &'a Cell<StmtInfoIdx>,
  visit_path: Vec<AstKind<'ast>>,
  latest_side_effect_free_call_expr_node_id: Option<NodeId>,
  /// Import expressions that are inside lazy paths (e.g., inside a PURE-annotated function)
  /// and should be considered unreachable for chunk splitting purposes
  unreachable_import_expression_node_ids: FxHashSet<NodeId>,
}

impl<'a, 'ast: 'a> std::ops::Deref for CrossModuleOptimizationRunnerContext<'a, 'ast> {
  type Target = CrossModuleOptimizationImmutableCtx<'a, 'ast>;

  fn deref(&self) -> &Self::Target {
    &self.immutable_ctx
  }
}

impl<'ast> CrossModuleOptimizationRunnerContext<'_, 'ast> {
  fn local_symbol(&self, ident: &IdentifierReference) -> Option<SymbolRef> {
    let ref_id = ident.reference_id.get()?;
    let symbol_id = self.eval_ctx.scope.get_reference(ref_id).symbol_id()?;
    Some((self.immutable_ctx.module_idx, symbol_id).into())
  }

  fn resolve_member_binding(&self, member_expr: &MemberExpression) -> Option<SymbolRef> {
    let resolution = self.resolved_member_expr_refs.get(&member_expr.node_id())?;
    // The whole member must resolve to the binding. Trailing properties like `ns.fn.bar`
    // can read a getter or call a different function.
    if !resolution.prop_and_related_span_list.is_empty() {
      return None;
    }
    Some(self.symbols.canonical_ref_for(resolution.resolved?))
  }

  /// Resolve a call expression's callee to the canonical declaration symbol it targets, if it is
  /// a reference we can statically reason about:
  /// - a bare identifier callee (`fn()`), or
  /// - a namespace/named member-access callee that fully resolves to an exported binding
  ///   (`ns.fn()`).
  ///
  /// Handling the member-access form is what makes `@__NO_SIDE_EFFECTS__` apply to namespace
  /// imports across chunk boundaries (the call stays as `ns.fn()` until finalization, long after
  /// side-effect detection has run).
  fn resolve_callee_canonical_symbol(&self, callee: &Expression<'ast>) -> Option<SymbolRef> {
    if let Some(ident) = callee.as_identifier() {
      return Some(self.symbols.canonical_ref_for(self.local_symbol(ident)?));
    }
    self.resolve_member_binding(callee.as_member_expression()?)
  }

  /// Whether a canonical binding has its initialized value when the current statement runs.
  /// See `internal-docs/linking/module-execution-order/implementation.md`.
  fn binding_is_initialized(&self, symbol_ref: SymbolRef) -> bool {
    if symbol_ref.is_declared_by_function(self.symbols) {
      return true;
    }
    let module_idx = self.immutable_ctx.module_idx;
    if symbol_ref.owner == module_idx {
      let declaring_stmts = self.stmt_infos[module_idx].declared_stmts_by_symbol(&symbol_ref);
      !declaring_stmts.is_empty()
        && declaring_stmts
          .iter()
          .all(|&declaring_stmt| declaring_stmt < self.toplevel_stmt_idx.get())
    } else {
      self.modules[symbol_ref.owner].exec_order() < self.modules[module_idx].exec_order()
        && self.import_cycles.imports_finish_before_evaluation(self.modules, module_idx)
    }
  }

  fn binding_read_is_safe(&self, canonical_ref: SymbolRef) -> bool {
    let Some(owner) = self.modules[canonical_ref.owner].as_normal() else { return false };
    // Import lowering can turn an identifier read into a CommonJS or external property read.
    // See `internal-docs/linking/reference-needed-symbols/implementation.md`.
    if self.symbols.get(canonical_ref).namespace_alias.is_some() {
      return false;
    }
    if canonical_ref.owner != self.immutable_ctx.module_idx {
      return owner.exports_kind != ExportsKind::CommonJs
        && self.binding_is_initialized(canonical_ref);
    }
    // A local root-scope `var` already has the value `undefined` before its declaration.
    let flags = self.eval_ctx.scope.symbol_flags(canonical_ref.symbol);
    (flags.is_function_scoped_declaration()
      && canonical_ref.is_declared_in_root_scope(self.symbols))
      || self.binding_is_initialized(canonical_ref)
  }
}

impl<'a, 'ast: 'a> VisitJs<'ast> for CrossModuleOptimizationRunnerContext<'a, 'ast> {
  fn enter_node(&mut self, kind: AstKind<'ast>) {
    self.visit_path.push(kind);
  }

  fn leave_node(&mut self, _kind: AstKind<'ast>) {
    self.visit_path.pop();
  }

  fn visit_program(&mut self, program: &oxc::ast::ast::Program<'ast>) {
    // Custom visit
    for (idx, stmt) in program.body.iter().enumerate() {
      let pre_node_id_len = self.side_effect_free_call_expr_node_ids.len();
      self.visit_statement(stmt);
      let has_separately_exported_call = std::mem::take(&mut self.has_separately_exported_call);
      if pre_node_id_len != self.side_effect_free_call_expr_node_ids.len() {
        let stmt_info_idx = StmtInfoIdx::new(idx + 1);
        let constants = LinkedConstantLookup {
          module_idx: self.immutable_ctx.module_idx,
          stmt_idx: stmt_info_idx,
          modules: self.immutable_ctx.modules,
          import_cycles: self.immutable_ctx.import_cycles,
          symbols: self.immutable_ctx.symbols,
          stmt_infos: self.immutable_ctx.stmt_infos,
          constants: self.immutable_ctx.constant_symbol_map,
        };
        let flags = StmtEvalAnalyzer::new(
          self.immutable_ctx.ast_scope,
          self.immutable_ctx.flat_options,
          self.immutable_ctx.options,
          Some(&self.side_effect_free_call_expr_node_ids),
          Some(self.immutable_ctx.namespace_object_symbol_ids),
          Some(&constants),
        )
        .with_top_level_return_ends_module(
          !self.immutable_ctx.modules[self.immutable_ctx.module_idx]
            .as_normal()
            .is_some_and(|module| matches!(module.exports_kind, ExportsKind::Esm)),
        )
        .analyze_stmt(stmt)
        .tree_shaking_flags();
        // Cross-module optimization only refreshes tree-shaking flags. Module-level
        // execution-order sensitivity is recorded during AST scanning.
        let drops_effects = has_separately_exported_call
          && !flags.contains(StmtEvalFlags::UnknownSideEffect)
          && self.stmt_infos[self.immutable_ctx.module_idx][stmt_info_idx]
            .eval_flags
            .contains(StmtEvalFlags::UnknownSideEffect);
        if !drops_effects || StatementRemovalGuard::can_remove(self, stmt) {
          self.tree_shaking_flags_mutations.insert(stmt_info_idx, flags);
        }
      }
      self.toplevel_stmt_idx.set(self.toplevel_stmt_idx.get() + 1);
    }
  }

  fn visit_import_expression(&mut self, it: &oxc::ast::ast::ImportExpression<'ast>) {
    if let Some(node_ids) =
      self.stmt_idx_to_dynamic_import_expr_node_ids.get(&self.toplevel_stmt_idx.get())
    {
      if node_ids.contains(&it.node_id()) {
        // search the latest side effect free call expression parent
        let side_effect_free_call = self.visit_path.iter().rev().find_map(|ast| {
          let node_id = self.latest_side_effect_free_call_expr_node_id?;
          (ast.node_id() == node_id).then_some(ast)
        });
        if let Some(kind) = side_effect_free_call {
          let is_lazy_path = is_lazy_path_from_first_lazy_node(&self.visit_path, kind);
          if is_lazy_path {
            // This import expression is inside a lazy path (e.g., inside a PURE-annotated
            // function callback), so it's unreachable and can be ignored for chunk splitting
            self.unreachable_import_expression_node_ids.insert(it.node_id());
          }
        }
      }
    }
    walk_js::walk_import_expression(self, it);
  }

  fn visit_call_expression(&mut self, it: &CallExpression<'ast>) {
    let previous_lazy_call = self.latest_side_effect_free_call_expr_node_id;
    if let Some(symbol_ref) = self.resolve_callee_canonical_symbol(&it.callee)
      && symbol_ref.is_side_effect_free_function(self.symbols, self.modules)
      && symbol_ref.is_not_reassigned(self.symbols)
    {
      let flags = symbol_ref.flags(self.symbols).copied().unwrap_or_default();
      let exported_separately = flags.contains(SymbolRefFlags::ExportedSeparately);
      if !exported_separately
        || (self.binding_is_initialized(symbol_ref) && call_result_runs_no_code(&self.visit_path))
      {
        self.side_effect_free_call_expr_node_ids.insert(it.node_id());
        self.has_separately_exported_call |= exported_separately;
        // A truly empty function cannot invoke its callback arguments.
        if !flags.contains(SymbolRefFlags::PureAnnotationOnly)
          && !self.stmt_idx_to_dynamic_import_expr_node_ids.is_empty()
        {
          self.latest_side_effect_free_call_expr_node_id = Some(it.node_id());
        }
      }
    }
    walk_js::walk_call_expression(self, it);
    self.latest_side_effect_free_call_expr_node_id = previous_lazy_call;
  }

  fn visit_export_declaration(&mut self, it: &ExportDeclaration<'ast>) {
    if self.immutable_ctx.config.inline_const_optimization
      && let Declaration::VariableDeclaration(var_decl) = &it.declaration
    {
      var_decl.declarations.iter().for_each(|declarator| {
        if let BindingPattern::BindingIdentifier(ref binding) = declarator.id {
          let symbol_ref: SymbolRef = (self.immutable_ctx.module_idx, binding.symbol_id()).into();
          let local_db = self.immutable_ctx.symbols.local_db(self.immutable_ctx.module_idx);
          // `IsNotReassigned` does not see an assignment inside direct `eval`.
          let is_not_assigned = symbol_ref.is_not_reassigned(self.immutable_ctx.symbols)
            && !eval_can_assign(local_db.scoping(), symbol_ref.symbol);

          if is_not_assigned
            && let Some(value) = declarator
              .init
              .as_ref()
              .and_then(|expr| try_extract_const_literal(self.immutable_ctx.eval_ctx, expr))
          {
            if self.local_constant_symbol_map.get(&symbol_ref).map(|meta| &meta.value)
              != Some(&value)
            {
              self.local_constant_symbol_map.insert(symbol_ref, ConstExportMeta::new(value, false));
            }
          }
        }
      });
    }
    walk_js::walk_export_declaration(self, it);
  }

  fn visit_export_default_declaration(&mut self, it: &ExportDefaultDeclaration<'ast>) {
    // Only walk child nodes if we need to track import expressions (when
    // `stmt_idx_to_dynamic_import_expr_node_ids` is not empty) or if `inline_const_optimization` is enabled.
    // This placement ensures we skip walking when neither is needed.
    if !self.immutable_ctx.config.inline_const_optimization
      && self.immutable_ctx.stmt_idx_to_dynamic_import_expr_node_ids.is_empty()
    {
      return;
    }
    walk_js::walk_export_default_declaration(self, it);
    let Some(expr) = it.declaration.as_expression() else {
      return;
    };
    let local_binding_for_default_export = match &it.declaration {
      oxc::ast::match_expression!(ExportDefaultDeclarationKind) => None,
      ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => {
        fn_decl.id.as_ref().map(BindingIdentifier::symbol_id)
      }
      ExportDefaultDeclarationKind::ClassDeclaration(cls_decl) => {
        cls_decl.id.as_ref().map(BindingIdentifier::symbol_id)
      }
      ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => unreachable!(),
    };

    let symbol_id =
      local_binding_for_default_export.unwrap_or(self.immutable_ctx.export_default_symbol.symbol);
    let symbol_ref: SymbolRef = (self.immutable_ctx.module_idx, symbol_id).into();
    if let Some(v) = try_extract_const_literal(self.immutable_ctx.eval_ctx, expr) {
      if self.local_constant_symbol_map.get(&symbol_ref).map(|meta| &meta.value) != Some(&v) {
        self.local_constant_symbol_map.insert(symbol_ref, ConstExportMeta::new(v, false));
      }
    }
  }
}

/// Checks effects that the general statement analyzer does not model. Run this only when a
/// separately exported call would let us relax a statement's side-effect flags. Check the whole
/// statement because other expressions in it can also read bindings or fail to terminate.
struct StatementRemovalGuard<'ctx, 'a, 'ast> {
  ctx: &'ctx CrossModuleOptimizationRunnerContext<'a, 'ast>,
  can_remove: bool,
}

impl<'ast> StatementRemovalGuard<'_, '_, 'ast> {
  fn can_remove(
    ctx: &CrossModuleOptimizationRunnerContext<'_, 'ast>,
    stmt: &Statement<'ast>,
  ) -> bool {
    let mut guard = StatementRemovalGuard { ctx, can_remove: true };
    guard.visit_statement(stmt);
    guard.can_remove
  }
}

impl<'ast> VisitJs<'ast> for StatementRemovalGuard<'_, '_, 'ast> {
  fn visit_statement(&mut self, stmt: &Statement<'ast>) {
    let loop_test = match stmt {
      Statement::WhileStatement(stmt) => Some(&stmt.test),
      Statement::DoWhileStatement(stmt) => Some(&stmt.test),
      _ => None,
    };
    self.can_remove &= loop_test
      .is_none_or(|test| test.get_inner_expression().to_boolean(self.ctx.eval_ctx) == Some(false));
    if self.can_remove {
      walk_js::walk_statement(self, stmt);
    }
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference<'ast>) {
    if self.can_remove
      && let Some(local_ref) = self.ctx.local_symbol(ident)
    {
      self.can_remove =
        self.ctx.binding_read_is_safe(self.ctx.symbols.canonical_ref_for(local_ref));
    }
  }

  fn visit_member_expression(&mut self, member: &MemberExpression<'ast>) {
    // The statement analyzer assumes a namespace read only evaluates the key. Require a
    // resolved binding here, since CommonJS getters and ESM TDZ reads can both have effects.
    if self.can_remove
      && let Some(ident) = member.object().as_identifier()
      && let Some(local_ref) = self.ctx.local_symbol(ident)
      && self.ctx.namespace_object_symbol_ids.contains(&local_ref.symbol)
    {
      self.can_remove = self
        .ctx
        .resolve_member_binding(member)
        .is_some_and(|symbol_ref| self.ctx.binding_read_is_safe(symbol_ref));
    }
    if self.can_remove {
      walk_js::walk_member_expression(self, member);
    }
  }

  fn visit_call_expression(&mut self, call: &CallExpression<'ast>) {
    if !self.can_remove {
      return;
    }
    if self.ctx.side_effect_free_call_expr_node_ids.contains(&call.node_id()) {
      self.visit_expression(&call.callee);
      for arg in &call.arguments {
        if arg.as_expression().is_some_and(|expr| {
          matches!(
            expr.get_inner_expression(),
            Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
          )
        }) {
          // Creating a callback does not evaluate its body or parameters. The pure call
          // contract covers any effects of invoking it.
          continue;
        }
        self.visit_argument(arg);
      }
    } else {
      walk_js::walk_call_expression(self, call);
    }
  }
}

/// Whether the call whose ancestors are `visit_path` runs no code on its result. The result can be
/// discarded, bound to a plain name, exported as default, or tested by `if` or `switch`.
/// Destructuring, coercion and passing the result to another call can run code on the result.
fn call_result_runs_no_code(visit_path: &[AstKind]) -> bool {
  let mut ancestors = visit_path.iter().rev();
  while let Some(kind) = ancestors.next() {
    match kind {
      AstKind::ChainExpression(_) | AstKind::ParenthesizedExpression(_) => {}
      AstKind::ExpressionStatement(_)
      | AstKind::ExportDefaultDeclaration(_)
      | AstKind::IfStatement(_)
      | AstKind::SwitchStatement(_)
      | AstKind::SwitchCase(_) => return true,
      AstKind::VariableDeclarator(declarator) => {
        let Some(AstKind::VariableDeclaration(decl)) = ancestors.next() else { return false };
        return !decl.kind.is_using()
          && matches!(declarator.id, BindingPattern::BindingIdentifier(_));
      }
      _ => return false,
    }
  }
  false
}

/// If ast path from first lazy node to the terminal node are all lazy.
/// e.g.
/// ```js
/// /*@__PURE__ */Foo(() => {
///   import('mod')
/// })
/// ```
/// CallExpr -> ExprStmt -> ArrowFunc -> ImportExpr(target)
/// eager       eager       lazy        lazy
fn is_lazy_path_from_first_lazy_node<'ast>(
  visit_path: &[AstKind<'ast>],
  target: &AstKind<'ast>,
) -> bool {
  let target_node_id = target.node_id();

  // Find the last lazy node after the target node
  let last_lazy_idx = visit_path.iter().rposition(|kind| {
    matches!(
      kind,
      AstKind::ArrowFunctionExpression(_) | AstKind::Function(_) | AstKind::FunctionBody(_)
    )
  });

  let Some(last_lazy_idx) = last_lazy_idx else {
    // No lazy node found, path is reachable
    return false;
  };

  for kind in visit_path[..=last_lazy_idx].iter().rev() {
    // All nodes from first lazy node to target must be lazy
    if kind.node_id() == target_node_id {
      return true;
    }
    if !matches!(
      kind,
      AstKind::ArrowFunctionExpression(_) | AstKind::Function(_) | AstKind::FunctionBody(_)
    ) {
      return false;
    }
  }

  // If the visit path did not included the target node, return false
  false
}
