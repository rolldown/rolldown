use oxc::{
  ast::{
    AstKind,
    ast::{
      BindingIdentifier, BindingPattern, Declaration, ExportDefaultDeclaration,
      ExportDefaultDeclarationKind, ExportNamedDeclaration, Expression,
    },
    builder::AstBuilder,
  },
  ast_visit::{Visit, walk},
  semantic::{NodeId, SymbolId},
};
use rolldown_common::{
  AstScopes, ConstExportMeta, FlatOptions, GetLocalDb, IndexModules, MemberExprRefResolutionMap,
  ModuleIdx, NormalModule, SharedNormalizedBundlerOptions, Specifier, StmtEvalFlags, StmtInfoIdx,
  SymbolRef, SymbolRefDb, SymbolRefFlags,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_ecmascript_utils::ExpressionExt;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_scanner::{
    const_eval::{ConstEvalCtx, try_extract_const_literal},
    stmt_eval_analyzer::StmtEvalAnalyzer,
  },
  stages::link_stage::passes::GlobalConstantsDraft,
};

#[derive(Clone, Copy)]
pub(in crate::stages::link_stage) struct ModuleOptimizationInput<'a> {
  pub module: &'a NormalModule,
  pub ast: &'a EcmaAst,
  pub modules: &'a IndexModules,
  pub symbols: &'a SymbolRefDb,
  pub global_constants: &'a GlobalConstantsDraft,
  pub flat_options: FlatOptions,
  pub options: &'a SharedNormalizedBundlerOptions,
  pub inline_const_optimization: bool,
  pub stmt_idx_to_dynamic_import_expr_node_ids:
    Option<&'a FxHashMap<StmtInfoIdx, FxHashSet<NodeId>>>,
  pub resolved_member_expr_refs: &'a MemberExprRefResolutionMap,
}

pub(in crate::stages::link_stage) struct ModuleOptimizationMutation {
  pub eval_flags: FxHashMap<StmtInfoIdx, StmtEvalFlags>,
  pub constants: FxHashMap<SymbolRef, ConstExportMeta>,
  pub unreachable_nodes: FxHashSet<NodeId>,
}

pub(in crate::stages::link_stage) fn analyze_module(
  input: ModuleOptimizationInput<'_>,
) -> Option<ModuleOptimizationMutation> {
  let module_idx = input.module.idx;
  let constant_map = FxHashMap::default();
  input.ast.program.with_dependent(|owner, dependent| {
    let module_symbols = input.symbols.local_db(module_idx);
    let eval_ctx = ConstEvalCtx {
      ast: AstBuilder::new(&owner.allocator),
      scope: module_symbols.scoping(),
      overrode_get_constant_value_from_reference_id: Some(&|reference_id| {
        let reference = module_symbols.scoping().get_reference(reference_id);
        let symbol_id = reference.symbol_id()?;
        let canonical_ref = input.symbols.canonical_ref_for((module_idx, symbol_id).into());
        input
          .global_constants
          .get(&canonical_ref)
          .map(|meta| oxc_ecmascript::constant_evaluation::ConstantValue::from(&meta.value))
      }),
      constant_map: &constant_map,
    };
    let empty_dynamic_imports = FxHashMap::default();
    let stmt_idx_to_dynamic_import_expr_node_ids =
      input.stmt_idx_to_dynamic_import_expr_node_ids.unwrap_or(&empty_dynamic_imports);
    // The scanner builds the same set in `add_star_import`, but does not persist it. Rebuild it
    // from the owner-local named imports so statement evaluation preserves namespace-read rules.
    let namespace_object_symbol_ids = input
      .module
      .named_imports
      .iter()
      .filter_map(|(local_ref, named_import)| {
        std::matches!(named_import.imported, Specifier::Star).then_some(local_ref.symbol)
      })
      .collect::<FxHashSet<SymbolId>>();
    let mut runner = ModuleOptimizationRunner {
      local_constants: FxHashMap::default(),
      eval_flags: FxHashMap::default(),
      side_effect_free_call_expr_node_ids: FxHashSet::default(),
      facts: ModuleOptimizationFacts {
        eval_ctx: &eval_ctx,
        export_default_symbol: input.module.default_export_ref,
        module_idx,
        inline_const_optimization: input.inline_const_optimization,
        modules: input.modules,
        symbols: input.symbols,
        flat_options: input.flat_options,
        options: input.options,
        ast_scope: &module_symbols.ast_scopes,
        stmt_idx_to_dynamic_import_expr_node_ids,
        resolved_member_expr_refs: input.resolved_member_expr_refs,
        namespace_object_symbol_ids: &namespace_object_symbol_ids,
      },
      toplevel_stmt_idx: StmtInfoIdx::from_raw_unchecked(1),
      visit_path: Vec::new(),
      latest_side_effect_free_call_expr_node_id: None,
      unreachable_nodes: FxHashSet::default(),
    };
    runner.visit_program(&dependent.program);
    if runner.eval_flags.is_empty()
      && runner.local_constants.is_empty()
      && runner.unreachable_nodes.is_empty()
    {
      return None;
    }
    Some(ModuleOptimizationMutation {
      eval_flags: runner.eval_flags,
      constants: runner.local_constants,
      unreachable_nodes: runner.unreachable_nodes,
    })
  })
}

struct ModuleOptimizationFacts<'a, 'ast: 'a> {
  eval_ctx: &'a ConstEvalCtx<'a, 'ast>,
  export_default_symbol: SymbolRef,
  module_idx: ModuleIdx,
  inline_const_optimization: bool,
  modules: &'a IndexModules,
  symbols: &'a SymbolRefDb,
  flat_options: FlatOptions,
  options: &'a SharedNormalizedBundlerOptions,
  ast_scope: &'a AstScopes,
  stmt_idx_to_dynamic_import_expr_node_ids: &'a FxHashMap<StmtInfoIdx, FxHashSet<NodeId>>,
  resolved_member_expr_refs: &'a MemberExprRefResolutionMap,
  namespace_object_symbol_ids: &'a FxHashSet<SymbolId>,
}

struct ModuleOptimizationRunner<'a, 'ast: 'a> {
  local_constants: FxHashMap<SymbolRef, ConstExportMeta>,
  eval_flags: FxHashMap<StmtInfoIdx, StmtEvalFlags>,
  side_effect_free_call_expr_node_ids: FxHashSet<NodeId>,
  facts: ModuleOptimizationFacts<'a, 'ast>,
  toplevel_stmt_idx: StmtInfoIdx,
  visit_path: Vec<AstKind<'ast>>,
  latest_side_effect_free_call_expr_node_id: Option<NodeId>,
  unreachable_nodes: FxHashSet<NodeId>,
}

impl ModuleOptimizationRunner<'_, '_> {
  fn resolve_callee_canonical_symbol(&self, callee: &Expression<'_>) -> Option<SymbolRef> {
    if let Some(identifier) = callee.as_identifier() {
      let reference_id = identifier.reference_id.get()?;
      let symbol_id = self.facts.eval_ctx.scope.get_reference(reference_id).symbol_id()?;
      return Some(self.facts.symbols.canonical_ref_for((self.facts.module_idx, symbol_id).into()));
    }

    let member_expr = callee.as_member_expression()?;
    let resolution = self.facts.resolved_member_expr_refs.get(&member_expr.node_id())?;
    if !resolution.prop_and_related_span_list.is_empty() {
      return None;
    }
    Some(self.facts.symbols.canonical_ref_for(resolution.resolved?))
  }
}

impl<'ast> Visit<'ast> for ModuleOptimizationRunner<'_, 'ast> {
  fn enter_node(&mut self, kind: AstKind<'ast>) {
    self.visit_path.push(kind);
  }

  fn leave_node(&mut self, _kind: AstKind<'ast>) {
    self.visit_path.pop();
  }

  fn visit_program(&mut self, program: &oxc::ast::ast::Program<'ast>) {
    for (index, statement) in program.body.iter().enumerate() {
      let previous_side_effect_free_call_count = self.side_effect_free_call_expr_node_ids.len();
      self.visit_statement(statement);
      if previous_side_effect_free_call_count != self.side_effect_free_call_expr_node_ids.len() {
        let stmt_info_idx = StmtInfoIdx::new(index + 1);
        let facts = StmtEvalAnalyzer::new(
          self.facts.ast_scope,
          self.facts.flat_options,
          self.facts.options,
          Some(&self.side_effect_free_call_expr_node_ids),
          Some(self.facts.namespace_object_symbol_ids),
        )
        .analyze_stmt(statement);
        // Cross-module optimization refreshes only tree-shaking flags. Execution-order
        // sensitivity remains the scan-stage fact.
        self.eval_flags.insert(stmt_info_idx, facts.tree_shaking_flags());
      }
      self.toplevel_stmt_idx += 1;
    }
  }

  fn visit_import_expression(&mut self, expression: &oxc::ast::ast::ImportExpression<'ast>) {
    if self
      .facts
      .stmt_idx_to_dynamic_import_expr_node_ids
      .get(&self.toplevel_stmt_idx)
      .is_some_and(|node_ids| node_ids.contains(&expression.node_id()))
    {
      let side_effect_free_call = self.visit_path.iter().rev().find_map(|kind| {
        let node_id = self.latest_side_effect_free_call_expr_node_id?;
        (kind.node_id() == node_id).then_some(kind)
      });
      if let Some(kind) = side_effect_free_call
        && is_lazy_path_from_first_lazy_node(&self.visit_path, kind)
      {
        self.unreachable_nodes.insert(expression.node_id());
      }
    }
    walk::walk_import_expression(self, expression);
  }

  fn visit_call_expression(&mut self, expression: &oxc::ast::ast::CallExpression<'ast>) {
    let mut previous_node_id = None;
    let (is_side_effect_free_function, is_pure_annotation_only) = self
      .resolve_callee_canonical_symbol(&expression.callee)
      .map(|symbol_ref| {
        let is_side_effect_free = symbol_ref
          .is_side_effect_free_function(self.facts.symbols, self.facts.modules)
          && symbol_ref.is_not_reassigned(self.facts.symbols);
        let is_annotation_only = is_side_effect_free
          && self
            .facts
            .symbols
            .local_db(symbol_ref.owner)
            .flags
            .get(&symbol_ref.symbol)
            .is_some_and(|flags| flags.contains(SymbolRefFlags::PureAnnotationOnly));
        (is_side_effect_free, is_annotation_only)
      })
      .unwrap_or((false, false));

    if is_side_effect_free_function {
      self.side_effect_free_call_expr_node_ids.insert(expression.node_id());
      if !is_pure_annotation_only {
        previous_node_id =
          self.latest_side_effect_free_call_expr_node_id.replace(expression.node_id());
      }
    }
    walk::walk_call_expression(self, expression);
    // Preserve the existing behavior: when there was no previous node, the current node remains
    // installed instead of being reset to `None`.
    if let Some(node_id) = previous_node_id {
      self.latest_side_effect_free_call_expr_node_id = Some(node_id);
    }
  }

  fn visit_export_named_declaration(&mut self, declaration: &ExportNamedDeclaration<'ast>) {
    if declaration.source.is_none()
      && self.facts.inline_const_optimization
      && let Some(Declaration::VariableDeclaration(variable)) = &declaration.declaration
    {
      for declarator in &variable.declarations {
        if let BindingPattern::BindingIdentifier(binding) = &declarator.id {
          let symbol_ref = (self.facts.module_idx, binding.symbol_id()).into();
          let is_not_assigned = self
            .facts
            .symbols
            .local_db(self.facts.module_idx)
            .flags
            .get(&binding.symbol_id())
            .is_some_and(|flags| flags.contains(SymbolRefFlags::IsNotReassigned));
          if is_not_assigned
            && let Some(value) = declarator
              .init
              .as_ref()
              .and_then(|expression| try_extract_const_literal(self.facts.eval_ctx, expression))
            && self.local_constants.get(&symbol_ref).map(|meta| &meta.value) != Some(&value)
          {
            self.local_constants.insert(symbol_ref, ConstExportMeta::new(value, false));
          }
        }
      }
    }
    walk::walk_export_named_declaration(self, declaration);
  }

  fn visit_export_default_declaration(&mut self, declaration: &ExportDefaultDeclaration<'ast>) {
    if !self.facts.inline_const_optimization
      && self.facts.stmt_idx_to_dynamic_import_expr_node_ids.is_empty()
    {
      return;
    }
    walk::walk_export_default_declaration(self, declaration);
    let Some(expression) = declaration.declaration.as_expression() else { return };
    let local_binding_for_default_export = match &declaration.declaration {
      ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
        function.id.as_ref().map(BindingIdentifier::symbol_id)
      }
      ExportDefaultDeclarationKind::ClassDeclaration(class) => {
        class.id.as_ref().map(BindingIdentifier::symbol_id)
      }
      ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => std::unreachable!(
        "TypeScript interface declarations must be removed before cross-module optimization"
      ),
      _ => None,
    };
    let symbol_id =
      local_binding_for_default_export.unwrap_or(self.facts.export_default_symbol.symbol);
    let symbol_ref = (self.facts.module_idx, symbol_id).into();
    if let Some(value) = try_extract_const_literal(self.facts.eval_ctx, expression)
      && self.local_constants.get(&symbol_ref).map(|meta| &meta.value) != Some(&value)
    {
      self.local_constants.insert(symbol_ref, ConstExportMeta::new(value, false));
    }
  }
}

fn is_lazy_path_from_first_lazy_node<'ast>(
  visit_path: &[AstKind<'ast>],
  target: &AstKind<'ast>,
) -> bool {
  let target_node_id = target.node_id();
  let last_lazy_index = visit_path.iter().rposition(|kind| {
    std::matches!(
      kind,
      AstKind::ArrowFunctionExpression(_) | AstKind::Function(_) | AstKind::FunctionBody(_)
    )
  });
  let Some(last_lazy_index) = last_lazy_index else { return false };
  for kind in visit_path[..=last_lazy_index].iter().rev() {
    if kind.node_id() == target_node_id {
      return true;
    }
    let is_lazy = std::matches!(
      kind,
      AstKind::ArrowFunctionExpression(_) | AstKind::Function(_) | AstKind::FunctionBody(_)
    );
    if !is_lazy {
      return false;
    }
  }
  false
}
