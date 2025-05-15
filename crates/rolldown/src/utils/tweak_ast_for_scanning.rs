use itertools::Itertools;
use oxc::allocator::{Address, Allocator, TakeIn};
use oxc::ast::NONE;
use oxc::ast::ast::{self, BindingPatternKind, Declaration, ImportOrExportKind, Statement};
use oxc::ast_visit::{VisitMut, walk_mut};
use oxc::span::{SPAN, Span};
use rolldown_ecmascript_utils::{AstSnippet, StatementExt};
use rustc_hash::FxHashMap;

/// Pre-process is a essential step to make rolldown generate correct and efficient code.
pub struct PreProcessor<'ast> {
  snippet: AstSnippet<'ast>,
  /// used to store none_hoisted statements.
  top_level_stmt_temp_storage: Vec<Statement<'ast>>,
  keep_names: bool,
  statement_stack: Vec<Address>,
  statement_replace_map: FxHashMap<Address, Vec<Statement<'ast>>>,
}

impl<'ast> PreProcessor<'ast> {
  pub fn new(alloc: &'ast Allocator, keep_names: bool) -> Self {
    Self {
      snippet: AstSnippet::new(alloc),
      top_level_stmt_temp_storage: vec![],
      keep_names,
      statement_stack: vec![],
      statement_replace_map: FxHashMap::default(),
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
    let original_body = program.body.take_in(self.snippet.alloc());
    program.body.reserve_exact(original_body.len());
    self.top_level_stmt_temp_storage = Vec::with_capacity(
      original_body.iter().filter(|stmt| !stmt.is_module_declaration_with_source()).count(),
    );

    for mut stmt in original_body {
      let stmt_addr = Address::from_ptr(&stmt);
      self.statement_stack.push(stmt_addr);
      walk_mut::walk_statement(self, &mut stmt);
      self.statement_stack.pop();
      if let Some(stmts) = self.statement_replace_map.remove(&stmt_addr) {
        self.top_level_stmt_temp_storage.extend(stmts);
      } else if stmt.is_module_declaration_with_source() {
        program.body.push(stmt);
      } else {
        self.top_level_stmt_temp_storage.push(stmt);
      }
    }
    program.body.extend(std::mem::take(&mut self.top_level_stmt_temp_storage));
  }

  fn visit_statements(&mut self, it: &mut oxc::allocator::Vec<'ast, Statement<'ast>>) {
    if self.keep_names {
      let stmts = it.take_in(self.snippet.alloc());
      for mut stmt in stmts {
        let stmt_addr = Address::from_ptr(&stmt);
        self.statement_stack.push(stmt_addr);
        walk_mut::walk_statement(self, &mut stmt);
        self.statement_stack.pop();

        if let Some(stmts) = self.statement_replace_map.remove(&stmt_addr) {
          it.extend(stmts);
        } else {
          it.push(stmt);
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
          let stmt_addr = self.statement_stack.last().copied().unwrap();
          let new_stmts = self.split_var_declaration(decl, None);
          self.statement_replace_map.insert(stmt_addr, new_stmts);
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

    let Some(Declaration::VariableDeclaration(ref mut var_decl)) = named_decl.declaration else {
      return;
    };

    if var_decl.declarations.len() > 1
      && var_decl
        .declarations
        .iter()
        // TODO: support nested destructuring tree shake, `export const {a, b} = obj; export const
        // [a, b] = arr;`
        .any(|declarator| matches!(declarator.id.kind, BindingPatternKind::BindingIdentifier(_)))
    {
      let rewritten = self.split_var_declaration(var_decl, Some(named_decl.span));
      self.statement_replace_map.insert(self.statement_stack.last().copied().unwrap(), rewritten);
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
              self.snippet.builder.expression_identifier(SPAN, "require"),
              NONE,
              self.snippet.builder.vec1(ast::Argument::from(consequent)),
              false,
            ),
            self.snippet.builder.expression_call(
              SPAN,
              self.snippet.builder.expression_identifier(SPAN, "require"),
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
      ast::Expression::ImportExpression(expr) if expr.options.is_none() => {
        let source = &mut expr.source;
        match source {
          ast::Expression::ConditionalExpression(cond_expr) => {
            let test = cond_expr.test.take_in(self.snippet.alloc());
            let consequent = cond_expr.consequent.take_in(self.snippet.alloc());
            let alternative = cond_expr.alternate.take_in(self.snippet.alloc());

            let new_cond_expr = self.snippet.builder.expression_conditional(
              SPAN,
              test,
              self.snippet.builder.expression_import(SPAN, consequent, None, None),
              self.snippet.builder.expression_import(SPAN, alternative, None, None),
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
