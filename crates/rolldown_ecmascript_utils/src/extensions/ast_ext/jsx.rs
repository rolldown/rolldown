use oxc::allocator::Box;
use oxc::ast::ast::{IdentifierReference, JSXMemberExpressionObject};

pub trait JsxExt<'ast> {
  fn rewrite_ident_reference(&mut self, ident_ref: Box<'ast, IdentifierReference<'ast>>);
}

impl<'ast> JsxExt<'ast> for JSXMemberExpressionObject<'ast> {
  fn rewrite_ident_reference(&mut self, ident_ref: Box<'ast, IdentifierReference<'ast>>) {
    let mut object = self;
    loop {
      match object {
        JSXMemberExpressionObject::IdentifierReference(ident) => {
          *ident = ident_ref;
          break;
        }
        JSXMemberExpressionObject::MemberExpression(member_expr) => {
          object = &mut member_expr.object;
        }
        JSXMemberExpressionObject::ThisExpression(_) => break,
      }
    }
  }
}
