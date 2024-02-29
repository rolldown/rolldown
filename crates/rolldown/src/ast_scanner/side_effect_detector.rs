use std::borrow::Cow;

use oxc::{
  ast::ast::{IdentifierReference, MemberExpression},
  span::Atom,
};
use rolldown_common::AstScope;
use rustc_hash::FxHashSet;

// Probably we should generate this using macros.
static SIDE_EFFECT_FREE_MEMBER_EXPR_2: once_cell::sync::Lazy<
  FxHashSet<(Cow<'static, Atom>, Cow<'static, Atom>)>,
> = once_cell::sync::Lazy::new(|| {
  [
    ("Object", "create"),
    ("Object", "defineProperty"),
    ("Object", "getOwnPropertyDescriptor"),
    ("Object", "getPrototypeOf"),
    ("Object", "getOwnPropertyNames"),
  ]
  .into_iter()
  .map(|(obj, prop)| (Cow::Owned(obj.into()), Cow::Owned(prop.into())))
  .collect()
});

// hyf0: clippy::type_complexity: This is only a temporary solution.
#[allow(clippy::type_complexity)]
static SIDE_EFFECT_FREE_MEMBER_EXPR_3: once_cell::sync::Lazy<
  FxHashSet<(Cow<'static, Atom>, Cow<'static, Atom>, Cow<'static, Atom>)>,
> = once_cell::sync::Lazy::new(|| {
  [("Object", "prototype", "hasOwnProperty"), ("Object", "prototype", "constructor")]
    .into_iter()
    .map(|(obj, obj_mid, prop)| {
      (Cow::Owned(obj.into()), Cow::Owned(obj_mid.into()), Cow::Owned(prop.into()))
    })
    .collect()
});

pub struct SideEffectDetector<'a> {
  pub scope: &'a AstScope,
}

impl<'a> SideEffectDetector<'a> {
  fn is_unresolved_reference(&self, ident_ref: &IdentifierReference) -> bool {
    self.scope.is_unresolved(ident_ref.reference_id.get().unwrap())
  }

  fn detect_side_effect_of_class(&self, cls: &oxc::ast::ast::Class) -> bool {
    use oxc::ast::ast::ClassElement;
    cls.body.body.iter().any(|elm| match elm {
      ClassElement::StaticBlock(static_block) => {
        static_block.body.iter().any(|stmt| self.detect_side_effect_of_stmt(stmt))
      }
      ClassElement::MethodDefinition(_) => false,
      ClassElement::PropertyDefinition(def) => {
        (match &def.key {
          // FIXME: this is wrong, we should also always check the `def.value`.
          oxc::ast::ast::PropertyKey::Identifier(_)
          | oxc::ast::ast::PropertyKey::PrivateIdentifier(_) => false,
          oxc::ast::ast::PropertyKey::Expression(expr) => self.detect_side_effect_of_expr(expr),
        } || def.value.as_ref().is_some_and(|init| self.detect_side_effect_of_expr(init)))
      }
      ClassElement::AccessorProperty(def) => {
        (match &def.key {
          // FIXME: this is wrong, we should also always check the `def.value`.
          oxc::ast::ast::PropertyKey::Identifier(_)
          | oxc::ast::ast::PropertyKey::PrivateIdentifier(_) => false,
          oxc::ast::ast::PropertyKey::Expression(expr) => self.detect_side_effect_of_expr(expr),
        } || def.value.as_ref().is_some_and(|init| self.detect_side_effect_of_expr(init)))
      }
      ClassElement::TSAbstractMethodDefinition(_)
      | ClassElement::TSAbstractPropertyDefinition(_)
      | ClassElement::TSIndexSignature(_) => unreachable!("ts should be transpiled"),
    })
  }

  fn detect_side_effect_of_member_expr(expr: &oxc::ast::ast::MemberExpression) -> bool {
    // MemberExpression is considered having side effect by default, unless it's some builtin global variables.
    let MemberExpression::StaticMemberExpression(member_expr) = expr else {
      return true;
    };
    let prop_name = &member_expr.property.name;
    match &member_expr.object {
      oxc::ast::ast::Expression::Identifier(ident) => {
        let object_name = &ident.name;
        // Check if `object_name.prop_name` is pure
        !SIDE_EFFECT_FREE_MEMBER_EXPR_2
          .contains(&(Cow::Borrowed(object_name), Cow::Borrowed(prop_name)))
      }
      oxc::ast::ast::Expression::MemberExpression(mem_expr) => {
        let MemberExpression::StaticMemberExpression(mem_expr) = &**mem_expr else {
          return true;
        };
        let mid_prop = &mem_expr.property.name;
        let oxc::ast::ast::Expression::Identifier(obj_ident) = &mem_expr.object else {
          return true;
        };
        let object_name = &obj_ident.name;
        // Check if `object_name.mid_prop.prop_name` is pure
        !SIDE_EFFECT_FREE_MEMBER_EXPR_3.contains(&(
          Cow::Borrowed(object_name),
          Cow::Borrowed(mid_prop),
          Cow::Borrowed(prop_name),
        ))
      }
      _ => true,
    }
  }

  fn detect_side_effect_of_expr(&self, expr: &oxc::ast::ast::Expression) -> bool {
    use oxc::ast::ast::Expression;
    match expr {
      Expression::BooleanLiteral(_)
      | Expression::NullLiteral(_)
      | Expression::NumericLiteral(_)
      | Expression::BigintLiteral(_)
      | Expression::RegExpLiteral(_)
      | Expression::FunctionExpression(_)
      | Expression::ArrowFunctionExpression(_)
      | Expression::StringLiteral(_) => false,
      Expression::MemberExpression(mem_expr) => Self::detect_side_effect_of_member_expr(mem_expr),
      Expression::ClassExpression(cls) => self.detect_side_effect_of_class(cls),
      // Accessing global variables considered as side effect.
      Expression::Identifier(ident) => self.is_unresolved_reference(ident),
      _ => true,
    }
  }

  fn detect_side_effect_of_decl(&self, decl: &oxc::ast::ast::Declaration) -> bool {
    use oxc::ast::ast::Declaration;
    match decl {
      Declaration::VariableDeclaration(var_decl) => var_decl
        .declarations
        .iter()
        .any(|decl| decl.init.as_ref().is_some_and(|init| self.detect_side_effect_of_expr(init))),
      Declaration::FunctionDeclaration(_) => false,
      Declaration::ClassDeclaration(cls_decl) => self.detect_side_effect_of_class(cls_decl),
      Declaration::UsingDeclaration(_) => todo!(),
      Declaration::TSTypeAliasDeclaration(_)
      | Declaration::TSInterfaceDeclaration(_)
      | Declaration::TSEnumDeclaration(_)
      | Declaration::TSModuleDeclaration(_)
      | Declaration::TSImportEqualsDeclaration(_) => unreachable!("ts should be transpiled"),
    }
  }

  pub fn detect_side_effect_of_stmt(&self, stmt: &oxc::ast::ast::Statement) -> bool {
    use oxc::ast::ast::Statement;
    match stmt {
      Statement::Declaration(decl) => self.detect_side_effect_of_decl(decl),
      Statement::ExpressionStatement(expr) => self.detect_side_effect_of_expr(&expr.expression),
      Statement::ModuleDeclaration(module_decl) => match &**module_decl {
        oxc::ast::ast::ModuleDeclaration::ImportDeclaration(_)
        | oxc::ast::ast::ModuleDeclaration::ExportAllDeclaration(_) => true,
        oxc::ast::ast::ModuleDeclaration::ExportDefaultDeclaration(default_decl) => {
          match &default_decl.declaration {
            oxc::ast::ast::ExportDefaultDeclarationKind::Expression(expr) => {
              self.detect_side_effect_of_expr(expr)
            }
            oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(_) => false,
            oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
              self.detect_side_effect_of_class(decl)
            }
            oxc::ast::ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
            | oxc::ast::ast::ExportDefaultDeclarationKind::TSEnumDeclaration(_) => {
              unreachable!("ts should be transpiled")
            }
          }
        }
        oxc::ast::ast::ModuleDeclaration::ExportNamedDeclaration(named_decl) => {
          if named_decl.source.is_some() {
            // `export { ... } from '...'` is considered as side effect.
            true
          } else {
            named_decl
              .declaration
              .as_ref()
              .map_or(false, |decl| self.detect_side_effect_of_decl(decl))
          }
        }
        oxc::ast::ast::ModuleDeclaration::TSExportAssignment(_)
        | oxc::ast::ast::ModuleDeclaration::TSNamespaceExportDeclaration(_) => {
          unreachable!("ts should be transpiled")
        }
      },
      Statement::BlockStatement(_)
      | Statement::BreakStatement(_)
      | Statement::DebuggerStatement(_)
      | Statement::DoWhileStatement(_)
      | Statement::EmptyStatement(_)
      | Statement::ForInStatement(_)
      | Statement::ForOfStatement(_)
      | Statement::ForStatement(_)
      | Statement::IfStatement(_)
      | Statement::LabeledStatement(_)
      | Statement::ReturnStatement(_)
      | Statement::SwitchStatement(_)
      | Statement::ThrowStatement(_)
      | Statement::TryStatement(_)
      | Statement::WhileStatement(_)
      | Statement::WithStatement(_)
      | Statement::ContinueStatement(_) => true,
    }
  }
}
