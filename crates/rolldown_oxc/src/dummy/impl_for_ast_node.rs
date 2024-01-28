use std::cell::Cell;

use oxc::{
  allocator::{Allocator, Box},
  ast::ast::{self, Modifiers},
  semantic::ReferenceFlag,
  span::{Atom, SourceType},
};

use crate::Dummy as DummyIn;

impl<'ast> DummyIn<'ast> for ast::VariableDeclarationKind {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::Var
  }
}
impl<'ast> DummyIn<'ast> for ast::ThisExpression {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: DummyIn::dummy(alloc) }
  }
}

impl<'ast> DummyIn<'ast> for ast::VariableDeclaration<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: DummyIn::dummy(alloc),
      kind: DummyIn::dummy(alloc),
      declarations: DummyIn::dummy(alloc),
      modifiers: Modifiers::default(),
    }
  }
}
impl<'ast> DummyIn<'ast> for ast::Declaration<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::VariableDeclaration(Box(alloc.alloc(DummyIn::dummy(alloc))))
  }
}
impl<'ast> DummyIn<'ast> for ast::ExpressionStatement<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: DummyIn::dummy(alloc), expression: DummyIn::dummy(alloc) }
  }
}
impl<'ast> DummyIn<'ast> for ast::FunctionType {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::FunctionDeclaration
  }
}

impl<'ast> DummyIn<'ast> for ast::FormalParameterKind {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::Signature
  }
}

impl<'ast> DummyIn<'ast> for ast::FormalParameters<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: DummyIn::dummy(alloc),
      kind: DummyIn::dummy(alloc),
      items: DummyIn::dummy(alloc),
      rest: DummyIn::dummy(alloc),
    }
  }
}
impl<'ast> DummyIn<'ast> for ast::ClassBody<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: DummyIn::dummy(alloc), body: DummyIn::dummy(alloc) }
  }
}
impl<'ast> DummyIn<'ast> for ast::ClassType {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::ClassDeclaration
  }
}
impl<'ast> DummyIn<'ast> for ast::Modifiers<'ast> {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Self::empty()
  }
}
impl<'ast> DummyIn<'ast> for ast::Class<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      r#type: DummyIn::dummy(alloc),
      span: DummyIn::dummy(alloc),
      id: DummyIn::dummy(alloc),
      super_class: DummyIn::dummy(alloc),
      body: DummyIn::dummy(alloc),
      type_parameters: DummyIn::dummy(alloc),
      super_type_parameters: DummyIn::dummy(alloc),
      implements: DummyIn::dummy(alloc),
      decorators: DummyIn::dummy(alloc),
      modifiers: DummyIn::dummy(alloc),
    }
  }
}
impl<'ast> DummyIn<'ast> for ast::Function<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      r#type: DummyIn::dummy(alloc),
      span: DummyIn::dummy(alloc),
      id: DummyIn::dummy(alloc),
      expression: DummyIn::dummy(alloc),
      generator: DummyIn::dummy(alloc),
      r#async: DummyIn::dummy(alloc),
      params: DummyIn::dummy(alloc),
      body: DummyIn::dummy(alloc),
      type_parameters: DummyIn::dummy(alloc),
      return_type: DummyIn::dummy(alloc),
      modifiers: Modifiers::default(),
      this_param: DummyIn::dummy(alloc),
    }
  }
}
impl<'ast> DummyIn<'ast> for ast::Expression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::ThisExpression(Box(alloc.alloc(DummyIn::dummy(alloc))))
  }
}

impl<'ast> DummyIn<'ast> for ast::IdentifierName {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: DummyIn::dummy(alloc), name: Atom::new_inline("") }
  }
}

impl<'ast> DummyIn<'ast> for ast::StaticMemberExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: DummyIn::dummy(alloc),
      object: DummyIn::dummy(alloc),
      property: DummyIn::dummy(alloc),
      optional: DummyIn::dummy(alloc),
    }
  }
}

impl<'ast> DummyIn<'ast> for ast::IdentifierReference {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: DummyIn::dummy(alloc),
      name: Atom::new_inline(""),
      reference_id: Cell::default(),
      reference_flag: ReferenceFlag::default(),
    }
  }
}

impl<'ast> DummyIn<'ast> for ast::Program<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: DummyIn::dummy(alloc),
      source_type: SourceType::default(),
      directives: DummyIn::dummy(alloc),
      hashbang: DummyIn::dummy(alloc),
      body: DummyIn::dummy(alloc),
    }
  }
}

impl<'ast> DummyIn<'ast> for ast::VariableDeclarator<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: DummyIn::dummy(alloc),
      kind: DummyIn::dummy(alloc),
      id: DummyIn::dummy(alloc),
      init: DummyIn::dummy(alloc),
      definite: DummyIn::dummy(alloc),
    }
  }
}

impl<'ast> DummyIn<'ast> for ast::BindingPattern<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      kind: DummyIn::dummy(alloc),
      type_annotation: DummyIn::dummy(alloc),
      optional: DummyIn::dummy(alloc),
    }
  }
}

impl<'ast> DummyIn<'ast> for ast::BindingPatternKind<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self::BindingIdentifier(DummyIn::dummy(alloc))
  }
}

impl<'ast> DummyIn<'ast> for ast::BindingIdentifier {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self { span: DummyIn::dummy(alloc), name: DummyIn::dummy(alloc), symbol_id: Cell::default() }
  }
}

impl<'ast> DummyIn<'ast> for Atom {
  fn dummy(_alloc: &'ast Allocator) -> Self {
    Atom::new_inline("")
  }
}

impl<'ast> DummyIn<'ast> for ast::CallExpression<'ast> {
  fn dummy(alloc: &'ast Allocator) -> Self {
    Self {
      span: DummyIn::dummy(alloc),
      callee: DummyIn::dummy(alloc),
      arguments: DummyIn::dummy(alloc),
      optional: DummyIn::dummy(alloc),
      type_parameters: DummyIn::dummy(alloc),
    }
  }
}
