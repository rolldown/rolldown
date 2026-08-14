pub trait FunctionExt {
  /// This trait only considers side effects from parameters and function body.
  /// Using very naive checks for side effects.
  fn is_side_effect_free(&self) -> bool;
}

impl FunctionExt for oxc::ast::ast::FormalParameters<'_> {
  fn is_side_effect_free(&self) -> bool {
    self.items.iter().all(|param| {
      // Check for default values with side effects
      // Type annotations are removed at compile time and cannot have side effects
      // No need to check for them

      // Check for destructuring patterns that might have side effects
      // Also check for initializers (default parameter values) which might have side effects
      match &param.pattern {
        // `function foo({ x } ) {}` probably has trigger side effects if x is a getter
        | oxc::ast::ast::BindingPattern::ObjectPattern(_)
        // `function foo([x]) {}` probably has trigger side effects if x is a getter
        | oxc::ast::ast::BindingPattern::ArrayPattern(_)
        | oxc::ast::ast::BindingPattern::AssignmentPattern(_) => {
          // `function foo(x = global()) {}`
          // Default parameter values might have side effects
          false
        }
        oxc::ast::ast::BindingPattern::BindingIdentifier(_) => {
          // Simple identifiers are safe, but check if there's an initializer
          param.initializer.is_none()
        }
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
