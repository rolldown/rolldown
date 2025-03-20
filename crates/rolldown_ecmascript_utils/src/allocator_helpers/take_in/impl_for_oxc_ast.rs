use std::cell::Cell;

use oxc::{
  allocator::{Allocator, Box},
  ast::ast,
  span::{Atom, SPAN, SourceType},
};

use super::TakeIn;

impl<'ast> TakeIn<'ast> for ast::VariableDeclarationKind {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::Var
  }
}
impl<'ast> TakeIn<'ast> for ast::ThisExpression {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc) }
  }
}

impl<'ast> TakeIn<'ast> for ast::VariableDeclaration<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      kind: TakeIn::dummy(alloc),
      declarations: TakeIn::dummy(alloc),
      declare: TakeIn::dummy(alloc),
    }
  }
}
impl<'ast> TakeIn<'ast> for ast::Declaration<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::VariableDeclaration(Box::new_in(TakeIn::dummy(alloc), alloc))
  }
}
impl<'ast> TakeIn<'ast> for ast::ExpressionStatement<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), expression: TakeIn::dummy(alloc) }
  }
}
impl<'ast> TakeIn<'ast> for ast::FunctionType {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::FunctionDeclaration
  }
}

impl<'ast> TakeIn<'ast> for ast::FormalParameterKind {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::Signature
  }
}

impl<'ast> TakeIn<'ast> for ast::FormalParameters<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      kind: TakeIn::dummy(alloc),
      items: TakeIn::dummy(alloc),
      rest: TakeIn::dummy(alloc),
    }
  }
}
impl<'ast> TakeIn<'ast> for ast::ClassBody<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), body: TakeIn::dummy(alloc) }
  }
}
impl<'ast> TakeIn<'ast> for ast::ClassType {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::ClassDeclaration
  }
}
impl<'ast> TakeIn<'ast> for ast::Class<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      r#type: TakeIn::dummy(alloc),
      span: TakeIn::dummy(alloc),
      id: TakeIn::dummy(alloc),
      super_class: TakeIn::dummy(alloc),
      body: TakeIn::dummy(alloc),
      type_parameters: TakeIn::dummy(alloc),
      super_type_arguments: TakeIn::dummy(alloc),
      implements: TakeIn::dummy(alloc),
      decorators: TakeIn::dummy(alloc),
      r#abstract: TakeIn::dummy(alloc),
      declare: TakeIn::dummy(alloc),
      scope_id: Cell::default(),
    }
  }
}
impl<'ast> TakeIn<'ast> for ast::Function<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      r#type: TakeIn::dummy(alloc),
      span: TakeIn::dummy(alloc),
      id: TakeIn::dummy(alloc),
      generator: TakeIn::dummy(alloc),
      r#async: TakeIn::dummy(alloc),
      declare: TakeIn::dummy(alloc),
      params: TakeIn::dummy(alloc),
      body: TakeIn::dummy(alloc),
      type_parameters: TakeIn::dummy(alloc),
      return_type: TakeIn::dummy(alloc),
      this_param: TakeIn::dummy(alloc),
      scope_id: Cell::default(),
      pure: TakeIn::dummy(alloc),
    }
  }
}
impl<'ast> TakeIn<'ast> for ast::Expression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::ThisExpression(Box::new_in(TakeIn::dummy(alloc), alloc))
  }
}

impl<'ast> TakeIn<'ast> for ast::IdentifierName<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), name: TakeIn::dummy(alloc) }
  }
}

impl<'ast> TakeIn<'ast> for ast::StaticMemberExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      object: TakeIn::dummy(alloc),
      property: TakeIn::dummy(alloc),
      optional: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::IdentifierReference<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), name: TakeIn::dummy(alloc), reference_id: Cell::default() }
  }
}

impl<'ast> TakeIn<'ast> for ast::Program<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      source_type: SourceType::default(),
      source_text: "",
      comments: TakeIn::dummy(alloc),
      directives: TakeIn::dummy(alloc),
      hashbang: TakeIn::dummy(alloc),
      body: TakeIn::dummy(alloc),
      scope_id: Cell::default(),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::VariableDeclarator<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      kind: TakeIn::dummy(alloc),
      id: TakeIn::dummy(alloc),
      init: TakeIn::dummy(alloc),
      definite: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::BindingPattern<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      kind: TakeIn::dummy(alloc),
      type_annotation: TakeIn::dummy(alloc),
      optional: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::BindingPatternKind<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::BindingIdentifier(TakeIn::dummy(alloc))
  }
}

impl<'ast> TakeIn<'ast> for ast::BindingIdentifier<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), name: TakeIn::dummy(alloc), symbol_id: Cell::default() }
  }
}

impl<'ast> TakeIn<'ast> for Atom<'ast> {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Atom::new_const("")
  }
}

impl<'ast> TakeIn<'ast> for ast::CallExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      callee: TakeIn::dummy(alloc),
      arguments: TakeIn::dummy(alloc),
      optional: TakeIn::dummy(alloc),
      type_arguments: TakeIn::dummy(alloc),
      pure: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::ArrowFunctionExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      expression: TakeIn::dummy(alloc),
      r#async: TakeIn::dummy(alloc),
      params: TakeIn::dummy(alloc),
      body: TakeIn::dummy(alloc),
      type_parameters: TakeIn::dummy(alloc),
      return_type: TakeIn::dummy(alloc),
      scope_id: Cell::default(),
      pure: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::FunctionBody<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      directives: TakeIn::dummy(alloc),
      statements: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::FormalParameter<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      pattern: TakeIn::dummy(alloc),
      accessibility: TakeIn::dummy(alloc),
      readonly: TakeIn::dummy(alloc),
      decorators: TakeIn::dummy(alloc),
      r#override: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::SequenceExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), expressions: TakeIn::dummy(alloc) }
  }
}

impl<'ast> TakeIn<'ast> for ast::ParenthesizedExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), expression: TakeIn::dummy(alloc) }
  }
}

impl<'ast> TakeIn<'ast> for ast::AssignmentExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      operator: TakeIn::dummy(alloc),
      left: TakeIn::dummy(alloc),
      right: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::AssignmentTarget<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::AssignmentTargetIdentifier(TakeIn::dummy(alloc))
  }
}

impl<'ast> TakeIn<'ast> for ast::SimpleAssignmentTarget<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::AssignmentTargetIdentifier(TakeIn::dummy(alloc))
  }
}

impl<'ast> TakeIn<'ast> for ast::ArrayAssignmentTarget<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      elements: TakeIn::dummy(alloc),
      rest: TakeIn::dummy(alloc),
      trailing_comma: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::ObjectAssignmentTarget<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      properties: TakeIn::dummy(alloc),
      rest: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::AssignmentTargetPropertyIdentifier<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { init: TakeIn::dummy(alloc), span: TakeIn::dummy(alloc), binding: TakeIn::dummy(alloc) }
  }
}

impl<'ast> TakeIn<'ast> for ast::AssignmentTargetMaybeDefault<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::AssignmentTargetIdentifier(TakeIn::dummy(alloc))
  }
}

impl<'ast> TakeIn<'ast> for ast::AssignmentTargetPropertyProperty<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      name: TakeIn::dummy(alloc),
      span: TakeIn::dummy(alloc),
      binding: TakeIn::dummy(alloc),
      computed: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::AssignmentTargetWithDefault<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), binding: TakeIn::dummy(alloc), init: TakeIn::dummy(alloc) }
  }
}

impl<'ast> TakeIn<'ast> for ast::ObjectExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      properties: TakeIn::dummy(alloc),
      trailing_comma: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::ObjectProperty<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      kind: TakeIn::dummy(alloc),
      key: TakeIn::dummy(alloc),
      value: TakeIn::dummy(alloc),
      method: TakeIn::dummy(alloc),
      shorthand: TakeIn::dummy(alloc),
      computed: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::ObjectPropertyKind<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::ObjectProperty(TakeIn::dummy(alloc))
  }
}

impl<'ast> TakeIn<'ast> for ast::PropertyKind {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::Init
  }
}

impl<'ast> TakeIn<'ast> for ast::PropertyKey<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::Identifier(TakeIn::dummy(alloc))
  }
}

impl<'ast> TakeIn<'ast> for ast::UnaryExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      operator: TakeIn::dummy(alloc),
      argument: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::StringLiteral<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), value: TakeIn::dummy(alloc), raw: None, lossy: false }
  }
}
impl<'ast> TakeIn<'ast> for ast::ImportOrExportKind {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::Value
  }
}

impl<'ast> TakeIn<'ast> for ast::ImportDeclaration<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      specifiers: TakeIn::dummy(alloc),
      source: TakeIn::dummy(alloc),
      with_clause: TakeIn::dummy(alloc),
      import_kind: TakeIn::dummy(alloc),
      phase: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::ObjectPattern<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: TakeIn::dummy(alloc),
      properties: TakeIn::dummy(alloc),
      rest: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::BindingProperty<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: SPAN,
      key: TakeIn::dummy(alloc),
      value: TakeIn::dummy(alloc),
      shorthand: TakeIn::dummy(alloc),
      computed: TakeIn::dummy(alloc),
    }
  }
}

impl<'ast> TakeIn<'ast> for ast::ReturnStatement<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: TakeIn::dummy(alloc), argument: TakeIn::dummy(alloc) }
  }
}
