use oxc::semantic::NodeId;
use rustc_hash::{FxHashMap, FxHashSet};

/// Sub-statement edits that advanced tree shaking decided in the link stage. The module finalizer
/// applies them before it visits the children of a node, so it never visits a removed subtree.
/// All keys are AST node ids, which stay the same between the link stage and the finalizer.
///
/// See internal-docs/advanced-tree-shaking/implementation.md.
#[derive(Debug, Default)]
pub struct TreeShakingRewrites {
  /// Statements in a statement list (never top-level ones) to remove.
  pub removed_stmts: FxHashSet<NodeId>,
  /// Object literal properties to remove.
  pub removed_props: FxHashSet<NodeId>,
  /// Call arguments to remove. A trailing argument is dropped, others become `void 0`.
  pub removed_args: FxHashSet<NodeId>,
  /// `if` statements whose test value is known.
  pub folded_ifs: FxHashMap<NodeId, FoldedIf>,
  /// Expressions to replace.
  pub folded_exprs: FxHashMap<NodeId, FoldedExpr>,
}

impl TreeShakingRewrites {
  pub fn is_empty(&self) -> bool {
    self.removed_stmts.is_empty()
      && self.removed_props.is_empty()
      && self.removed_args.is_empty()
      && self.folded_ifs.is_empty()
      && self.folded_exprs.is_empty()
  }

  pub fn merge(&mut self, other: Self) {
    self.removed_stmts.extend(other.removed_stmts);
    self.removed_props.extend(other.removed_props);
    self.removed_args.extend(other.removed_args);
    self.folded_ifs.extend(other.folded_ifs);
    self.folded_exprs.extend(other.folded_exprs);
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldedIf {
  /// `true` keeps the consequent, `false` keeps the alternate.
  pub keep_consequent: bool,
  /// The test has effects, so the `if` statement stays and only the dead branch goes.
  pub keep_test: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldedExpr {
  /// Replace a conditional expression with its consequent (`true`) or its alternate (`false`).
  Conditional(bool),
  /// Replace a logical expression with its left operand (`true`) or its right operand (`false`).
  Logical(bool),
  /// Replace the expression with a boolean literal.
  Bool(bool),
}
