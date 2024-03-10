use once_cell::sync::Lazy;
use oxc::ast::ast::{IdentifierReference, MemberExpression};
use rolldown_common::AstScope;
use rustc_hash::FxHashSet;

// Probably we should generate this using macros.
static SIDE_EFFECT_FREE_MEMBER_EXPR_2: Lazy<FxHashSet<(&'static str, &'static str)>> =
  Lazy::new(|| {
    [
      ("Object", "create"),
      ("Object", "defineProperty"),
      ("Object", "getOwnPropertyDescriptor"),
      ("Object", "getPrototypeOf"),
      ("Object", "getOwnPropertyNames"),
    ]
    .into_iter()
    .collect()
  });

static SIDE_EFFECT_FREE_MEMBER_EXPR_3: Lazy<FxHashSet<(&'static str, &'static str, &'static str)>> =
  Lazy::new(|| {
    [("Object", "prototype", "hasOwnProperty"), ("Object", "prototype", "constructor")]
      .into_iter()
      .collect()
  });

/// Detect if a statement "may" have side effect.
pub struct SideEffectDetector<'a> {
  pub scope: &'a AstScope,
}

impl<'a> SideEffectDetector<'a> {
  pub fn new(scope: &'a AstScope) -> Self {
    Self { scope }
  }

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
      ClassElement::TSIndexSignature(_) => unreachable!("ts should be transpiled"),
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
        !SIDE_EFFECT_FREE_MEMBER_EXPR_2.contains(&(object_name.as_str(), prop_name.as_str()))
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
          object_name.as_str(),
          mid_prop.as_str(),
          prop_name.as_str(),
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
      Expression::ObjectExpression(obj_expr) => {
        obj_expr.properties.iter().any(|obj_prop| match obj_prop {
          oxc::ast::ast::ObjectPropertyKind::ObjectProperty(prop) => {
            let key_side_effect = match &prop.key {
              oxc::ast::ast::PropertyKey::Identifier(_)
              | oxc::ast::ast::PropertyKey::PrivateIdentifier(_) => false,
              oxc::ast::ast::PropertyKey::Expression(expr) => self.detect_side_effect_of_expr(expr),
            };

            let prop_init_side_effect =
              prop.init.as_ref().map_or(false, |expr| self.detect_side_effect_of_expr(expr));

            let value_side_effect = self.detect_side_effect_of_expr(&prop.value);

            key_side_effect || prop_init_side_effect || value_side_effect
          }
          oxc::ast::ast::ObjectPropertyKind::SpreadProperty(_) => {
            // ...[expression] is considered as having side effect.
            // see crates/rolldown/tests/fixtures/rollup/object-spread-side-effect
            true
          }
        })
      }
      Expression::UnaryExpression(unary_expr) => {
        self.detect_side_effect_of_expr(&unary_expr.argument)
      }
      Expression::MemberExpression(mem_expr) => Self::detect_side_effect_of_member_expr(mem_expr),
      Expression::ClassExpression(cls) => self.detect_side_effect_of_class(cls),
      // Accessing global variables considered as side effect.
      Expression::Identifier(ident) => self.is_unresolved_reference(ident),
      Expression::TemplateLiteral(literal) => {
        literal.expressions.iter().any(|expr| self.detect_side_effect_of_expr(expr))
      }
      Expression::TSAsExpression(_)
      | Expression::TSSatisfiesExpression(_)
      | Expression::TSTypeAssertion(_)
      | Expression::TSNonNullExpression(_)
      | Expression::TSInstantiationExpression(_) => unreachable!("ts should be transpiled"),

      // TODO: Implement these
      Expression::MetaProperty(_) => true,
      Expression::Super(_) => true,

      Expression::ArrayExpression(_) => true,
      Expression::AssignmentExpression(_) => true,
      Expression::AwaitExpression(_) => true,
      Expression::BinaryExpression(_) => true,
      Expression::CallExpression(_) => true,
      Expression::ChainExpression(_) => true,
      Expression::ConditionalExpression(_) => true,
      Expression::ImportExpression(_) => true,
      Expression::LogicalExpression(logic_expr) => {
        self.detect_side_effect_of_expr(&logic_expr.left)
          || self.detect_side_effect_of_expr(&logic_expr.right)
      }
      Expression::NewExpression(_) => true,
      Expression::ParenthesizedExpression(paren_expr) => {
        self.detect_side_effect_of_expr(&paren_expr.expression)
      }
      Expression::SequenceExpression(seq_expr) => {
        seq_expr.expressions.iter().any(|expr| self.detect_side_effect_of_expr(expr))
      }
      Expression::TaggedTemplateExpression(_) => true,
      Expression::ThisExpression(_) => true,
      Expression::UpdateExpression(_) => true,
      Expression::YieldExpression(_) => true,
      Expression::PrivateInExpression(_) => true,

      Expression::JSXElement(_) => true,
      Expression::JSXFragment(_) => true,
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

#[cfg(test)]
mod test {
  use oxc::span::SourceType;
  use rolldown_common::AstScope;
  use rolldown_oxc_utils::OxcCompiler;

  use crate::ast_scanner::side_effect_detector::SideEffectDetector;

  fn get_statements_side_effect(code: &str) -> bool {
    let source_type = SourceType::default()
      .with_always_strict(true)
      .with_module(true)
      .with_jsx(true)
      .with_typescript(false);
    let program = OxcCompiler::parse(code, source_type);

    let ast_scope = {
      let semantic = program.make_semantic(source_type);
      let (mut symbol_table, scope) = semantic.into_symbol_table_and_scope_tree();
      AstScope::new(scope, std::mem::take(&mut symbol_table.references))
    };

    let has_side_effect = program
      .program()
      .body
      .iter()
      .any(|stmt| SideEffectDetector::new(&ast_scope).detect_side_effect_of_stmt(stmt));

    has_side_effect
  }

  #[test]
  fn test_side_effect() {
    assert!(!get_statements_side_effect("export { a }"));
    assert!(!get_statements_side_effect("const a = {}"));
    assert!(!get_statements_side_effect(
      "const PatchFlags = {
        'TEXT':1,
        '1':'TEXT',
        'CLASS':2,
        '2':'CLASS',
        'STYLE':4,
        '4':'STYLE',
        'PROPS':8,
        '8':'PROPS',
        'FULL_PROPS':16,
        '16':'FULL_PROPS',
        'NEED_HYDRATION':32,
        '32':'NEED_HYDRATION',
        'STABLE_FRAGMENT':64,
        '64':'STABLE_FRAGMENT',
        'KEYED_FRAGMENT':128,
        '128':'KEYED_FRAGMENT',
        'UNKEYED_FRAGMENT':256,
        '256':'UNKEYED_FRAGMENT',
        'NEED_PATCH':512,
        '512':'NEED_PATCH',
        'DYNAMIC_SLOTS':1024,
        '1024':'DYNAMIC_SLOTS',
        'DEV_ROOT_FRAGMENT':2048,
        '2048':'DEV_ROOT_FRAGMENT',
        'HOISTED': -1,
        '-1':'HOISTED',
        'BAIL': -2,
        '-2':'BAIL'
      };",
    ));
  }

  #[test]
  fn test_template_literal() {
    assert!(!get_statements_side_effect("`hello`"));
    assert!(!get_statements_side_effect("const foo = ''; `hello${foo}`"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("`hello${foo}`"));
    assert!(get_statements_side_effect("const foo = {}; `hello${foo.bar}`"));
    assert!(get_statements_side_effect("tag`hello`"));
  }
}
