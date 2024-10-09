use oxc::ast::ast::{self, IdentifierReference};
use rolldown_common::SymbolRef;
use rolldown_ecmascript::ExpressionExt;

use super::ScopeHoistingFinalizer;

impl<'me, 'ast> ScopeHoistingFinalizer<'me, 'ast> {
  /// return `None` if
  /// - the reference is for a global variable/the reference doesn't have a `SymbolId`
  /// - the reference doesn't have a `ReferenceId`
  /// - the canonical name is the same as the original name
  pub fn generate_finalized_expr_for_reference(
    &self,
    id_ref: &IdentifierReference<'ast>,
    is_callee: bool,
  ) -> Option<ast::Expression<'ast>> {
    // Some `IdentifierReference`s constructed by bundler don't have `ReferenceId` and we just ignore them.
    let reference_id = id_ref.reference_id.get()?;

    // we will hit this branch if the reference points to a global variable
    let symbol_id = self.scope.symbol_id_for(reference_id)?;

    let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();
    let mut expr = self.finalized_expr_for_symbol_ref(symbol_ref, is_callee);

    // See https://github.com/oxc-project/oxc/issues/4606

    match &mut expr {
      ast::Expression::Identifier(it) => {
        it.span = id_ref.span;
      }
      ast::Expression::StaticMemberExpression(it) => {
        it.span = id_ref.span;
      }
      _ => {}
    }

    Some(expr)

    // let canonical_name = self.canonical_name_for(canonical_ref);
    // if id_ref.name != canonical_name.as_str() {
    //   return Some(self.snippet.id_ref_expr(canonical_name, id_ref.span));
    // }
    // None
  }

  /// return `None` if
  /// - the reference is for a global variable/the reference doesn't have a `SymbolId`
  /// - the reference doesn't have a `ReferenceId`
  /// - the canonical name is the same as the original name
  pub fn generate_finalized_simple_assignment_target_for_reference(
    &self,
    id_ref: &IdentifierReference,
  ) -> Option<ast::SimpleAssignmentTarget<'ast>> {
    // Some `IdentifierReference`s constructed by bundler don't have `ReferenceId` and we just ignore them.
    let reference_id = id_ref.reference_id.get()?;

    // we will hit this branch if the reference points to a global variable
    let symbol_id = self.scope.symbol_id_for(reference_id)?;

    let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();
    let canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);
    let symbol = self.ctx.symbol_db.get(canonical_ref);

    if let Some(ns_alias) = &symbol.namespace_alias {
      let canonical_ns_name = self.canonical_name_for(ns_alias.namespace_ref);
      let prop_name = &ns_alias.property_name;
      let access_expr = self.snippet.literal_prop_access_member_expr(canonical_ns_name, prop_name);

      return Some(ast::SimpleAssignmentTarget::from(access_expr));
    }

    let canonical_name = self.canonical_name_for(canonical_ref);
    if id_ref.name != canonical_name.as_str() {
      return Some(ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(
        self.snippet.alloc_id_ref(canonical_name, id_ref.span),
      ));
    }

    None
  }

  pub fn try_rewrite_identifier_reference_expr(
    &mut self,
    expr: &mut ast::Expression<'ast>,
    is_callee: bool,
  ) {
    if let Some(id_ref) = expr.as_identifier_mut() {
      if let Some(new_expr) = self.generate_finalized_expr_for_reference(id_ref, is_callee) {
        *expr = new_expr;
      } else {
        // Nevertheless, this identifier is processed and don't need to be processed again.
        *id_ref.reference_id.get_mut() = None;
      }
    }
  }

  pub fn rewrite_simple_assignment_target(
    &mut self,
    simple_target: &mut ast::SimpleAssignmentTarget<'ast>,
  ) {
    // Some `IdentifierReference`s constructed by bundler don't have `ReferenceId` and we just ignore them.
    let ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(target_id_ref) = simple_target
    else {
      return;
    };

    let Some(reference_id) = target_id_ref.reference_id.get() else {
      return;
    };

    let Some(symbol_id) = self.scope.symbol_id_for(reference_id) else {
      return;
    };

    let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();
    let canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);
    let symbol = self.ctx.symbol_db.get(canonical_ref);

    if let Some(ns_alias) = &symbol.namespace_alias {
      let canonical_ns_name = self.canonical_name_for(ns_alias.namespace_ref);
      let prop_name = &ns_alias.property_name;
      let access_expr = self.snippet.literal_prop_access_member_expr(canonical_ns_name, prop_name);
      *simple_target = ast::SimpleAssignmentTarget::from(access_expr);
    } else {
      let canonical_name = self.canonical_name_for(canonical_ref);
      if target_id_ref.name != canonical_name.as_str() {
        target_id_ref.name = self.snippet.atom(canonical_name);
      }
      *target_id_ref.reference_id.get_mut() = None;
    }
  }
}
