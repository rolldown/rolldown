use oxc::ast::ast::{
  ArrayExpressionElement, BindingPattern, Class, ClassElement, Declaration,
  ExportDefaultDeclarationKind, Expression, Function, ObjectPropertyKind, Program, Statement,
  VariableDeclaration, VariableDeclarationKind,
};
use oxc::semantic::SymbolId;
use oxc::syntax::operator::{BinaryOperator, UnaryOperator};
use rolldown_common::AstScopes;
use rustc_hash::FxHashSet;

/// Prove that the local body can initialize at its first binding consumer. Static dependencies
/// need a separate graph-wide proof. See internal-docs/code-splitting/design.md.
pub(super) fn has_independent_initialization(
  program: &Program<'_>,
  scopes: &AstScopes,
  keep_names: bool,
) -> bool {
  let mut analyzer = IndependentInit { scopes, initialized: FxHashSet::default(), keep_names };
  for stmt in &program.body {
    let function = match stmt {
      Statement::FunctionDeclaration(function) => Some(function.as_ref()),
      Statement::ExportDeclaration(export) => match &export.declaration {
        Declaration::FunctionDeclaration(function) => Some(function.as_ref()),
        _ => None,
      },
      Statement::ExportDefaultDeclaration(export) => match &export.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => Some(function.as_ref()),
        _ => None,
      },
      _ => None,
    };
    if let Some(function) = function {
      analyzer.initialize_function(function);
    }
  }
  program.body.iter().all(|stmt| analyzer.statement(stmt))
}

struct IndependentInit<'a> {
  scopes: &'a AstScopes,
  initialized: FxHashSet<SymbolId>,
  keep_names: bool,
}

impl IndependentInit<'_> {
  fn initialize_function(&mut self, function: &Function<'_>) {
    if let Some(symbol_id) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
      self.initialized.insert(symbol_id);
    }
  }

  fn statement(&mut self, stmt: &Statement<'_>) -> bool {
    match stmt {
      Statement::ImportDeclaration(_)
      | Statement::ExportAllDeclaration(_)
      | Statement::ExportFromDeclaration(_)
      | Statement::ExportNamedDeclaration(_)
      | Statement::EmptyStatement(_) => true,
      Statement::FunctionDeclaration(_) => !self.keep_names,
      Statement::VariableDeclaration(declaration) => self.variables(declaration),
      Statement::ClassDeclaration(class) => self.class_declaration(class),
      Statement::ExportDeclaration(export) => match &export.declaration {
        Declaration::FunctionDeclaration(_) => !self.keep_names,
        Declaration::VariableDeclaration(declaration) => self.variables(declaration),
        Declaration::ClassDeclaration(class) => self.class_declaration(class),
        _ => false,
      },
      Statement::ExportDefaultDeclaration(export) => match &export.declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(_) => !self.keep_names,
        ExportDefaultDeclarationKind::ClassDeclaration(class) => self.class_declaration(class),
        expression => expression.as_expression().is_some_and(|expr| self.expression(expr)),
      },
      _ => false,
    }
  }

  fn variables(&mut self, declaration: &VariableDeclaration<'_>) -> bool {
    if !matches!(
      declaration.kind,
      VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const
    ) {
      return false;
    }
    declaration.declarations.iter().all(|declarator| {
      let BindingPattern::BindingIdentifier(id) = &declarator.id else { return false };
      if declarator.init.as_ref().is_some_and(|init| !self.expression(init)) {
        return false;
      }
      let Some(symbol_id) = id.symbol_id.get() else { return false };
      self.initialized.insert(symbol_id);
      true
    })
  }

  fn class_declaration(&mut self, class: &Class<'_>) -> bool {
    if !self.class(class) {
      return false;
    }
    if let Some(symbol_id) = class.id.as_ref().and_then(|id| id.symbol_id.get()) {
      self.initialized.insert(symbol_id);
    }
    true
  }

  fn class(&self, class: &Class<'_>) -> bool {
    !self.keep_names
      && class.heritage.is_none()
      && class.decorators.is_empty()
      && class.body.body.iter().all(|element| match element {
        ClassElement::MethodDefinition(method) => !method.computed && method.decorators.is_empty(),
        ClassElement::PropertyDefinition(property) => {
          !property.computed && !property.r#static && property.decorators.is_empty()
        }
        ClassElement::AccessorProperty(property) => {
          !property.computed && !property.r#static && property.decorators.is_empty()
        }
        _ => false,
      })
  }

  fn expression(&self, expression: &Expression<'_>) -> bool {
    match expression.get_inner_expression() {
      Expression::BooleanLiteral(_)
      | Expression::NullLiteral(_)
      | Expression::NumericLiteral(_)
      | Expression::BigIntLiteral(_)
      | Expression::StringLiteral(_)
      | Expression::RegExpLiteral(_) => true,
      Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => {
        !self.keep_names
      }
      Expression::ClassExpression(class) => self.class(class),
      Expression::Identifier(identifier) => identifier
        .reference_id
        .get()
        .and_then(|reference_id| self.scopes.symbol_id_for(reference_id))
        .is_some_and(|symbol_id| self.initialized.contains(&symbol_id)),
      Expression::ObjectExpression(object) => object.properties.iter().all(|property| {
        matches!(property, ObjectPropertyKind::ObjectProperty(property)
          if !property.computed && self.expression(&property.value))
      }),
      Expression::ArrayExpression(array) => array.elements.iter().all(|element| match element {
        ArrayExpressionElement::SpreadElement(_) => false,
        ArrayExpressionElement::Elision(_) => true,
        element => self.expression(element.to_expression()),
      }),
      Expression::TemplateLiteral(template) => template.expressions.is_empty(),
      Expression::LogicalExpression(logical) => {
        self.expression(&logical.left) && self.expression(&logical.right)
      }
      Expression::ConditionalExpression(conditional) => {
        self.expression(&conditional.test)
          && self.expression(&conditional.consequent)
          && self.expression(&conditional.alternate)
      }
      Expression::SequenceExpression(sequence) => {
        sequence.expressions.iter().all(|expression| self.expression(expression))
      }
      Expression::UnaryExpression(unary) => match unary.operator {
        UnaryOperator::Void | UnaryOperator::LogicalNot | UnaryOperator::Typeof => {
          self.expression(&unary.argument)
        }
        UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => matches!(
          unary.argument.get_inner_expression(),
          Expression::NumericLiteral(_) | Expression::BigIntLiteral(_)
        ),
        UnaryOperator::UnaryPlus => {
          matches!(unary.argument.get_inner_expression(), Expression::NumericLiteral(_))
        }
        UnaryOperator::Delete => false,
      },
      Expression::BinaryExpression(binary) => {
        matches!(binary.operator, BinaryOperator::StrictEquality | BinaryOperator::StrictInequality)
          && self.expression(&binary.left)
          && self.expression(&binary.right)
      }
      _ => false,
    }
  }
}
