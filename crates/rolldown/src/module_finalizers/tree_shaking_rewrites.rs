use oxc::{
  allocator::{self, TakeIn},
  ast::ast::{self, Expression, Statement},
  semantic::NodeId,
  span::SPAN,
};

use crate::types::tree_shaking_rewrites::{FoldedExpr, TreeShakingRewrites};

use super::ScopeHoistingFinalizer;

/// Apply the edits of advanced tree shaking. Each edit happens before the finalizer visits the
/// children of the node, so the finalizer never visits a removed subtree. Synthetic nodes have
/// `NodeId::DUMMY` and never get an edit.
///
/// See internal-docs/advanced-tree-shaking/implementation.md.
impl<'ast> ScopeHoistingFinalizer<'_, 'ast> {
  pub(super) fn remove_tree_shaken_statements(
    &self,
    stmts: &mut allocator::Vec<'ast, Statement<'ast>>,
    rewrites: &TreeShakingRewrites,
  ) {
    if rewrites.removed_stmts.is_empty() && rewrites.folded_ifs.is_empty() {
      return;
    }
    stmts.retain_mut(|stmt| {
      let node_id = stmt.node_id();
      if node_id != NodeId::DUMMY && rewrites.removed_stmts.contains(&node_id) {
        return false;
      }
      !self.fold_if_statement(stmt, rewrites)
    });
  }

  /// Replace a folded `if` statement with its live branch. Returns `true` when nothing is left,
  /// and then `stmt` is an empty statement.
  pub(super) fn fold_if_statement(
    &self,
    stmt: &mut Statement<'ast>,
    rewrites: &TreeShakingRewrites,
  ) -> bool {
    loop {
      let Statement::IfStatement(if_stmt) = stmt else {
        return false;
      };
      let node_id = if_stmt.node_id();
      if node_id == NodeId::DUMMY {
        return false;
      }
      let Some(folded) = rewrites.folded_ifs.get(&node_id).copied() else {
        return false;
      };
      if folded.keep_test {
        if folded.keep_consequent {
          if_stmt.alternate = None;
        } else {
          if_stmt.consequent = Statement::new_empty_statement(SPAN, self);
        }
        return false;
      }
      let live = if folded.keep_consequent {
        Some(if_stmt.consequent.take_in(self))
      } else {
        if_stmt.alternate.take()
      };
      match live {
        Some(live) => *stmt = live,
        None => {
          *stmt = Statement::new_empty_statement(SPAN, self);
          return true;
        }
      }
    }
  }

  pub(super) fn apply_tree_shaking_expr_rewrites(
    &self,
    expr: &mut Expression<'ast>,
    rewrites: &TreeShakingRewrites,
  ) {
    loop {
      let node_id = expr.node_id();
      if node_id == NodeId::DUMMY {
        break;
      }
      let Some(folded) = rewrites.folded_exprs.get(&node_id).copied() else {
        break;
      };
      match (folded, &mut *expr) {
        (FoldedExpr::Conditional(keep_consequent), Expression::ConditionalExpression(cond)) => {
          *expr = if keep_consequent {
            cond.consequent.take_in(self)
          } else {
            cond.alternate.take_in(self)
          };
        }
        (FoldedExpr::Logical(keep_left), Expression::LogicalExpression(logical)) => {
          *expr = if keep_left { logical.left.take_in(self) } else { logical.right.take_in(self) };
        }
        (FoldedExpr::Bool(value), _) => {
          *expr = Expression::new_boolean_literal(SPAN, value, self);
          break;
        }
        _ => break,
      }
    }
    match expr {
      Expression::ObjectExpression(obj) if !rewrites.removed_props.is_empty() => {
        obj.properties.retain(|prop| !is_removed(&rewrites.removed_props, prop_node_id(prop)));
      }
      Expression::CallExpression(call) if !rewrites.removed_args.is_empty() => {
        self.remove_call_arguments(&mut call.arguments, rewrites);
      }
      _ => {}
    }
  }

  fn remove_call_arguments(
    &self,
    arguments: &mut allocator::Vec<'ast, ast::Argument<'ast>>,
    rewrites: &TreeShakingRewrites,
  ) {
    while arguments.last().is_some_and(|arg| is_removed(&rewrites.removed_args, arg.node_id())) {
      arguments.pop();
    }
    for arg in arguments.iter_mut() {
      if is_removed(&rewrites.removed_args, arg.node_id()) {
        *arg = ast::Argument::from(Expression::new_void_0(SPAN, self));
      }
    }
  }
}

fn prop_node_id(prop: &ast::ObjectPropertyKind<'_>) -> NodeId {
  match prop {
    ast::ObjectPropertyKind::ObjectProperty(prop) => prop.node_id(),
    ast::ObjectPropertyKind::SpreadProperty(spread) => spread.node_id(),
  }
}

fn is_removed(set: &rustc_hash::FxHashSet<NodeId>, node_id: NodeId) -> bool {
  node_id != NodeId::DUMMY && set.contains(&node_id)
}
