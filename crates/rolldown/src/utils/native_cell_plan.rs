//! Plan-only eligibility for a first native declaration-cell placement slice.
//!
//! This does not emit, move or omit anything. Local plans permit only a physical split retaining
//! all evaluation edges and the complete original namespace facade. Bypassing facade evaluation
//! additionally requires current linked-graph facts. Symbol IDs and spans belong to this exact AST.

use oxc::{
  ast::ast::{self, Declaration, Expression, Statement},
  ast_visit::{VisitJs, walk_js},
  semantic::{ScopeFlags, ScopeId, Scoping, SymbolId},
  span::Span,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exclusion {
  NonModule,
  Await,
  Eval,
  MissingSemanticBinding,
  CapturedBinding(String),
  UnresolvedBinding(String),
  SelfBinding,
  WrittenBinding,
  ContextSensitiveExpression,
  AsyncOrGenerator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCell {
  pub name: String,
  pub symbol: SymbolId,
  pub declaration: Span,
  pub export_statement: Span,
  /// Reads outside the declaration; the facade must keep a local imported binding for these.
  pub local_read_count: usize,
  pub exported_names: Vec<String>,
}

/// An exported `const` statement whose every declarator binds a primitive literal. Its bindings
/// move to the cell unchanged; the facade imports and re-exports each one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantCell {
  pub names: Vec<String>,
  pub declaration: Span,
  pub export_statement: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedFunction {
  pub name: String,
  pub reason: Exclusion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCellPlan {
  pub functions: Vec<FunctionCell>,
  /// Only from [`plan_source`]; see [`collect_constant_cells`].
  pub constants: Vec<ConstantCell>,
  pub rejected: Vec<RejectedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePlanError {
  InvalidSource(Vec<String>),
  Excluded(Exclusion),
}

/// Parse with the compiler's own Oxc version; offsets are UTF-8 byte offsets in `source`.
/// This local plan does not certify graph facts or permit omission of facade evaluation; it may be
/// used before linking, only for an evaluation-preserving split. It does not prove cycles,
/// ambiguity, or transitive await safe to bypass.
pub fn plan_source(source: &str) -> Result<LocalCellPlan, SourcePlanError> {
  let allocator = oxc::allocator::Allocator::default();
  let parsed = oxc::parser::Parser::new(&allocator, source, oxc::span::SourceType::mjs()).parse();
  if !parsed.diagnostics.is_empty() {
    return Err(SourcePlanError::InvalidSource(
      parsed.diagnostics.iter().map(ToString::to_string).collect(),
    ));
  }
  let semantic = oxc::semantic::SemanticBuilder::new().build(&parsed.program);
  if !semantic.diagnostics.is_empty() {
    return Err(SourcePlanError::InvalidSource(
      semantic.diagnostics.iter().map(ToString::to_string).collect(),
    ));
  }
  check_local_module(&parsed.program).map_err(SourcePlanError::Excluded)?;
  let scoping = semantic.semantic.scoping();
  let mut plan = collect_function_cells(&parsed.program, scoping);
  plan.constants = collect_constant_cells(&parsed.program, scoping);
  Ok(plan)
}

/// Literal `const` bindings that no code can observe uninitialized. Unlike a hoisted function,
/// a `const` has a temporal dead zone, which moving it to an earlier-evaluated cell would erase.
/// So the module must have no static import or re-export (it is in no cycle, and no other
/// module's code runs before its body), and only inert statements, which run no code, may precede
/// the declaration. A later evaluation of the facade then never reads these bindings before the
/// original declaration point, and every other module reads them only after that point.
///
/// A cell only helps when the rest of the module is omittable by annotation alone: every other
/// top-level statement is inert or a declaration initialized by literals or `/* @__PURE__ */`
/// calls, and at least one such call exists. Otherwise the facade is either inert itself or has
/// effects that keep its evaluation, and the split would only change how edits update the page.
fn collect_constant_cells(program: &ast::Program<'_>, scoping: &Scoping) -> Vec<ConstantCell> {
  let mut constants = Vec::new();
  if program.body.iter().any(|statement| {
    matches!(
      statement,
      Statement::ImportDeclaration(_)
        | Statement::ExportAllDeclaration(_)
        | Statement::ExportFromDeclaration(_)
    )
  }) {
    return constants;
  }
  if !omittable_by_annotation(program) {
    return constants;
  }
  for statement in &program.body {
    match statement {
      Statement::EmptyStatement(_)
      | Statement::FunctionDeclaration(_)
      | Statement::ExportNamedDeclaration(_) => {}
      Statement::ExportDefaultDeclaration(export)
        if matches!(
          export.declaration,
          ast::ExportDefaultDeclarationKind::FunctionDeclaration(_)
        ) => {}
      Statement::ExportDeclaration(export) => match &export.declaration {
        Declaration::FunctionDeclaration(_) => {}
        Declaration::VariableDeclaration(declaration) => {
          let Some(names) = literal_constant_names(declaration, scoping) else { break };
          constants.push(ConstantCell {
            names,
            declaration: declaration.span,
            export_statement: export.span,
          });
        }
        _ => break,
      },
      _ => break,
    }
  }
  constants
}

fn omittable_by_annotation(program: &ast::Program<'_>) -> bool {
  let mut annotated = false;
  let mut initializers = |declaration: &ast::VariableDeclaration<'_>| {
    !declaration.declare
      && declaration.declarations.iter().all(|declarator| {
        matches!(declarator.id, ast::BindingPattern::BindingIdentifier(_))
          && match &declarator.init {
            None => declaration.kind != ast::VariableDeclarationKind::Const,
            Some(Expression::CallExpression(call)) if call.pure => {
              annotated = true;
              true
            }
            Some(Expression::NewExpression(new)) if new.pure => {
              annotated = true;
              true
            }
            Some(init) => is_primitive_literal(init),
          }
      })
  };
  let inert = program.body.iter().all(|statement| match statement {
    Statement::EmptyStatement(_)
    | Statement::FunctionDeclaration(_)
    | Statement::ExportNamedDeclaration(_) => true,
    Statement::ExportDefaultDeclaration(export) => {
      matches!(export.declaration, ast::ExportDefaultDeclarationKind::FunctionDeclaration(_))
    }
    Statement::VariableDeclaration(declaration) => initializers(declaration),
    Statement::ExportDeclaration(export) => match &export.declaration {
      Declaration::FunctionDeclaration(_) => true,
      Declaration::VariableDeclaration(declaration) => initializers(declaration),
      _ => false,
    },
    _ => false,
  });
  inert && annotated
}

pub fn is_primitive_literal(expression: &Expression<'_>) -> bool {
  match expression {
    Expression::NumericLiteral(_)
    | Expression::StringLiteral(_)
    | Expression::BooleanLiteral(_)
    | Expression::NullLiteral(_)
    | Expression::BigIntLiteral(_) => true,
    Expression::TemplateLiteral(template) => template.expressions.is_empty(),
    _ => false,
  }
}

fn literal_constant_names(
  declaration: &ast::VariableDeclaration<'_>,
  scoping: &Scoping,
) -> Option<Vec<String>> {
  if declaration.kind != ast::VariableDeclarationKind::Const || declaration.declare {
    return None;
  }
  let mut names = Vec::new();
  for declarator in &declaration.declarations {
    let ast::BindingPattern::BindingIdentifier(id) = &declarator.id else { return None };
    let literal = is_primitive_literal(declarator.init.as_ref()?);
    let symbol = id.symbol_id.get()?;
    if !literal || scoping.get_resolved_references(symbol).any(oxc::semantic::Reference::is_write) {
      return None;
    }
    names.push(id.name.to_string());
  }
  Some(names)
}

#[derive(Default)]
struct ModuleBarriers {
  await_found: bool,
  eval_found: bool,
}

impl<'ast> VisitJs<'ast> for ModuleBarriers {
  fn visit_await_expression(&mut self, it: &ast::AwaitExpression<'ast>) {
    self.await_found = true;
    walk_js::walk_await_expression(self, it);
  }

  fn visit_for_of_statement(&mut self, it: &ast::ForOfStatement<'ast>) {
    self.await_found |= it.r#await;
    walk_js::walk_for_of_statement(self, it);
  }

  fn visit_identifier_reference(&mut self, it: &ast::IdentifierReference<'ast>) {
    // Aliased/indirect eval is excluded too; this slice does not prove reflective locality.
    self.eval_found |= it.name == "eval";
  }
}

fn check_local_module(program: &ast::Program<'_>) -> Result<(), Exclusion> {
  if !program.source_type.is_module() {
    return Err(Exclusion::NonModule);
  }
  let mut barriers = ModuleBarriers::default();
  barriers.visit_program(program);
  if barriers.await_found {
    return Err(Exclusion::Await);
  }
  if barriers.eval_found {
    return Err(Exclusion::Eval);
  }
  Ok(())
}

struct FunctionReferences<'scope> {
  scoping: &'scope Scoping,
  function_scope: ScopeId,
  function_symbol: SymbolId,
  reason: Option<Exclusion>,
}

impl FunctionReferences<'_> {
  fn exclude(&mut self, reason: Exclusion) {
    self.reason.get_or_insert(reason);
  }
}

impl<'ast> VisitJs<'ast> for FunctionReferences<'_> {
  fn visit_identifier_reference(&mut self, it: &ast::IdentifierReference<'ast>) {
    let Some(reference) = it.reference_id.get() else {
      self.exclude(Exclusion::MissingSemanticBinding);
      return;
    };
    let Some(symbol) = self.scoping.get_reference(reference).symbol_id() else {
      self.exclude(Exclusion::UnresolvedBinding(it.name.to_string()));
      return;
    };
    if symbol == self.function_symbol {
      // A recursive declaration reads the mutable outer binding, not a private function name.
      self.exclude(Exclusion::SelfBinding);
    } else if !self
      .scoping
      .scope_ancestors(self.scoping.symbol_scope_id(symbol))
      .any(|scope| scope == self.function_scope)
    {
      self.exclude(Exclusion::CapturedBinding(it.name.to_string()));
    }
  }

  fn visit_expression(&mut self, it: &Expression<'ast>) {
    if matches!(it, Expression::ImportMeta(_) | Expression::NewTarget(_)) {
      self.exclude(Exclusion::ContextSensitiveExpression);
    }
    walk_js::walk_expression(self, it);
  }

  fn visit_this_expression(&mut self, _it: &ast::ThisExpression) {
    self.exclude(Exclusion::ContextSensitiveExpression);
  }

  fn visit_import_expression(&mut self, it: &ast::ImportExpression<'ast>) {
    self.exclude(Exclusion::ContextSensitiveExpression);
    walk_js::walk_import_expression(self, it);
  }
}

/// Only directly exported function declarations enter this first slice. Variables, classes,
/// default exports and forwarding declarations remain in their original native records.
fn collect_function_cells(program: &ast::Program<'_>, scoping: &Scoping) -> LocalCellPlan {
  let mut plan =
    LocalCellPlan { functions: Vec::new(), constants: Vec::new(), rejected: Vec::new() };
  for statement in &program.body {
    let Statement::ExportDeclaration(export) = statement else { continue };
    let Declaration::FunctionDeclaration(function) = &export.declaration else {
      continue;
    };
    let Some(id) = &function.id else { continue };
    let reason = if function.r#async || function.generator {
      Some(Exclusion::AsyncOrGenerator)
    } else if let (Some(symbol), Some(scope)) = (id.symbol_id.get(), function.scope_id.get()) {
      if scoping.get_resolved_references(symbol).any(oxc::semantic::Reference::is_write) {
        Some(Exclusion::WrittenBinding)
      } else {
        let mut references = FunctionReferences {
          scoping,
          function_scope: scope,
          function_symbol: symbol,
          reason: None,
        };
        references.visit_function(function, ScopeFlags::Function);
        references.reason
      }
    } else {
      Some(Exclusion::MissingSemanticBinding)
    };
    if let Some(reason) = reason {
      plan.rejected.push(RejectedFunction { name: id.name.to_string(), reason });
    } else {
      let symbol = id.symbol_id.get().expect("checked semantic binding");
      let mut exported_names = vec![id.name.to_string()];
      for statement in &program.body {
        if let Statement::ExportNamedDeclaration(named) = statement {
          for specifier in &named.specifiers {
            if specifier.local.name() == id.name {
              exported_names.push(specifier.exported.name().to_string());
            }
          }
        }
      }
      exported_names.sort();
      exported_names.dedup();
      plan.functions.push(FunctionCell {
        name: id.name.to_string(),
        symbol,
        declaration: function.span,
        export_statement: export.span,
        local_read_count: scoping
          .get_resolved_references(symbol)
          .filter(|reference| reference.is_read())
          .count(),
        exported_names,
      });
    }
  }
  plan
}
