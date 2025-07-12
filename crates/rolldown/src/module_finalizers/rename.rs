use oxc::ast::ast::{self, Expression, IdentifierReference};
use rolldown_common::SymbolRef;
use rolldown_ecmascript_utils::ExpressionExt;

use super::ScopeHoistingFinalizer;

impl<'ast> ScopeHoistingFinalizer<'_, 'ast> {
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
    let mut expr = self.finalized_expr_for_symbol_ref(symbol_ref, is_callee, false);

    // See https://github.com/oxc-project/oxc/issues/4606

    match &mut expr {
      ast::Expression::Identifier(it) => {
        it.span = id_ref.span;
      }
      ast::Expression::StaticMemberExpression(it) => {
        it.span = id_ref.span;
        it.property.span = id_ref.span;
        if let Some(object) = it.object.as_identifier_mut() {
          object.span = id_ref.span;
        }
      }
      _ => {}
    }

    Some(expr)
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
    &self,
    ident_ref: &ast::IdentifierReference<'ast>,
    is_callee: bool,
  ) -> Option<Expression<'ast>> {
    if self.ctx.module.dummy_record_set.contains(&ident_ref.span) {
      // use `__require` instead of `require`
      return Some(self.finalized_expr_for_runtime_symbol("__require"));
    }

    if let Some(new_expr) = self.generate_finalized_expr_for_reference(ident_ref, is_callee) {
      Some(new_expr)
    } else {
      // Nevertheless, this identifier is processed and don't need to be processed again.
      ident_ref.reference_id.take();
      None
    }
  }

  pub fn rewrite_simple_assignment_target(
    &self,
    target: &mut ast::SimpleAssignmentTarget<'ast>,
  ) -> Option<()> {
    // Some `IdentifierReference`s constructed by bundler don't have `ReferenceId` and we just ignore them.
    if let ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(target_id_ref) = target {
      let reference_id = target_id_ref.reference_id.get()?;
      let symbol_id = self.scope.symbol_id_for(reference_id)?;

      let symbol_ref = (self.ctx.id, symbol_id).into();
      let canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);
      let symbol = self.ctx.symbol_db.get(canonical_ref);

      if let Some(ns_alias) = &symbol.namespace_alias {
        *target = ast::SimpleAssignmentTarget::from(self.snippet.literal_prop_access_member_expr(
          self.canonical_name_for(ns_alias.namespace_ref),
          &ns_alias.property_name,
        ));
      } else {
        let canonical_name = self.canonical_name_for(canonical_ref);
        if target_id_ref.name != canonical_name.as_str() {
          target_id_ref.name = self.snippet.atom(canonical_name);
        }
        target_id_ref.reference_id.take();
      }
    }
    None
  }

  pub fn rewrite_object_pat_shorthand(&self, pat: &mut ast::ObjectPattern<'ast>) {
    for prop in &mut pat.properties {
      match &mut prop.value.kind {
        // Ensure `const { a } = ...;` will be rewritten to `const { a: a } = ...` instead of `const { a } = ...`
        // Ensure `function foo({ a }) {}` will be rewritten to `function foo({ a: a }) {}` instead of `function foo({ a }) {}`
        ast::BindingPatternKind::BindingIdentifier(ident) if prop.shorthand => {
          if let Some(symbol_id) = ident.symbol_id.get() {
            let canonical_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
            if ident.name != canonical_name.as_str() {
              ident.name = self.snippet.atom(canonical_name);
              prop.shorthand = false;
            }
            ident.symbol_id.get_mut().take();
          }
        }
        // Ensure `const { a = 1 } = ...;` will be rewritten to `const { a: a = 1 } = ...` instead of `const { a = 1 } = ...`
        // Ensure `function foo({ a = 1 }) {}` will be rewritten to `function foo({ a: a = 1 }) {}` instead of `function foo({ a = 1 }) {}`
        ast::BindingPatternKind::AssignmentPattern(assign_pat) if prop.shorthand => {
          let ast::BindingPatternKind::BindingIdentifier(ident) = &mut assign_pat.left.kind else {
            continue;
          };
          if let Some(symbol_id) = ident.symbol_id.get() {
            let canonical_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
            if ident.name != canonical_name.as_str() {
              ident.name = self.snippet.atom(canonical_name);
              prop.shorthand = false;
            }
            ident.symbol_id.get_mut().take();
          }
        }
        _ => {
          // For other patterns:
          // - `const [a] = ...` or `function foo([a]) {}`
          // - `const { a: b } = ...` or `function foo({ a: b }) {}`
          // - `const { a: b = 1 } = ...` or `function foo({ a: b = 1 }) {}`
          // They could keep correct semantics after renaming, so we don't need to do anything special.
        }
      }
    }
  }
}
