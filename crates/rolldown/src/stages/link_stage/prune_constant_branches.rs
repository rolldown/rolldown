use oxc::{
  ast::{ast, builder::AstBuilder},
  ast_visit::{VisitJs, walk_js},
  semantic::{NodeId, ReferenceId},
};
use oxc_ecmascript::constant_evaluation::ConstantEvaluation;
use rolldown_common::{EcmaViewMeta, GetLocalDb, SymbolOrMemberExprRef, SymbolRef};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ast_scanner::const_eval::ConstEvalCtx;

use super::LinkStage;

impl LinkStage<'_> {
  /// References may only be removed if finalization deletes exactly the same dead subtree,
  /// even with minification disabled. See internal-docs/constant-branches/implementation.md.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn prune_constant_branches(&mut self) {
    if self.options.treeshake.is_none() || self.global_constant_symbol_map.is_empty() {
      return;
    }
    self.stmt_infos.par_iter_mut().zip(self.metas.par_iter_mut()).enumerate().for_each(
      |(idx, (stmt_infos, meta))| {
        let module_idx = rolldown_common::ModuleIdx::new(idx);
        let Some(module) = self.module_table[module_idx].as_normal() else { return };
        if module.meta.contains(EcmaViewMeta::Eval) {
          return;
        }
        let Some(ast) = self.ast_table[module_idx].as_ref() else { return };
        ast.program.with_dependent(|owner, dep| {
          let scope = self.symbols.local_db(module_idx).scoping();
          let constants = FxHashMap::default();
          let get_constant = |reference_id: ReferenceId| {
            let symbol = scope.get_reference(reference_id).symbol_id()?;
            let symbol_ref: SymbolRef = (module_idx, symbol).into();
            let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
            let constant = self.global_constant_symbol_map.get(&canonical_ref)?;
            // CJS properties are not immutable ESM bindings.
            (!constant.commonjs_export && canonical_ref.is_not_reassigned(&self.symbols))
              .then(|| (&constant.value).into())
          };
          let eval_ctx = ConstEvalCtx {
            ast: AstBuilder::new(&owner.allocator),
            scope,
            constant_map: &constants,
            overrode_get_constant_value_from_reference_id: Some(&get_constant),
          };
          for (stmt, info) in dep.program.body.iter().zip(stmt_infos.infos.iter_mut().skip(1)) {
            // Import lowering adds synthetic references to these statements. Until those
            // references and import records can be pruned together, leave them untouched.
            if !info.import_records.is_empty()
              || !info.referenced_symbols.iter().any(|reference| {
                let canonical_ref = self.symbols.canonical_ref_for(*reference.symbol_ref());
                self.global_constant_symbol_map.contains_key(&canonical_ref)
              })
            {
              continue;
            }
            let mut visitor = BranchVisitor {
              eval_ctx: &eval_ctx,
              branches: &mut meta.constant_branches,
              live_symbols: FxHashSet::default(),
              live_references: FxHashSet::default(),
              dead_symbols: FxHashSet::default(),
              dead_references: FxHashSet::default(),
              in_dead_branch: false,
              changed: false,
            };
            visitor.visit_statement(stmt);
            if visitor.changed {
              // Only remove references witnessed exclusively in deleted subtrees. Other
              // scan/link dependencies may be synthetic and have no IdentifierReference.
              info.referenced_symbols.retain(|reference| match reference {
                SymbolOrMemberExprRef::Symbol(symbol) => {
                  symbol.owner != module_idx
                    || !visitor.dead_symbols.contains(&symbol.symbol)
                    || visitor.live_symbols.contains(&symbol.symbol)
                }
                SymbolOrMemberExprRef::MemberExpr(member) => member.reference_id.is_none_or(|id| {
                  !visitor.dead_references.contains(&id) || visitor.live_references.contains(&id)
                }),
              });
            }
          }
        });
      },
    );
  }
}

struct BranchVisitor<'a, 'ast> {
  eval_ctx: &'a ConstEvalCtx<'a, 'ast>,
  branches: &'a mut FxHashMap<NodeId, bool>,
  live_symbols: FxHashSet<oxc::semantic::SymbolId>,
  live_references: FxHashSet<ReferenceId>,
  dead_symbols: FxHashSet<oxc::semantic::SymbolId>,
  dead_references: FxHashSet<ReferenceId>,
  in_dead_branch: bool,
  changed: bool,
}

impl<'ast> VisitJs<'ast> for BranchVisitor<'_, 'ast> {
  fn visit_identifier_reference(&mut self, it: &ast::IdentifierReference<'ast>) {
    if let Some(id) = it.reference_id.get() {
      let (references, symbols) = if self.in_dead_branch {
        (&mut self.dead_references, &mut self.dead_symbols)
      } else {
        (&mut self.live_references, &mut self.live_symbols)
      };
      references.insert(id);
      if let Some(symbol) = self.eval_ctx.scope.get_reference(id).symbol_id() {
        symbols.insert(symbol);
      }
    }
  }

  fn visit_if_statement(&mut self, it: &ast::IfStatement<'ast>) {
    if !self.in_dead_branch
      && let Some(value) = it.test.evaluate_value_to_boolean(self.eval_ctx)
      // Deleting a branch with hoisted declarations needs additional bookkeeping.
      && !has_hoisted_declarations(if value { it.alternate.as_ref() } else { Some(&it.consequent) })
    {
      self.changed = true;
      self.branches.insert(it.node_id(), value);
      self.visit_expression(&it.test);
      if value {
        self.visit_statement(&it.consequent);
      } else if let Some(alternate) = &it.alternate {
        self.visit_statement(alternate);
      }
      self.in_dead_branch = true;
      if value {
        if let Some(alternate) = &it.alternate {
          self.visit_statement(alternate);
        }
      } else {
        self.visit_statement(&it.consequent);
      }
      self.in_dead_branch = false;
    } else {
      walk_js::walk_if_statement(self, it);
    }
  }

  fn visit_conditional_expression(&mut self, it: &ast::ConditionalExpression<'ast>) {
    if !self.in_dead_branch
      && let Some(value) = it.test.evaluate_value_to_boolean(self.eval_ctx)
    {
      self.changed = true;
      self.branches.insert(it.node_id(), value);
      self.visit_expression(&it.test);
      self.visit_expression(if value { &it.consequent } else { &it.alternate });
      self.in_dead_branch = true;
      self.visit_expression(if value { &it.alternate } else { &it.consequent });
      self.in_dead_branch = false;
    } else {
      walk_js::walk_conditional_expression(self, it);
    }
  }

  fn visit_logical_expression(&mut self, it: &ast::LogicalExpression<'ast>) {
    if self.in_dead_branch {
      walk_js::walk_logical_expression(self, it);
      return;
    }
    let executes_rhs = match it.operator {
      ast::LogicalOperator::And => it.left.evaluate_value_to_boolean(self.eval_ctx),
      ast::LogicalOperator::Or => it.left.evaluate_value_to_boolean(self.eval_ctx).map(|v| !v),
      ast::LogicalOperator::Coalesce => it.left.evaluate_value(self.eval_ctx).map(|value| {
        matches!(
          value,
          oxc_ecmascript::constant_evaluation::ConstantValue::Null
            | oxc_ecmascript::constant_evaluation::ConstantValue::Undefined
        )
      }),
    };
    if executes_rhs == Some(false) {
      self.changed = true;
      self.branches.insert(it.node_id(), false);
      self.visit_expression(&it.left);
      self.in_dead_branch = true;
      self.visit_expression(&it.right);
      self.in_dead_branch = false;
    } else {
      walk_js::walk_logical_expression(self, it);
    }
  }
}

fn has_hoisted_declarations(stmt: Option<&ast::Statement<'_>>) -> bool {
  struct Detector(bool);
  impl<'ast> VisitJs<'ast> for Detector {
    fn visit_variable_declaration(&mut self, it: &ast::VariableDeclaration<'ast>) {
      self.0 |= it.kind == ast::VariableDeclarationKind::Var;
    }
    fn visit_function(&mut self, it: &ast::Function<'ast>, _flags: oxc::semantic::ScopeFlags) {
      self.0 |= it.r#type == ast::FunctionType::FunctionDeclaration;
    }
    fn visit_arrow_function_expression(&mut self, _it: &ast::ArrowFunctionExpression<'ast>) {}
  }
  let mut detector = Detector(false);
  if let Some(stmt) = stmt {
    detector.visit_statement(stmt);
  }
  detector.0
}
