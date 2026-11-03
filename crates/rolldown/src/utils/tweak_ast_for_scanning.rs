use itertools::Itertools;
use oxc::allocator::GetAllocator;
use oxc::allocator::{Allocator, Box as ArenaBox, CloneIn, ReplaceWith, TakeIn};
use oxc::ast::ast::{
  self, BindingPattern, Declaration, Expression, ImportDeclarationSpecifier, ImportOrExportKind,
  PropertyKey, Statement, VariableDeclarationKind,
};
use oxc::ast::builder::{AstBuilder, GetAstBuilder};
use oxc::ast_visit::{VisitJsMut, walk_js_mut};
use oxc::span::{SPAN, Span};
use oxc_str::{Ident, IdentHashMap, IdentHashSet};
use rolldown_ecmascript_utils::StatementExt;
use rustc_hash::FxHashSet;

type NamespaceImportSource<'ast> =
  (ast::StringLiteral<'ast>, Option<ArenaBox<'ast, ast::WithClause<'ast>>>);

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
  /// Top-level namespace imports and their `const` aliases. The scanner tree-shakes a namespace
  /// only through member reads, so destructuring these is rewritten into member reads.
  namespace_names: IdentHashSet<'ast>,
  /// Top-level `const alias = ns;` declarations, rewritten into `import * as alias` so the
  /// scanner sees a namespace import instead of a reference to the whole namespace.
  namespace_aliases: IdentHashMap<'ast, NamespaceImportSource<'ast>>,
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
      namespace_names: IdentHashSet::default(),
      namespace_aliases: IdentHashMap::default(),
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
      *it = ast::Statement::new_empty_statement(stmt.span, self);
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
      .take_in(self)
      .into_iter()
      .enumerate()
      .map(|(i, declarator)| {
        let new_decl = ast::VariableDeclaration::boxed(
          if i == 0 && named_decl_span.is_none() { var_decl_span } else { SPAN },
          var_decl.kind,
          oxc::allocator::Vec::from_iter_in([declarator], self),
          var_decl.declare,
          self,
        );
        if let Some(named_decl_span) = named_decl_span {
          Statement::new_export_declaration(
            if i == 0 { named_decl_span } else { SPAN },
            Declaration::VariableDeclaration(new_decl),
            self,
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
      Statement::ExportDeclaration(export_decl) => {
        let export_decl_span = export_decl.span;
        let Declaration::VariableDeclaration(var_decl) = &mut export_decl.declaration else {
          return None;
        };
        Self::should_split_var_declaration(var_decl)
          .then(|| self.split_var_declaration(var_decl, Some(export_decl_span)))
      }
      Statement::VariableDeclaration(var_decl) => ((top_level || self.keep_names)
        && Self::should_split_var_declaration(var_decl))
      .then(|| self.split_var_declaration(var_decl, None)),
      _ => None,
    }
  }

  fn identifier_alias(
    declarator: &ast::VariableDeclarator<'ast>,
  ) -> Option<(Ident<'ast>, Ident<'ast>)> {
    let BindingPattern::BindingIdentifier(id) = &declarator.id else { return None };
    let Some(Expression::Identifier(init)) = &declarator.init else { return None };

    Some((id.name, init.name))
  }

  /// Runs before any statement is walked, since a function declared above an alias can
  /// destructure it. Only declarations that become a single-declarator `const` after
  /// `split_multi_declarator` count, matching what `rewrite_namespace_alias` accepts.
  fn collect_namespace_bindings(&mut self, body: &[Statement<'ast>]) {
    let mut import_stmt_of: IdentHashMap<'ast, usize> = IdentHashMap::default();

    for (idx, stmt) in body.iter().enumerate() {
      let Statement::ImportDeclaration(decl) = stmt else { continue };

      for spec in decl.specifiers.iter().flatten() {
        if let ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) = spec {
          import_stmt_of.insert(spec.local.name, idx);
        }
      }
    }

    if import_stmt_of.is_empty() {
      return;
    }

    for stmt in body {
      let Statement::VariableDeclaration(decl) = stmt else { continue };

      if decl.kind != VariableDeclarationKind::Const
        || decl.declare
        || !(decl.declarations.len() == 1 || Self::should_split_var_declaration(decl))
      {
        continue;
      }

      for declarator in &decl.declarations {
        let Some((alias, target)) = Self::identifier_alias(declarator) else { continue };
        let Some(&idx) = import_stmt_of.get(&target) else { continue };
        let Statement::ImportDeclaration(import_decl) = &body[idx] else { unreachable!() };
        let source = import_decl.source.clone_in(self.allocator());
        let with_clause = import_decl.with_clause.clone_in(self.allocator());
        self.namespace_aliases.insert(alias, (source, with_clause));
        import_stmt_of.insert(alias, idx);
      }
    }

    self.namespace_names.extend(import_stmt_of.into_keys());
  }

  /// `const alias = ns;` -> `import * as alias from '<source of ns>';`
  ///
  /// Both imports link to the same namespace object, so `alias === ns` still holds, also for
  /// CommonJS importees. Reading `alias` before its declaration no longer throws (TDZ).
  fn rewrite_namespace_alias(&mut self, stmt: Statement<'ast>) -> Statement<'ast> {
    let Statement::VariableDeclaration(decl) = &stmt else { return stmt };

    if decl.kind != VariableDeclarationKind::Const || decl.declare || decl.declarations.len() != 1 {
      return stmt;
    }

    let Some((alias, _)) = Self::identifier_alias(&decl.declarations[0]) else { return stmt };
    let Some((source, with_clause)) = self.namespace_aliases.remove(&alias) else { return stmt };

    let Statement::VariableDeclaration(decl) = stmt else { unreachable!() };
    let decl = decl.unbox();
    let Some(declarator) = decl.declarations.into_iter().next() else { unreachable!() };
    let BindingPattern::BindingIdentifier(local) = declarator.id else { unreachable!() };
    let specifier =
      ImportDeclarationSpecifier::new_import_namespace_specifier(local.span, local.unbox(), self);

    Statement::new_import_declaration(
      decl.span,
      Some(oxc::allocator::Vec::from_iter_in([specifier], self)),
      source,
      None,
      with_clause,
      ImportOrExportKind::Value,
      self,
    )
  }

  fn push_top_level_stmt(
    &mut self,
    body: &mut oxc::allocator::Vec<'ast, Statement<'ast>>,
    stmt: Statement<'ast>,
  ) {
    let stmt = self.rewrite_namespace_alias(stmt);

    if stmt.is_module_declaration_with_source() {
      body.push(stmt);
    } else {
      self.top_level_stmt_temp_storage.push(stmt);
    }
  }

  /// Without scope info, a local that shadows the import is rewritten too. That is safe because
  /// both forms read the same properties in the same order and throw on the same values. They
  /// differ only if `ns` changes between reads: a binding named `ns` is rejected, and a getter
  /// that reassigns a shadowing local is not handled.
  fn is_namespace_destructuring(&self, declarator: &ast::VariableDeclarator<'ast>) -> bool {
    let BindingPattern::ObjectPattern(pattern) = &declarator.id else { return false };
    let Some(Expression::Identifier(ns)) = &declarator.init else { return false };

    self.namespace_names.contains(&ns.name)
      && pattern.rest.is_none()
      && !pattern.properties.is_empty()
      && pattern.properties.iter().all(|prop| {
        !prop.computed
          && matches!(prop.key, PropertyKey::StaticIdentifier(_))
          && matches!(&prop.value, BindingPattern::BindingIdentifier(id) if id.name != ns.name)
      })
  }

  /// `const { a, b: c } = ns` -> `const a = ns.a, c = ns.b`
  fn split_namespace_destructuring(&self, decl: &mut ast::VariableDeclaration<'ast>) {
    let declarations = decl.declarations.take_in(self);
    let mut rewritten = oxc::allocator::Vec::with_capacity_in(declarations.len(), self);

    for declarator in declarations {
      if !self.is_namespace_destructuring(&declarator) {
        rewritten.push(declarator);
        continue;
      }

      let (BindingPattern::ObjectPattern(pattern), Some(Expression::Identifier(ns))) =
        (declarator.id, declarator.init)
      else {
        unreachable!()
      };

      for prop in pattern.unbox().properties {
        let (PropertyKey::StaticIdentifier(key), BindingPattern::BindingIdentifier(binding)) =
          (prop.key, prop.value)
        else {
          unreachable!()
        };

        // The scanner skips unspanned member expressions, so reuse the property span.
        let init = Expression::new_static_member_expression(
          prop.span,
          Expression::new_identifier(ns.span, ns.name, self),
          key.unbox(),
          false,
          self,
        );

        rewritten.push(ast::VariableDeclarator::new(
          prop.span,
          BindingPattern::BindingIdentifier(binding),
          None,
          Some(init),
          false,
          self,
        ));
      }
    }

    decl.declarations = rewritten;
  }
}

impl<'ast> VisitJsMut<'ast> for PreProcessor<'ast, '_> {
  fn visit_import_declaration(&mut self, it: &mut ast::ImportDeclaration<'ast>) {
    if matches!(it.phase, Some(ast::ImportPhase::Defer)) {
      self.defer_spans.push(it.span);
      it.phase = None;
    }
    walk_js_mut::walk_import_declaration(self, it);
  }

  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    let original_body = program.body.take_in(self);
    program.body.reserve_exact(original_body.len());
    self.top_level_stmt_temp_storage = Vec::with_capacity(
      original_body.iter().filter(|stmt| !stmt.is_module_declaration_with_source()).count(),
    );
    self.collect_namespace_bindings(&original_body);

    for mut stmt in original_body {
      if self.try_drop_labeled(&mut stmt) {
        self.top_level_stmt_temp_storage.push(stmt);
        continue;
      }
      walk_js_mut::walk_statement(self, &mut stmt);
      if let Some(split) = self.split_multi_declarator(&mut stmt, true) {
        for stmt in split {
          self.push_top_level_stmt(&mut program.body, stmt);
        }
      } else {
        self.push_top_level_stmt(&mut program.body, stmt);
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
    walk_js_mut::walk_statement(self, it);
    if let Some(split) = self.split_multi_declarator(it, false) {
      *it =
        Statement::new_block_statement(SPAN, oxc::allocator::Vec::from_iter_in(split, self), self);
    }
  }

  /// If `keep_names` is true, we keep the names of (function/class) variable
  /// declarations even when they are not top level, by splitting multi-declarator
  /// `var`s here so each binding becomes independently tree-shakeable.
  fn visit_statements(&mut self, it: &mut oxc::allocator::Vec<'ast, Statement<'ast>>) {
    if !self.keep_names {
      walk_js_mut::walk_statements(self, it);
      return;
    }
    let stmts = it.take_in(self);
    for mut stmt in stmts {
      if self.try_drop_labeled(&mut stmt) {
        it.push(stmt);
        continue;
      }
      walk_js_mut::walk_statement(self, &mut stmt);
      if let Some(split) = self.split_multi_declarator(&mut stmt, false) {
        it.extend(split);
      } else {
        it.push(stmt);
      }
    }
  }

  fn visit_variable_declaration(&mut self, it: &mut ast::VariableDeclaration<'ast>) {
    walk_js_mut::walk_variable_declaration(self, it);

    if matches!(
      it.kind,
      VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const
    ) && !self.namespace_names.is_empty()
      && it.declarations.iter().any(|declarator| self.is_namespace_destructuring(declarator))
    {
      self.split_namespace_destructuring(it);
    }
  }

  fn visit_import_expression(&mut self, it: &mut ast::ImportExpression<'ast>) {
    if matches!(it.phase, Some(ast::ImportPhase::Defer)) {
      self.defer_spans.push(it.span);
      it.phase = None;
    }
    walk_js_mut::walk_import_expression(self, it);
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
            ast::Expression::new_identifier(SPAN, "require", self),
            None,
            [ast::Argument::from(cond_expr.consequent)],
            false,
            self,
          ),
          ast::Expression::new_call_expression(
            SPAN,
            ast::Expression::new_identifier(SPAN, "require", self),
            None,
            [ast::Argument::from(cond_expr.alternate)],
            false,
            self,
          ),
          self,
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
          ast::Expression::new_import_expression(SPAN, cond_expr.consequent, None, None, self),
          ast::Expression::new_import_expression(SPAN, cond_expr.alternate, None, None, self),
          self,
        )
      });
    }
    walk_js_mut::walk_expression(self, it);
  }
}

impl<'ast> GetAstBuilder<'ast> for PreProcessor<'ast, '_> {
  type Builder = AstBuilder<'ast>;

  #[inline]
  fn builder(&self) -> &AstBuilder<'ast> {
    &self.ast_builder
  }
}

impl<'ast> GetAllocator<'ast> for PreProcessor<'ast, '_> {
  #[inline]
  fn allocator(&self) -> &'ast Allocator {
    self.ast_builder.allocator()
  }
}
