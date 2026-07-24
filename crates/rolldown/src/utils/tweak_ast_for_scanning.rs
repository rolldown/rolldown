use itertools::Itertools;
use oxc::allocator::GetAllocator;
use oxc::allocator::{Allocator, ReplaceWith, TakeIn};
use oxc::ast::ast::{self, BindingPattern, Declaration, ImportOrExportKind, Statement};
use oxc::ast::builder::{AstBuilder, NONE};
use oxc::ast_visit::{VisitMut, walk_mut};
use oxc::span::{SPAN, Span};
use rolldown_ecmascript_utils::StatementExt;
use rustc_hash::FxHashSet;

/// Pre-process is a essential step to make rolldown generate correct and efficient code.
pub struct PreProcessor<'ast, 'a> {
  ast_builder: AstBuilder<'ast>,
  /// used to store none_hoisted statements.
  top_level_stmt_temp_storage: Vec<Statement<'ast>>,
  keep_names: bool,
  /// Labels listed in `transform.dropLabels`. When non-empty, any matching
  /// `LabeledStatement` is replaced with an empty statement before scanning,
  /// so dynamic imports nested inside the dropped block never enter the
  /// module graph.
  drop_labels: Option<&'a FxHashSet<String>>,
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
      ast_builder: AstBuilder::new(alloc),
      top_level_stmt_temp_storage: vec![],
      keep_names,
      drop_labels: drop_labels.filter(|set| !set.is_empty()),
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
      *it = ast::Statement::new_empty_statement(stmt.span, &self.ast_builder);
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
    // Keep the original statement span on the first replacement (on the export
    // wrapper when there is one, on the declaration itself otherwise) so
    // comments attached to the statement's start position stay attached.
    let var_decl_span = var_decl.span;
    var_decl
      .declarations
      .take_in(&self.ast_builder.allocator())
      .into_iter()
      .enumerate()
      .map(|(i, declarator)| {
        let new_decl = ast::VariableDeclaration::boxed(
          if i == 0 && named_decl_span.is_none() { var_decl_span } else { SPAN },
          var_decl.kind,
          oxc::allocator::Vec::from_iter_in([declarator], &self.ast_builder),
          var_decl.declare,
          &self.ast_builder,
        );
        if let Some(named_decl_span) = named_decl_span {
          Statement::new_export_named_declaration(
            if i == 0 { named_decl_span } else { SPAN },
            Some(Declaration::VariableDeclaration(new_decl)),
            [],
            // Since it is `export a = 1, b = 2;`, source should be `None`
            None,
            ImportOrExportKind::Value,
            NONE,
            &self.ast_builder,
          )
        } else {
          Statement::VariableDeclaration(new_decl)
        }
      })
      .collect_vec()
  }

  fn should_split_var_declaration(var_decl: &ast::VariableDeclaration<'ast>) -> bool {
    var_decl.declarations.len() > 1
      && var_decl
        .declarations
        .iter()
        .all(|declarator| matches!(declarator.id, BindingPattern::BindingIdentifier(_)))
  }

  /// Decide whether a single statement should be split into one statement per
  /// declarator, and if so produce the replacement statements.
  ///
  /// The decision is made here — at the statement level, where the full
  /// statement (export wrapper included) is visible — so every statement is
  /// inspected exactly once by exactly one caller. There is no deferred
  /// replacement map and no exported-vs-bare ambiguity, which is what made the
  /// previous two-visitor approach (and its `next_declaration_is_exported`
  /// referee flag) fragile.
  ///
  /// - `export var a = 1, b = 2;` -> `export var a = 1; export var b = 2;` (always; per-export tree-shaking)
  /// - `var a = 1, b = 2;` at top level -> `var a = 1; var b = 2;` (always; per-declarator tree-shaking)
  /// - `var a = 1, b = 2;` nested -> `var a = 1; var b = 2;` (only under `keepNames`)
  /// - `const [a] = iterable, b = 2;` -> left grouped; destructuring can perform iterator/property work
  ///
  /// Tree-shaking includes whole top-level statements, so a bare multi-declarator
  /// statement must be split too: demanding one declarator (via `export { a }`, or
  /// any other reference) would otherwise keep all of them, and the dce-only
  /// minifier can't clean up declarators that reference each other in a cycle
  /// (see rolldown/rolldown#10165). Nested declarations never get their own
  /// statement info, so splitting them only matters for `keepNames`. The split is
  /// limited to plain identifier bindings because destructuring has binding-time
  /// effects that are not represented after the declarator is isolated.
  fn split_multi_declarator(
    &self,
    stmt: &mut Statement<'ast>,
    top_level: bool,
  ) -> Option<Vec<Statement<'ast>>> {
    match stmt {
      Statement::ExportNamedDeclaration(named_decl) => {
        let named_decl_span = named_decl.span;
        let Some(Declaration::VariableDeclaration(var_decl)) = named_decl.declaration.as_mut()
        else {
          return None;
        };
        Self::should_split_var_declaration(var_decl)
          .then(|| self.split_var_declaration(var_decl, Some(named_decl_span)))
      }
      Statement::VariableDeclaration(var_decl) => ((top_level || self.keep_names)
        && Self::should_split_var_declaration(var_decl))
      .then(|| self.split_var_declaration(var_decl, None)),
      _ => None,
    }
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
    let original_body = program.body.take_in(&self.ast_builder.allocator());
    program.body.reserve_exact(original_body.len());
    self.top_level_stmt_temp_storage = Vec::with_capacity(
      original_body.iter().filter(|stmt| !stmt.is_module_declaration_with_source()).count(),
    );

    for mut stmt in original_body {
      if self.try_drop_labeled(&mut stmt) {
        self.top_level_stmt_temp_storage.push(stmt);
        continue;
      }
      walk_mut::walk_statement(self, &mut stmt);
      if let Some(split) = self.split_multi_declarator(&mut stmt, true) {
        self.top_level_stmt_temp_storage.extend(split);
      } else if stmt.is_module_declaration_with_source() {
        program.body.push(stmt);
      } else {
        self.top_level_stmt_temp_storage.push(stmt);
      }
    }
    program.body.extend(std::mem::take(&mut self.top_level_stmt_temp_storage));
  }

  /// Single-statement slots (e.g. a braceless `if (cond) var a = fn, b = fn;`)
  /// don't go through `visit_statements`, so the split is handled here too.
  /// Because the split yields several statements that can't occupy a single
  /// slot, the result is wrapped in a block.
  fn visit_statement(&mut self, it: &mut Statement<'ast>) {
    if self.try_drop_labeled(it) {
      return;
    }
    walk_mut::walk_statement(self, it);
    if let Some(split) = self.split_multi_declarator(it, false) {
      *it = Statement::new_block_statement(
        SPAN,
        oxc::allocator::Vec::from_iter_in(split, &self.ast_builder),
        &self.ast_builder,
      );
    }
  }

  /// If `keep_names` is true, we keep the names of (function/class) variable
  /// declarations even when they are not top level, by splitting multi-declarator
  /// `var`s here so each binding becomes independently tree-shakeable.
  fn visit_statements(&mut self, it: &mut oxc::allocator::Vec<'ast, Statement<'ast>>) {
    if !self.keep_names {
      walk_mut::walk_statements(self, it);
      return;
    }
    let stmts = it.take_in(&self.ast_builder.allocator());
    for mut stmt in stmts {
      if self.try_drop_labeled(&mut stmt) {
        it.push(stmt);
        continue;
      }
      walk_mut::walk_statement(self, &mut stmt);
      if let Some(split) = self.split_multi_declarator(&mut stmt, false) {
        it.extend(split);
      } else {
        it.push(stmt);
      }
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
    // transpose `require(test ? 'a' : 'b')` into `test ? require('a') : require('b')`
    if matches!(it, ast::Expression::CallExpression(expr)
    if expr.callee.is_specific_id("require")
      && expr.arguments.len() == 1
      && matches!(
        expr.arguments[0].as_expression(),
        Some(ast::Expression::ConditionalExpression(_))
      ))
    {
      it.replace_with(|old| {
        let ast::Expression::CallExpression(call_expr) = old else { unreachable!() };
        let Some(ast::Argument::ConditionalExpression(cond_expr)) =
          call_expr.unbox().arguments.into_iter().next()
        else {
          unreachable!()
        };
        let cond_expr = cond_expr.unbox();
        ast::Expression::new_conditional_expression(
          SPAN,
          cond_expr.test,
          ast::Expression::new_call_expression(
            SPAN,
            ast::Expression::new_identifier(SPAN, "require", &self.ast_builder),
            NONE,
            [ast::Argument::from(cond_expr.consequent)],
            false,
            &self.ast_builder,
          ),
          ast::Expression::new_call_expression(
            SPAN,
            ast::Expression::new_identifier(SPAN, "require", &self.ast_builder),
            NONE,
            [ast::Argument::from(cond_expr.alternate)],
            false,
            &self.ast_builder,
          ),
          &self.ast_builder,
        )
      });
    }
    // transpose `import(test ? 'a' : 'b')` into `test ? import('a') : import('b')`
    else if matches!(it, ast::Expression::ImportExpression(expr)
      if expr.options.is_none()
        && matches!(expr.source, ast::Expression::ConditionalExpression(_)))
    {
      it.replace_with(|old| {
        let ast::Expression::ImportExpression(import_expr) = old else { unreachable!() };
        let ast::Expression::ConditionalExpression(cond_expr) = import_expr.unbox().source else {
          unreachable!()
        };
        let cond_expr = cond_expr.unbox();
        ast::Expression::new_conditional_expression(
          SPAN,
          cond_expr.test,
          ast::Expression::new_import_expression(
            SPAN,
            cond_expr.consequent,
            None,
            None,
            &self.ast_builder,
          ),
          ast::Expression::new_import_expression(
            SPAN,
            cond_expr.alternate,
            None,
            None,
            &self.ast_builder,
          ),
          &self.ast_builder,
        )
      });
    }
    walk_mut::walk_expression(self, it);
  }
}
