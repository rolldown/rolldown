use oxc::{
  allocator::{Allocator, Box},
  ast::ast::{self, Modifiers},
};

use crate::DummyIn;

impl<'ast> DummyIn<'ast> for ast::VariableDeclarationKind {
  fn dummy_in(_alloc: &'ast Allocator) -> Self {
    Self::Var
  }
}
impl<'ast> DummyIn<'ast> for ast::ThisExpression {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Self { span: DummyIn::dummy_in(alloc) }
  }
}

impl<'ast> DummyIn<'ast> for ast::VariableDeclaration<'ast> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Self {
      span: DummyIn::dummy_in(alloc),
      kind: DummyIn::dummy_in(alloc),
      declarations: DummyIn::dummy_in(alloc),
      modifiers: Modifiers::default(),
    }
  }
}
impl<'ast> DummyIn<'ast> for ast::Declaration<'ast> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Self::VariableDeclaration(Box(alloc.alloc(DummyIn::dummy_in(alloc))))
  }
}
impl<'ast> DummyIn<'ast> for ast::ExpressionStatement<'ast> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Self { span: DummyIn::dummy_in(alloc), expression: DummyIn::dummy_in(alloc) }
  }
}
impl<'ast> DummyIn<'ast> for ast::FunctionType {
  fn dummy_in(_alloc: &'ast Allocator) -> Self {
    Self::FunctionDeclaration
  }
}

impl<'ast> DummyIn<'ast> for ast::FormalParameterKind {
  fn dummy_in(_alloc: &'ast Allocator) -> Self {
    Self::Signature
  }
}

impl<'ast> DummyIn<'ast> for ast::FormalParameters<'ast> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Self {
      span: DummyIn::dummy_in(alloc),
      kind: DummyIn::dummy_in(alloc),
      items: DummyIn::dummy_in(alloc),
      rest: DummyIn::dummy_in(alloc),
    }
  }
}
impl<'ast> DummyIn<'ast> for ast::ClassBody<'ast> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Self { span: DummyIn::dummy_in(alloc), body: DummyIn::dummy_in(alloc) }
  }
}
impl<'ast> DummyIn<'ast> for ast::ClassType {
  fn dummy_in(_alloc: &'ast Allocator) -> Self {
    Self::ClassDeclaration
  }
}
impl<'ast> DummyIn<'ast> for ast::Modifiers<'ast> {
  fn dummy_in(_alloc: &'ast Allocator) -> Self {
    Self::empty()
  }
}
impl<'ast> DummyIn<'ast> for ast::Class<'ast> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Self {
      r#type: DummyIn::dummy_in(alloc),
      span: DummyIn::dummy_in(alloc),
      id: DummyIn::dummy_in(alloc),
      super_class: DummyIn::dummy_in(alloc),
      body: DummyIn::dummy_in(alloc),
      type_parameters: DummyIn::dummy_in(alloc),
      super_type_parameters: DummyIn::dummy_in(alloc),
      implements: DummyIn::dummy_in(alloc),
      decorators: DummyIn::dummy_in(alloc),
      modifiers: DummyIn::dummy_in(alloc),
    }
  }
}
impl<'ast> DummyIn<'ast> for ast::Function<'ast> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Self {
      r#type: DummyIn::dummy_in(alloc),
      span: DummyIn::dummy_in(alloc),
      id: DummyIn::dummy_in(alloc),
      expression: DummyIn::dummy_in(alloc),
      generator: DummyIn::dummy_in(alloc),
      r#async: DummyIn::dummy_in(alloc),
      params: DummyIn::dummy_in(alloc),
      body: DummyIn::dummy_in(alloc),
      type_parameters: DummyIn::dummy_in(alloc),
      return_type: DummyIn::dummy_in(alloc),
      modifiers: Modifiers::default(),
    }
  }
}
impl<'ast> DummyIn<'ast> for ast::Expression<'ast> {
  fn dummy_in(alloc: &'ast Allocator) -> Self {
    Self::ThisExpression(Box(alloc.alloc(DummyIn::dummy_in(alloc))))
  }
}
