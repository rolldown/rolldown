use oxc::ast::ast;
use oxc::ast::VisitMut;

use super::IsolatingModuleFinalizer;

impl<'me, 'ast> VisitMut<'ast> for IsolatingModuleFinalizer<'me, 'ast> {
  fn visit_program(&mut self, _program: &mut ast::Program<'ast>) {
    // wrap the program within a function
  }
}
