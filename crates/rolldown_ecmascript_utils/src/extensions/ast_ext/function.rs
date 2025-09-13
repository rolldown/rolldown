pub trait FunctionExt {
  /// This trait only considers side effects from parameters and function body.
  /// Using very naive checks for side effects.
  fn is_side_effect_free(&self) -> bool;
}

impl FunctionExt for oxc::ast::ast::FormalParameters<'_> {
  fn is_side_effect_free(&self) -> bool {
    self.items.iter().all(|param| {
      // Check for default values with side effects
      if param.pattern.type_annotation.is_some() {
        return false; // Type annotations might have side effects
      }

      // Check for destructuring patterns that might have side effects
      match &param.pattern.kind {
        // `function foo({ x } ) {}` probably has trigger side effects if x is a getter
        oxc::ast::ast::BindingPatternKind::ObjectPattern(_)
        // `function foo([x]) {}` probably has trigger side effects if x is a getter
        | oxc::ast::ast::BindingPatternKind::ArrayPattern(_) => false, // Destructuring might have side effects
        oxc::ast::ast::BindingPatternKind::AssignmentPattern(_) => {
          // `function foo(x = global()) {}`
          // Default parameter values might have side effects
          false
        }
        oxc::ast::ast::BindingPatternKind::BindingIdentifier(_) => true, // Simple identifiers are safe
      }
    })
  }
}

impl FunctionExt for oxc::ast::ast::Function<'_> {
  fn is_side_effect_free(&self) -> bool {
    // Check if body is empty
    let body_empty = match &self.body {
      Some(body) => body.statements.is_empty(),
      None => true,
    };

    body_empty && self.params.is_side_effect_free()
  }
}

impl FunctionExt for oxc::ast::ast::ArrowFunctionExpression<'_> {
  fn is_side_effect_free(&self) -> bool {
    // Check if body is empty
    let body_empty = self.body.is_empty();

    // Check if parameters have side effects
    body_empty && self.params.is_side_effect_free()
  }
}
