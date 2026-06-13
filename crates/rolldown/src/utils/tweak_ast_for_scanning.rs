use itertools::Itertools;
use oxc::allocator::{Address, Allocator, GetAddress, TakeIn};
use oxc::ast::NONE;
use oxc::ast::ast::{self, BindingPattern, Declaration, ImportOrExportKind, Statement};
use oxc::ast_visit::{VisitMut, walk_mut};
use oxc::span::{SPAN, Span};
use rolldown_ecmascript_utils::{AstFactory, StatementExt};
use rustc_hash::{FxHashMap, FxHashSet};

/// Pre-process is a essential step to make rolldown generate correct and efficient code.
pub struct PreProcessor<'ast, 'a> {
  ast_factory: AstFactory<'ast>,
  /// used to store none_hoisted statements.
  top_level_stmt_temp_storage: Vec<Statement<'ast>>,
  keep_names: bool,
  /// Labels listed in `transform.dropLabels`. When non-empty, any matching
  /// `LabeledStatement` is replaced with an empty statement before scanning,
  /// so dynamic imports nested inside the dropped block never enter the
  /// module graph.
  drop_labels: Option<&'a FxHashSet<String>>,
  statement_stack: Vec<Address>,
  statement_replace_map: FxHashMap<Address, Vec<Statement<'ast>>>,
  /// Spans of `import defer ...` statements / expressions whose `defer` phase
  /// was lowered to a regular import. Read after `visit_program` to emit the
  /// `UNSUPPORTED_FEATURE` warning.
  defer_spans: Vec<Span>,
}

impl<'ast, 'a> PreProcessor<'ast, 'a> {
  pub fn new(
    alloc: &'ast Allocator,
    keep_names: bool,
    drop_labels: Option<&'a FxHashSet<String>>,
  ) -> Self {
    Self {
      ast_factory: AstFactory::new(alloc),
      top_level_stmt_temp_storage: vec![],
      keep_names,
      drop_labels: drop_labels.filter(|set| !set.is_empty()),
      statement_stack: vec![],
      statement_replace_map: FxHashMap::default(),
      defer_spans: vec![],
    }
  }

  pub fn take_defer_spans(&mut self) -> Vec<Span> {
    std::mem::take(&mut self.defer_spans)
  }

  /// Replace `it` with an empty statement when it is a `LabeledStatement`
  /// whose label name appears in `drop_labels`. Returns true if a replacement
  /// was performed, so callers can skip walking into the dropped subtree.
  fn try_drop_labeled(&self, it: &mut Statement<'ast>) -> bool {
    let Some(labels) = self.drop_labels else { return false };
    if let Statement::LabeledStatement(stmt) = it
      && labels.contains(stmt.label.name.as_str())
    {
      *it = self.ast_factory.statement_empty(stmt.span);
      return true;
    }
    false
  }

  /// split `var a = 1, b = 2;` into `var a = 1; var b = 2;`
  fn split_var_declaration(
    &self,
    var_decl: &mut ast::VariableDeclaration<'ast>,
    named_decl_span: Option<Span>,
  ) -> Vec<Statement<'ast>> {
    var_decl
      .declarations
      .take_in(self.ast_factory.allocator)
      .into_iter()
      .enumerate()
      .map(|(i, declarator)| {
        let new_decl = self.ast_factory.alloc_variable_declaration(
          SPAN,
          var_decl.kind,
          self.ast_factory.vec_from_iter([declarator]),
          var_decl.declare,
        );
        if let Some(named_decl_span) = named_decl_span {
          Statement::ExportNamedDeclaration(self.ast_factory.alloc_export_named_declaration(
            if i == 0 { named_decl_span } else { SPAN },
            Some(Declaration::VariableDeclaration(new_decl)),
            self.ast_factory.vec(),
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

impl<'ast> VisitMut<'ast> for PreProcessor<'ast, '_> {
  fn visit_import_declaration(&mut self, it: &mut ast::ImportDeclaration<'ast>) {
    if matches!(it.phase, Some(ast::ImportPhase::Defer)) {
      self.defer_spans.push(it.span);
      it.phase = None;
    }
    walk_mut::walk_import_declaration(self, it);
  }

  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    let original_body = program.body.take_in(self.ast_factory.allocator);
    program.body.reserve_exact(original_body.len());
    self.top_level_stmt_temp_storage = Vec::with_capacity(
      original_body.iter().filter(|stmt| !stmt.is_module_declaration_with_source()).count(),
    );

    for mut stmt in original_body {
      if self.try_drop_labeled(&mut stmt) {
        self.top_level_stmt_temp_storage.push(stmt);
        continue;
      }
      let stmt_addr = stmt.address();
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

  /// Some declaration like:
  /// ```js
  /// if var a = function() {}
  /// else {
  ///   somethingElse();
  /// }
  /// ```
  /// Will not reach `visit_statements`, so we need to handle it separately.
  /// Since we already intercept `visit_statements`, these two visitor now are mutually exclusive.
  fn visit_statement(&mut self, it: &mut Statement<'ast>) {
    if self.try_drop_labeled(it) {
      return;
    }
    if self.keep_names {
      let stmt_addr = it.address();
      self.statement_stack.push(stmt_addr);
      walk_mut::walk_statement(self, it);
      self.statement_stack.pop();

      if let Some(stmts) = self.statement_replace_map.remove(&stmt_addr) {
        *it = Statement::BlockStatement(
          self.ast_factory.alloc_block_statement(SPAN, self.ast_factory.vec_from_iter(stmts)),
        );
      }
    } else {
      walk_mut::walk_statement(self, it);
    }
  }

  /// If `keep_names` is true, we will keep the names of (function/class) variable declarations even it is not top level.
  fn visit_statements(&mut self, it: &mut oxc::allocator::Vec<'ast, Statement<'ast>>) {
    if self.keep_names {
      let stmts = it.take_in(self.ast_factory.allocator);
      for mut stmt in stmts {
        if self.try_drop_labeled(&mut stmt) {
          it.push(stmt);
          continue;
        }
        let stmt_addr = stmt.address();
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
      Declaration::FunctionDeclaration(_)
      | Declaration::ClassDeclaration(_)
      | Declaration::TSTypeAliasDeclaration(_)
      | Declaration::TSInterfaceDeclaration(_)
      | Declaration::TSEnumDeclaration(_)
      | Declaration::TSModuleDeclaration(_)
      | Declaration::TSImportEqualsDeclaration(_)
      | Declaration::TSGlobalDeclaration(_) => {}
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
        .any(|declarator| matches!(declarator.id, BindingPattern::BindingIdentifier(_)))
    {
      let rewritten = self.split_var_declaration(var_decl, Some(named_decl.span));
      self.statement_replace_map.insert(self.statement_stack.last().copied().unwrap(), rewritten);
    }
  }

  fn visit_import_expression(&mut self, it: &mut ast::ImportExpression<'ast>) {
    if matches!(it.phase, Some(ast::ImportPhase::Defer)) {
      self.defer_spans.push(it.span);
      it.phase = None;
    }
    walk_mut::walk_import_expression(self, it);
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
          let test = cond_expr.test.take_in(self.ast_factory.allocator);
          let consequent = cond_expr.consequent.take_in(self.ast_factory.allocator);
          let alternative = cond_expr.alternate.take_in(self.ast_factory.allocator);
          let new_cond_expr = self.ast_factory.alloc_conditional_expression(
            SPAN,
            test,
            self.ast_factory.expression_call(
              SPAN,
              self.ast_factory.expression_identifier(SPAN, "require"),
              NONE,
              self.ast_factory.vec1(ast::Argument::from(consequent)),
              false,
            ),
            self.ast_factory.expression_call(
              SPAN,
              self.ast_factory.expression_identifier(SPAN, "require"),
              NONE,
              self.ast_factory.vec1(ast::Argument::from(alternative)),
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
            let test = cond_expr.test.take_in(self.ast_factory.allocator);
            let consequent = cond_expr.consequent.take_in(self.ast_factory.allocator);
            let alternative = cond_expr.alternate.take_in(self.ast_factory.allocator);

            let new_cond_expr = self.ast_factory.expression_conditional(
              SPAN,
              test,
              self.ast_factory.expression_import(SPAN, consequent, None, None),
              self.ast_factory.expression_import(SPAN, alternative, None, None),
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
