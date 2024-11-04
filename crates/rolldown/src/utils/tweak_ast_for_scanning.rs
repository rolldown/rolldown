use itertools::Itertools;
use oxc::allocator::{Allocator, Box};
use oxc::ast::ast::{self, BindingPatternKind, Declaration, ExpressionStatement, Statement};
use oxc::ast::visit::walk_mut;
use oxc::ast::{VisitMut, NONE};
use oxc::span::SPAN;
use rolldown_ecmascript::{AstSnippet, StatementExt, TakeIn};

/// Pre-process is a essential step to make rolldown generate correct and efficient code.
pub struct PreProcessor<'a, 'ast> {
  has_lazy_export: bool,
  snippet: AstSnippet<'a>,
  pub contains_use_strict: bool,
  none_hosted_stmts: Vec<Statement<'ast>>,
  need_push_ast: bool,
}

impl<'a, 'ast> PreProcessor<'a, 'ast> {
  pub fn new(alloc: &'a Allocator, has_lazy_export: bool) -> Self {
    Self {
      has_lazy_export,
      snippet: AstSnippet::new(alloc),
      contains_use_strict: false,
      none_hosted_stmts: vec![],
      need_push_ast: false,
    }
  }
}

impl<'ast, 'a: 'ast> VisitMut<'ast> for PreProcessor<'a, 'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    if self.has_lazy_export {
      program.body.extend(program.directives.take_in(self.snippet.alloc()).into_iter().map(|d| {
        let expr_stmt = ExpressionStatement {
          span: d.expression.span,
          expression: ast::Expression::StringLiteral(Box::new_in(
            d.expression,
            self.snippet.alloc(),
          )),
        };
        Statement::ExpressionStatement(Box::new_in(expr_stmt, self.snippet.alloc()))
      }));
    }

    program.directives.retain(|directive| {
      let is_use_strict = directive.is_use_strict();
      if is_use_strict {
        self.contains_use_strict = true;
        false
      } else {
        true
      }
    });
    let original_body = program.body.take_in(self.snippet.alloc());
    program.body.reserve_exact(original_body.len());
    self.none_hosted_stmts = Vec::with_capacity(
      original_body.iter().filter(|stmt| !stmt.is_module_declaration_with_source()).count(),
    );

    for mut stmt in original_body {
      self.need_push_ast = true;
      walk_mut::walk_statement(self, &mut stmt);
      if self.need_push_ast {
        if stmt.is_module_declaration_with_source() {
          program.body.push(stmt);
        } else {
          self.none_hosted_stmts.push(stmt);
        }
      }
    }
    program.body.extend(std::mem::take(&mut self.none_hosted_stmts));
  }

  fn visit_export_named_declaration(&mut self, named_decl: &mut ast::ExportNamedDeclaration<'ast>) {
    walk_mut::walk_export_named_declaration(self, named_decl);
    let named_decl_export_kind = named_decl.export_kind;
    let named_decl_span = named_decl.span;

    let Some(Declaration::VariableDeclaration(ref mut var_decl)) = named_decl.declaration else {
      return;
    };

    if var_decl
      .declarations
      .iter()
      // TODO: support nested destructuring tree shake, `export const {a, b} = obj; export const
      // [a, b] = arr;`
      .any(|declarator| matches!(declarator.id.kind, BindingPatternKind::BindingIdentifier(_)))
    {
      let rewritten = var_decl
        .declarations
        .take_in(self.snippet.alloc())
        .into_iter()
        .enumerate()
        .map(|(i, declarator)| {
          let is_first = i == 0;
          let new_decl = self.snippet.builder.alloc_variable_declaration(
            SPAN,
            var_decl.kind,
            self.snippet.builder.vec_from_iter([declarator]),
            var_decl.declare,
          );
          Statement::ExportNamedDeclaration(self.snippet.builder.alloc_export_named_declaration(
            if is_first { named_decl_span } else { SPAN },
            Some(Declaration::VariableDeclaration(new_decl)),
            self.snippet.builder.vec(),
            // Since it is `export a = 1, b = 2;`, source should be `None`
            None,
            named_decl_export_kind,
            NONE,
          ))
        })
        .collect_vec();
      self.none_hosted_stmts.extend(rewritten);
      self.need_push_ast = false;
    }
  }

  fn visit_expression(&mut self, it: &mut ast::Expression<'ast>) {
    let to_replaced = match it {
      // transpose `require(test ? 'a' : 'b')` into `test ? require('a') : require('b')`
      ast::Expression::CallExpression(expr)
        if expr.callee.is_specific_id("require") && expr.arguments.len() == 1 =>
      {
        let arg = expr.arguments.get_mut(0).unwrap();
        if let Some(cond_expr) = arg.as_expression_mut().and_then(|item| match item {
          ast::Expression::ConditionalExpression(cond) => Some(cond),
          _ => None,
        }) {
          let test = cond_expr.test.take_in(self.snippet.alloc());
          let consequent = cond_expr.consequent.take_in(self.snippet.alloc());
          let alternative = cond_expr.alternate.take_in(self.snippet.alloc());
          let new_cond_expr = self.snippet.builder.alloc_conditional_expression(
            SPAN,
            test,
            self.snippet.builder.expression_call(
              SPAN,
              self.snippet.builder.expression_identifier_reference(SPAN, "require"),
              NONE,
              self.snippet.builder.vec1(self.snippet.builder.argument_expression(consequent)),
              false,
            ),
            self.snippet.builder.expression_call(
              SPAN,
              self.snippet.builder.expression_identifier_reference(SPAN, "require"),
              NONE,
              self.snippet.builder.vec1(self.snippet.builder.argument_expression(alternative)),
              false,
            ),
          );

          Some(self.snippet.builder.expression_from_conditional(new_cond_expr))
        } else {
          None
        }
      }
      // transpose `import(test ? 'a' : 'b')` into `test ? import('a') : import('b')`
      ast::Expression::ImportExpression(expr) if expr.arguments.is_empty() => {
        let source = &mut expr.source;
        match source {
          ast::Expression::ConditionalExpression(cond_expr) => {
            let test = cond_expr.test.take_in(self.snippet.alloc());
            let consequent = cond_expr.consequent.take_in(self.snippet.alloc());
            let alternative = cond_expr.alternate.take_in(self.snippet.alloc());

            let new_cond_expr = self.snippet.builder.expression_conditional(
              SPAN,
              test,
              self.snippet.builder.expression_import(SPAN, consequent, self.snippet.builder.vec()),
              self.snippet.builder.expression_import(SPAN, alternative, self.snippet.builder.vec()),
            );

            Some(new_cond_expr)
          }
          _ => None,
        }
      }
      _ => None,
    };
    if let Some(replaced) = to_replaced {
      *it = replaced;
    }
    walk_mut::walk_expression(self, it);
  }
}
