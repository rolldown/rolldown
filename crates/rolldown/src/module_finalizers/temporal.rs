use oxc::{
  allocator::GetAllocator,
  ast::ast::{self, Expression},
  ast_visit::{VisitJsMut, walk_js_mut},
  span::SPAN,
};
use rolldown_common::SymbolRef;
use rolldown_ecmascript::ToSourceString;
use rolldown_ecmascript_utils::parse_injected_expression;

use super::ScopeHoistingFinalizer;

impl<'ast> ScopeHoistingFinalizer<'_, 'ast> {
  pub(super) fn imported_assignment_target(
    &self,
    binding: SymbolRef,
  ) -> Option<ast::SimpleAssignmentTarget<'ast>> {
    self.ctx.module.named_imports.get(&binding)?;
    let read = ast::ExpressionStatement::new(
      SPAN, self.finalized_expr_for_symbol_ref(binding, false, false).0, self,
    ).to_source_string();
    let read = read.trim().trim_end_matches(';');
    // A setter preserves RHS evaluation, compound reads, updates and destructuring order.
    // A local constant gives the intrinsic TypeError even if the source shadows TypeError.
    let expression = format!(
      "({{get value(){{return ({read})}},set value(_){{const immutable=0;immutable=_}}}}).value"
    );
    Some(ast::SimpleAssignmentTarget::from(
      parse_injected_expression(self.allocator(), &expression)
        .expect("generated immutable import assignment target")
        .into_member_expression(),
    ))
  }

  pub(super) fn preserve_class_inner_binding(
    &self,
    class: &mut ast::Class<'ast>,
    symbol: oxc::semantic::SymbolId,
  ) {
    struct ClassReferences<'a, 'ast> {
      scope: &'a rolldown_common::AstScopes,
      symbol: oxc::semantic::SymbolId,
      name: ast::Str<'ast>,
    }
    impl<'ast> VisitJsMut<'ast> for ClassReferences<'_, 'ast> {
      fn visit_identifier_reference(&mut self, reference: &mut ast::IdentifierReference<'ast>) {
        if reference
          .reference_id
          .get()
          .and_then(|id| self.scope.symbol_id_for(id))
          == Some(self.symbol)
        {
          reference.name = self.name.into();
          reference.reference_id.take();
        }
      }
    }
    // The named class expression has its own native lexical binding. Only direct body
    // references use it; superclass expressions and namespace getters still use the outer state.
    ClassReferences {
      scope: self.scope,
      symbol,
      name: ast::Str::from_str_in(self.canonical_name_for((self.ctx.idx, symbol).into()), self),
    }
    .visit_class_body(&mut class.body);
  }

  pub(super) fn guard_temporal_expression(
    &self,
    flag: SymbolRef,
    binding: SymbolRef,
    expression: Expression<'ast>,
  ) -> Expression<'ast> {
    let initialized = match self.native_temporal_owner(flag) {
      // The flag stays private to its record; the record registered it under its namespace.
      Some(namespace) => {
        let namespace = ast::ExpressionStatement::new(
          SPAN,
          self.finalized_expr_without_temporal_guard(namespace, false, false).0,
          self,
        )
        .to_source_string();
        let label = serde_json::to_string(binding.name(self.ctx.symbol_db)).expect("name JSON");
        parse_injected_expression(
          self.allocator(),
          &format!(
            "__rolldown_runtime__.nativeInitialized({},{label})",
            namespace.trim().trim_end_matches(';')
          ),
        )
        .expect("generated native temporal lookup")
      }
      None => self.finalized_expr_without_temporal_guard(flag, false, false).0,
    };
    let guard = Expression::new_call_expression(
      SPAN,
      self.finalized_expr_for_runtime_symbol("__checkTDZ"),
      None,
      [
        ast::Argument::from(initialized),
        ast::Argument::new_string_literal(
          SPAN,
          ast::Str::from_str_in(binding.name(self.ctx.symbol_db), self),
          None,
          self,
        ),
      ],
      false,
      self,
    );
    Expression::new_sequence_expression(
      SPAN,
      oxc::allocator::Vec::from_iter_in([guard, expression], self),
      self,
    )
  }

  /// The namespace of another module whose native record owns `flag`. The link stage
  /// referenced that namespace instead of the flag, so the flag never becomes an export.
  fn native_temporal_owner(&self, flag: SymbolRef) -> Option<SymbolRef> {
    if flag.owner == self.ctx.idx {
      return None;
    }
    let owner = self.ctx.modules[flag.owner].as_normal()?;
    crate::utils::native_records::has_native_namespace(
      self.ctx.options,
      owner,
      &self.ctx.linking_infos[flag.owner],
      self.ctx.symbol_db,
    )
    .then_some(owner.namespace_object_ref)
  }

  pub(super) fn initialize_temporal_flag(&self, flag: SymbolRef) -> Expression<'ast> {
    Expression::new_assignment_expression(
      SPAN,
      ast::AssignmentOperator::Assign,
      ast::AssignmentTarget::new_assignment_target_identifier(
        SPAN,
        ast::Str::from_str_in(self.canonical_name_for(flag), self),
        self,
      ),
      Expression::new_boolean_literal(SPAN, true, self),
      self,
    )
  }

  /// The target of an assignment to `binding`, if it is one of the module's guarded top-level
  /// bindings: an accessor that checks the binding before writing it, or one that initializes it.
  pub(super) fn temporal_assignment_target(
    &self,
    binding: SymbolRef,
    initialize: bool,
  ) -> Option<ast::SimpleAssignmentTarget<'ast>> {
    let (flag, constant) = self.temporal_bindings.get(&binding)?;
    let name = self.canonical_name_for(binding);
    let flag = self.canonical_name_for(*flag);
    let mut value = "__value".to_owned();
    while value == name || value == flag {
      value.push('_');
    }
    let label = serde_json::to_string(name).expect("identifier JSON");
    let expression = if initialize {
      format!("({{set value({value}){{{name}={value};{flag}=true}}}}).value")
    } else {
      let check = ast::ExpressionStatement::new(
        SPAN,
        self.finalized_expr_for_runtime_symbol("__checkTDZ"),
        self,
      )
      .to_source_string();
      let check = check.trim().trim_end_matches(';');
      format!(
        "({{get value(){{{check}({flag},{label});return {name}}},set value({value}){{{check}({flag},{label},{constant});{name}={value}}}}}).value"
      )
    };
    let expression = parse_injected_expression(self.allocator(), &expression)
      .expect("generated lexical assignment target");
    Some(ast::SimpleAssignmentTarget::from(
      expression.into_member_expression(),
    ))
  }

  pub(super) fn mark_destructured_initialization(&self, target: &mut ast::AssignmentTarget<'ast>) {
    struct Initializer<'a, 'me, 'ast> {
      finalizer: &'a ScopeHoistingFinalizer<'me, 'ast>,
    }
    impl<'ast> Initializer<'_, '_, 'ast> {
      fn initializer(&self, declared: &str) -> Option<ast::SimpleAssignmentTarget<'ast>> {
        let binding = *self.finalizer.temporal_declarations.get(declared)?;
        self.finalizer.temporal_assignment_target(binding, true)
      }
    }
    impl<'ast> VisitJsMut<'ast> for Initializer<'_, '_, 'ast> {
      fn visit_simple_assignment_target(&mut self, target: &mut ast::SimpleAssignmentTarget<'ast>) {
        if let ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(binding) = target
          && let Some(initializer) = self.initializer(binding.name.as_str())
        {
          *target = initializer;
          return;
        }
        walk_js_mut::walk_simple_assignment_target(self, target);
      }
      fn visit_assignment_target_property(
        &mut self,
        property: &mut ast::AssignmentTargetProperty<'ast>,
      ) {
        if let ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(binding) = property
          && let Some(target) = self.initializer(binding.binding.name.as_str())
        {
          let value = if let Some(init) = binding.init.take() {
            ast::AssignmentTargetMaybeDefault::new_assignment_target_with_default(
              SPAN,
              target.into(),
              init,
              self.finalizer,
            )
          } else {
            target.into()
          };
          *property = ast::AssignmentTargetProperty::new_assignment_target_property_property(
            SPAN,
            ast::PropertyKey::new_static_identifier(
              binding.span,
              binding.binding.name,
              self.finalizer,
            ),
            value,
            false,
            self.finalizer,
          );
          return;
        }
        walk_js_mut::walk_assignment_target_property(self, property);
      }
      // Only the declared bindings are initialized. The finalizer has already rewritten the
      // assignments in defaults and computed keys, and one that it left by name may be to a local
      // that shadows a declared binding.
      fn visit_expression(&mut self, _: &mut Expression<'ast>) {}
    }
    Initializer { finalizer: self }.visit_assignment_target(target);
  }
}
