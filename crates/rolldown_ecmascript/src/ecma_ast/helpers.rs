use oxc::{
  ast::ast::Program,
  semantic::{Scoping, Semantic, SemanticBuilder, Stats},
};

use crate::EcmaAst;

/// Returns a [`SemanticBuilder`] pre-configured for any `Scoping` that will be
/// passed to `Transformer::build_with_scoping`.
///
/// The TS transformer's enum lowering reads each member's constant value via
/// `Scoping::get_enum_member_value`. That table is only populated when the
/// builder has `enum_eval` enabled — without it, member aliases like
/// `Default = Theme.Light` fall through to the buggy reverse-mapping form for
/// string enums (`Foo[Foo["x"] = init] = "x"`), which corrupts the original
/// member's reverse mapping. Callers may chain additional options (e.g.
/// `with_check_syntax_error`, `with_stats`) on top.
pub fn semantic_builder_for_transform<'a>() -> SemanticBuilder<'a> {
  SemanticBuilder::new().with_enum_eval(true)
}

impl EcmaAst {
  pub fn is_body_empty(&self) -> bool {
    self.program().is_empty()
  }

  pub fn make_semantic<'ast>(program: &'ast Program<'ast>, with_cfg: bool) -> Semantic<'ast> {
    SemanticBuilder::new().with_cfg(with_cfg).build(program).semantic
  }

  pub fn make_scoping(&self) -> Scoping {
    self.make_scoping_with_stats().0
  }

  /// Build `Scoping` along with the `Stats` captured from the same `Semantic`,
  /// so consumers that may later rebuild scoping for the same AST can re-use
  /// the stats via `SemanticBuilder::with_stats` and skip `Stats::count`.
  pub fn make_scoping_with_stats(&self) -> (Scoping, Stats) {
    self.program.with_dependent(|_owner, dep| {
      let semantic = Self::make_semantic(&dep.program, /*with_cfg*/ false);
      let stats = semantic.stats();
      (semantic.into_scoping(), stats)
    })
  }

  /// Build `Scoping` re-using cached `Stats` from a previous build to skip
  /// the `Stats::count` AST traversal that pre-allocates buffers.
  pub fn make_scoping_using_stats(program: &Program<'_>, stats: Stats) -> Scoping {
    SemanticBuilder::new().with_stats(stats).build(program).semantic.into_scoping()
  }

  pub fn make_symbol_table_and_scope_tree_with_semantic_builder<'a>(
    &'a self,
    semantic_builder: SemanticBuilder<'a>,
  ) -> Scoping {
    self.program.with_dependent::<'a, Scoping>(|_owner, dep| {
      semantic_builder.build(&dep.program).semantic.into_scoping()
    })
  }
}
