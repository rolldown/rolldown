use itertools::Itertools;
use oxc::allocator::Allocator;
use oxc::ast::ast::{self, BindingPatternKind, Declaration, ImportOrExportKind, Statement};
use oxc::ast::visit::walk_mut;
use oxc::ast::{VisitMut, NONE};
use oxc::span::{Span, SPAN};
use rolldown_ecmascript_utils::{AstSnippet, StatementExt, TakeIn};

/// Pre-process is a essential step to make rolldown generate correct and efficient code.
pub struct PreProcessor<'ast> {
  snippet: AstSnippet<'ast>,
  pub contains_use_strict: bool,
  /// For top level statements, this is used to store none_hoisted statements.
  /// For none top level statements, this is used to store split `VarDeclaration`.
  stmt_temp_storage: Vec<Statement<'ast>>,
  need_push_ast: bool,
  keep_names: bool,
}

impl<'ast> PreProcessor<'ast> {
  pub fn new(alloc: &'ast Allocator, keep_names: bool) -> Self {
    Self {
      snippet: AstSnippet::new(alloc),
      contains_use_strict: false,
      stmt_temp_storage: vec![],
      need_push_ast: false,
      keep_names,
    }
  }

  /// split `var a = 1, b = 2;` into `var a = 1; var b = 2;`
  fn split_var_declaration(
    &self,
    var_decl: &mut ast::VariableDeclaration<'ast>,
    named_decl_span: Option<Span>,
  ) -> Vec<Statement<'ast>> {
    var_decl
      .declarations
      .take_in(self.snippet.alloc())
      .into_iter()
      .enumerate()
      .map(|(i, declarator)| {
        let new_decl = self.snippet.builder.alloc_variable_declaration(
          SPAN,
          var_decl.kind,
          self.snippet.builder.vec_from_iter([declarator]),
          var_decl.declare,
        );
        if let Some(named_decl_span) = named_decl_span {
          Statement::ExportNamedDeclaration(self.snippet.builder.alloc_export_named_declaration(
            if i == 0 { named_decl_span } else { SPAN },
            Some(Declaration::VariableDeclaration(new_decl)),
            self.snippet.builder.vec(),
            // Since it is `export a = 1, b = 2;`, source should be `None`
            None,
            ImportOrExportKind::Value,
            NONE,
          ))
        } else {
          Statement::VariableDeclaration(new_decl)
        }
      })
      .collect_vec()
  }
}

impl<'ast> VisitMut<'ast> for PreProcessor<'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
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
    self.stmt_temp_storage = Vec::with_capacity(
      original_body.iter().filter(|stmt| !stmt.is_module_declaration_with_source()).count(),
    );

    for mut stmt in original_body {
      self.need_push_ast = true;
      walk_mut::walk_statement(self, &mut stmt);
      if self.need_push_ast {
        if stmt.is_module_declaration_with_source() {
          program.body.push(stmt);
        } else {
          self.stmt_temp_storage.push(stmt);
        }
      }
    }
    program.body.extend(std::mem::take(&mut self.stmt_temp_storage));
  }

  fn visit_statements(&mut self, it: &mut oxc::allocator::Vec<'ast, Statement<'ast>>) {
    if self.keep_names {
      let stmts = it.take_in(self.snippet.alloc());
      for mut stmt in stmts {
        walk_mut::walk_statement(self, &mut stmt);
        if self.stmt_temp_storage.is_empty() {
          it.push(stmt);
        } else {
          it.extend(self.stmt_temp_storage.drain(..));
        }
      }
    } else {
      walk_mut::walk_statements(self, it);
    }
  }

  fn visit_declaration(&mut self, it: &mut Declaration<'ast>) {
    match it {
      Declaration::VariableDeclaration(decl) => {
        if decl.declarations.len() > 1 && self.keep_names {
          self.stmt_temp_storage.extend(self.split_var_declaration(decl, None));
          self.need_push_ast = false;
        }
      }
      Declaration::FunctionDeclaration(_) | Declaration::ClassDeclaration(_) => {}
      Declaration::TSTypeAliasDeclaration(_)
      | Declaration::TSInterfaceDeclaration(_)
      | Declaration::TSEnumDeclaration(_)
      | Declaration::TSModuleDeclaration(_)
      | Declaration::TSImportEqualsDeclaration(_) => unreachable!(),
    }
    walk_mut::walk_declaration(self, it);
  }

  fn visit_export_named_declaration(&mut self, named_decl: &mut ast::ExportNamedDeclaration<'ast>) {
    walk_mut::walk_export_named_declaration(self, named_decl);
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
      let rewritten = self.split_var_declaration(var_decl, Some(named_decl_span));
      self.stmt_temp_storage.extend(rewritten);
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
              self.snippet.builder.vec1(ast::Argument::from(consequent)),
              false,
            ),
            self.snippet.builder.expression_call(
              SPAN,
              self.snippet.builder.expression_identifier_reference(SPAN, "require"),
              NONE,
              self.snippet.builder.vec1(ast::Argument::from(alternative)),
              false,
            ),
          );

          Some(ast::Expression::ConditionalExpression(new_cond_expr))
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
