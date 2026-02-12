use oxc::{
  allocator::{Address, GetAddress, UnstableAddress},
  ast::{
    AstBuilder, AstKind,
    ast::{
      BindingPattern, Declaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
      ExportNamedDeclaration,
    },
  },
  ast_visit::{Visit, walk},
  semantic::ScopeFlags,
};
use rolldown_common::{
  AstScopes, ConstExportMeta, EcmaViewMeta, FlatOptions, GetLocalDb, ModuleIdx,
  SharedNormalizedBundlerOptions, SideEffectDetail, StmtInfoIdx, SymbolRef, SymbolRefDb,
  SymbolRefFlags,
};
use rolldown_ecmascript_utils::{ExpressionExt, is_top_level};
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_scanner::{
    const_eval::{ConstEvalCtx, try_extract_const_literal},
    side_effect_detector::SideEffectDetector,
  },
  module_finalizers::TraverseState,
};

use super::LinkStage;

type MutationResult = (
  Option<(ModuleIdx, FxHashMap<StmtInfoIdx, SideEffectDetail>)>,
  FxHashMap<SymbolRef, ConstExportMeta>,
  FxHashSet<Address>,
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
  #[expect(unused)]
  side_effects_free_function_optimization: bool,
  inline_const_optimization: bool,
}

type ModuleIdxAndStmtIdxToDynamicImportExprAddrMap =
  FxHashMap<ModuleIdx, FxHashMap<StmtInfoIdx, FxHashSet<Address>>>;

impl LinkStage<'_> {
  fn prepare_cross_module_optimization(&mut self) -> CrossModuleOptimizationConfig {
    let side_effect_free_function_symbols = self
      .module_table
      .iter()
      .zip(self.symbols.inner().iter())
      .filter_map(|(m, symbol_for_module)| {
        let normal_module = m.as_normal()?;
        let idx = normal_module.idx;
        normal_module
          .meta
          .contains(EcmaViewMeta::TopExportedSideEffectsFreeFunction)
          .then(move || {
            let symbol_for_module = symbol_for_module.as_ref()?;
            Some(symbol_for_module.flags.iter().filter_map(move |(symbol_id, flag)| {
              flag
                .contains(SymbolRefFlags::SideEffectsFreeFunction)
                .then_some(SymbolRef::from((idx, *symbol_id)))
            }))
          })
          .flatten()
      })
      .flatten()
      .collect::<FxHashSet<SymbolRef>>();
    self.side_effects_free_function_symbol_ref = side_effect_free_function_symbols;

    #[expect(clippy::bool_to_int_with_if)]
    let other_optimization_pass =
      if self.side_effects_free_function_symbol_ref.is_empty() { 0 } else { 1 };
    let cross_module_inline_const_pass = self.options.optimization.inline_const_pass() - 1;
    CrossModuleOptimizationConfig {
      pass: cross_module_inline_const_pass.max(other_optimization_pass),
      side_effects_free_function_optimization: !self
        .side_effects_free_function_symbol_ref
        .is_empty(),
      inline_const_optimization: cross_module_inline_const_pass >= 1,
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn cross_module_optimization(&mut self) -> FxHashSet<Address> {
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
    let mut unreachable_addresses = FxHashSet::default();
    // collect all modules that has dynamic import record
    // two dimension map module_idx -> stmt_idx -> dynamic_import_expression_address
    let mut module_idx_and_stmt_idx_to_dynamic_import_expr_addr_map = FxHashMap::default();
    self.entries.values().flatten().for_each(|entry| {
      entry.related_stmt_infos.iter().for_each(
        |(module_idx, stmt_idx, address, _import_record_idx)| {
          module_idx_and_stmt_idx_to_dynamic_import_expr_addr_map
            .entry(*module_idx)
            .or_insert_with(FxHashMap::default)
            .entry(*stmt_idx)
            .or_insert_with(FxHashSet::default)
            .insert(*address);
        },
      );
    });
    // Track modules to process in subsequent passes. None means process all modules (first pass).
    let mut modules_to_process: Option<FxHashSet<ModuleIdx>> = None;
    while ctx.config.pass > 0 && ctx.changed {
      ctx.config.pass -= 1;
      ctx.changed = false;
      let new_constant_refs = self.run(
        &mut ctx,
        &mut constant_symbol_map,
        &module_idx_and_stmt_idx_to_dynamic_import_expr_addr_map,
        &mut unreachable_addresses,
        modules_to_process.as_ref(),
      );
      if !ctx.changed {
        break;
      }
      modules_to_process = Some(self.find_modules_referencing_constants(&new_constant_refs));
    }
    self.global_constant_symbol_map = constant_symbol_map;
    // Return all unreachable import expression addresses instead of add it as a field of LinkStage,
    // Because this set is only used include statement stage.
    unreachable_addresses
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
    module_idx_and_stmt_idx_to_dynamic_import_expr_addr_map: &ModuleIdxAndStmtIdxToDynamicImportExprAddrMap,
    all_unreachable_addresses: &mut FxHashSet<Address>,
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
          let eval_ctx = ConstEvalCtx {
            ast: AstBuilder::new(&owner.allocator),
            scope: module_symbol_table.scoping(),
            overrode_get_constant_value_from_reference_id: Some(&|reference_id| {
              let reference = module_symbol_table.scoping().get_reference(reference_id);
              let symbol_id = reference.symbol_id()?;
              let symbol_ref: SymbolRef = (module_idx, symbol_id).into();
              let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
              constant_symbol_map
                .get(&canonical_ref)
                .map(|meta| oxc_ecmascript::constant_evaluation::ConstantValue::from(&meta.value))
            }),
            constant_map: &constant_map,
          };
          let default_stmt_idx_to_dynamic_import_expr_addr = FxHashMap::default();
          let stmt_idx_to_dynamic_import_expr_addr =
            module_idx_and_stmt_idx_to_dynamic_import_expr_addr_map
              .get(&module_idx)
              .unwrap_or(&default_stmt_idx_to_dynamic_import_expr_addr);
          let mut ctx = CrossModuleOptimizationRunnerContext {
            local_constant_symbol_map: FxHashMap::default(),
            side_effect_detail_mutations: FxHashMap::default(),
            scope_stack: vec![],
            traverse_state: TraverseState::empty(),
            side_effect_free_call_expr_addr: FxHashSet::default(),
            immutable_ctx: CrossModuleOptimizationImmutableCtx {
              eval_ctx: &eval_ctx,
              export_default_symbol: module.default_export_ref,
              module_idx,
              config: &cross_module_inline_const_ctx.config,
              global_side_effect_free_function_symbols: &self.side_effects_free_function_symbol_ref,
              symbols: &self.symbols,
              flat_options: self.flat_options,
              options: self.options,
              ast_scope: &self.symbols.local_db(module_idx).ast_scopes,
              stmt_idx_to_dynamic_import_expr_addr,
            },
            // `0` is preserved for namespace stmt
            toplevel_stmt_idx: StmtInfoIdx::from_raw_unchecked(1),
            visit_path: vec![],
            latest_side_effect_free_call_expr_addr: None,
            unreachable_import_expression_addresses: FxHashSet::default(),
          };
          ctx.visit_program(&dep.program);

          let side_effect_mutations = if ctx.side_effect_detail_mutations.is_empty() {
            None
          } else {
            Some((module_idx, ctx.side_effect_detail_mutations))
          };
          if side_effect_mutations.is_none()
            && ctx.local_constant_symbol_map.is_empty()
            && ctx.unreachable_import_expression_addresses.is_empty()
          {
            return None;
          }
          Some((
            side_effect_mutations,
            ctx.local_constant_symbol_map,
            ctx.unreachable_import_expression_addresses,
          ))
        })
      })
      .collect();

    let mut new_constant_refs = FxHashSet::default();
    for (side_effect_mutations, local_constants, unreachable_addresses) in mutation_result {
      if let Some((module_idx, mutations)) = side_effect_mutations {
        if let Some(module) = self.module_table[module_idx].as_normal_mut() {
          for (stmt_info_idx, side_effect_detail) in mutations {
            module.stmt_infos[stmt_info_idx].side_effect = side_effect_detail;
          }
        }
      }

      if !local_constants.is_empty() {
        cross_module_inline_const_ctx.changed = true;
        new_constant_refs.extend(local_constants.keys().copied());
        constant_symbol_map.extend(local_constants);
      }

      // Collect all unreachable import expression addresses
      all_unreachable_addresses.extend(unreachable_addresses);
    }
    new_constant_refs
  }
}

struct CrossModuleOptimizationImmutableCtx<'a, 'ast: 'a> {
  eval_ctx: &'a ConstEvalCtx<'a, 'ast>,
  export_default_symbol: SymbolRef,
  module_idx: ModuleIdx,
  config: &'a CrossModuleOptimizationConfig,
  global_side_effect_free_function_symbols: &'a FxHashSet<SymbolRef>,
  symbols: &'a SymbolRefDb,
  flat_options: FlatOptions,
  options: &'a SharedNormalizedBundlerOptions,
  ast_scope: &'a AstScopes,
  stmt_idx_to_dynamic_import_expr_addr: &'a FxHashMap<StmtInfoIdx, FxHashSet<Address>>,
}

struct CrossModuleOptimizationRunnerContext<'a, 'ast: 'a> {
  local_constant_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  side_effect_detail_mutations: FxHashMap<StmtInfoIdx, SideEffectDetail>,
  scope_stack: Vec<ScopeFlags>,
  traverse_state: TraverseState,
  side_effect_free_call_expr_addr: FxHashSet<Address>,
  immutable_ctx: CrossModuleOptimizationImmutableCtx<'a, 'ast>,
  toplevel_stmt_idx: StmtInfoIdx,
  visit_path: Vec<AstKind<'ast>>,
  latest_side_effect_free_call_expr_addr: Option<Address>,
  /// Import expressions that are inside lazy paths (e.g., inside a PURE-annotated function)
  /// and should be considered unreachable for chunk splitting purposes
  unreachable_import_expression_addresses: FxHashSet<Address>,
}

impl<'a, 'ast: 'a> std::ops::Deref for CrossModuleOptimizationRunnerContext<'a, 'ast> {
  type Target = CrossModuleOptimizationImmutableCtx<'a, 'ast>;

  fn deref(&self) -> &Self::Target {
    &self.immutable_ctx
  }
}

impl<'a, 'ast: 'a> Visit<'ast> for CrossModuleOptimizationRunnerContext<'a, 'ast> {
  fn enter_scope(
    &mut self,
    flags: oxc::semantic::ScopeFlags,
    _scope_id: &std::cell::Cell<Option<oxc::semantic::ScopeId>>,
  ) {
    self.scope_stack.push(flags);
    self.traverse_state.set(TraverseState::TopLevel, is_top_level(&self.scope_stack));
  }

  fn leave_scope(&mut self) {
    self.scope_stack.pop();
    self.traverse_state.set(TraverseState::TopLevel, is_top_level(&self.scope_stack));
  }

  fn enter_node(&mut self, kind: AstKind<'ast>) {
    self.visit_path.push(kind);
  }

  fn leave_node(&mut self, _kind: AstKind<'ast>) {
    self.visit_path.pop();
  }

  fn visit_program(&mut self, program: &oxc::ast::ast::Program<'ast>) {
    self.enter_scope(
      {
        let mut flags = ScopeFlags::Top;
        if program.source_type.is_strict() || program.has_use_strict_directive() {
          flags |= ScopeFlags::StrictMode;
        }
        flags
      },
      &program.scope_id,
    );
    // Custom visit
    for (idx, stmt) in program.body.iter().enumerate() {
      let pre_addr_len = self.side_effect_free_call_expr_addr.len();
      self.visit_statement(stmt);
      if pre_addr_len != self.side_effect_free_call_expr_addr.len() {
        let stmt_info_idx = StmtInfoIdx::new(idx + 1);
        let side_effect_detail = SideEffectDetector::new(
          self.immutable_ctx.ast_scope,
          self.immutable_ctx.flat_options,
          self.immutable_ctx.options,
          Some(&self.side_effect_free_call_expr_addr),
        )
        .detect_side_effect_of_stmt(stmt);
        self.side_effect_detail_mutations.insert(stmt_info_idx, side_effect_detail);
      }
      self.toplevel_stmt_idx += 1;
    }

    self.leave_scope();
  }

  fn visit_import_expression(&mut self, it: &oxc::ast::ast::ImportExpression<'ast>) {
    if let Some(addrs) = self.stmt_idx_to_dynamic_import_expr_addr.get(&self.toplevel_stmt_idx) {
      if addrs.contains(&it.unstable_address()) {
        // search the latest side effect free call expression parent
        let side_effect_free_call = self.visit_path.iter().rev().find_map(|ast| {
          let addr = self.latest_side_effect_free_call_expr_addr.as_ref()?;
          (&ast.address() == addr).then_some(ast)
        });
        if let Some(kind) = side_effect_free_call {
          let is_lazy_path = is_lazy_path_from_first_lazy_node(&self.visit_path, kind);
          if is_lazy_path {
            // This import expression is inside a lazy path (e.g., inside a PURE-annotated
            // function callback), so it's unreachable and can be ignored for chunk splitting
            self.unreachable_import_expression_addresses.insert(it.unstable_address());
          }
        }
      }
    }
    walk::walk_import_expression(self, it);
  }

  fn visit_call_expression(&mut self, it: &oxc::ast::ast::CallExpression<'ast>) {
    let mut pre_addr = None;
    if self.traverse_state.contains(TraverseState::TopLevel)
      || !self.immutable_ctx.stmt_idx_to_dynamic_import_expr_addr.is_empty()
    {
      let is_side_effects_free_function = it
        .callee
        .as_identifier()
        .and_then(|item| {
          let ref_id = item.reference_id.get()?;
          let symbol_id = self.immutable_ctx.eval_ctx.scope.get_reference(ref_id).symbol_id()?;

          let symbol_ref = self
            .immutable_ctx
            .symbols
            .canonical_ref_for((self.immutable_ctx.module_idx, symbol_id).into());
          Some(self.immutable_ctx.global_side_effect_free_function_symbols.contains(&symbol_ref))
        })
        .unwrap_or(false);

      if is_side_effects_free_function {
        self.side_effect_free_call_expr_addr.insert(it.unstable_address());
        pre_addr = self.latest_side_effect_free_call_expr_addr.replace(it.unstable_address());
      }
    }
    walk::walk_call_expression(self, it);
    if let Some(addr) = pre_addr {
      self.latest_side_effect_free_call_expr_addr = Some(addr);
    }
  }

  fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'ast>) {
    if it.source.is_none()
      && self.immutable_ctx.config.inline_const_optimization
      && let Some(ref decl) = it.declaration
      && let Declaration::VariableDeclaration(var_decl) = decl
    {
      var_decl.declarations.iter().for_each(|declarator| {
        if let BindingPattern::BindingIdentifier(ref binding) = declarator.id {
          let symbol_ref: SymbolRef = (self.immutable_ctx.module_idx, binding.symbol_id()).into();
          let is_not_assigned = self
            .immutable_ctx
            .symbols
            .local_db(self.immutable_ctx.module_idx)
            .flags
            .get(&symbol_ref.symbol)
            .is_some_and(|flag| flag.contains(SymbolRefFlags::IsNotReassigned));

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
    walk::walk_export_named_declaration(self, it);
  }

  fn visit_export_default_declaration(&mut self, it: &ExportDefaultDeclaration<'ast>) {
    // Only walk child nodes if we need to track import expressions (when
    // `stmt_idx_to_dynamic_import_expr_addr` is not empty) or if `inline_const_optimization` is enabled.
    // This placement ensures we skip walking when neither is needed.
    if !self.immutable_ctx.config.inline_const_optimization
      && self.immutable_ctx.stmt_idx_to_dynamic_import_expr_addr.is_empty()
    {
      return;
    }
    walk::walk_export_default_declaration(self, it);
    let Some(expr) = it.declaration.as_expression() else {
      return;
    };
    let local_binding_for_default_export = match &it.declaration {
      oxc::ast::match_expression!(ExportDefaultDeclarationKind) => None,
      ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => {
        fn_decl.id.as_ref().map(rolldown_ecmascript_utils::BindingIdentifierExt::expect_symbol_id)
      }
      ExportDefaultDeclarationKind::ClassDeclaration(cls_decl) => {
        cls_decl.id.as_ref().map(rolldown_ecmascript_utils::BindingIdentifierExt::expect_symbol_id)
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
  let target_addr = target.address();

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
    let addr = kind.address();
    // All nodes from first lazy node to target must be lazy
    if addr == target_addr {
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
