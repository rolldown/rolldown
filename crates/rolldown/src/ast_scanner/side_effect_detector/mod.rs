use std::sync::Arc;

use once_cell::sync::Lazy;
use oxc::ast::ast::{
  BindingPatternKind, Expression, IdentifierReference, MemberExpression, PropertyKey,
};
use oxc::ast::Trivias;
use rolldown_common::AstScopes;
use rustc_hash::FxHashSet;

mod annotation;
mod utils;

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
  pub scope: &'a AstScopes,
  pub source: &'a Arc<str>,
  pub trivias: &'a Trivias,
}

impl<'a> SideEffectDetector<'a> {
  pub fn new(scope: &'a AstScopes, source: &'a Arc<str>, trivias: &'a Trivias) -> Self {
    Self { scope, source, trivias }
  }

  fn is_unresolved_reference(&mut self, ident_ref: &IdentifierReference) -> bool {
    self.scope.is_unresolved(ident_ref.reference_id.get().unwrap())
  }

  fn detect_side_effect_of_class(&mut self, cls: &oxc::ast::ast::Class) -> bool {
    use oxc::ast::ast::{ClassElement, PropertyKey};
    cls.body.body.iter().any(|elm| match elm {
      ClassElement::StaticBlock(static_block) => {
        static_block.body.iter().any(|stmt| self.detect_side_effect_of_stmt(stmt))
      }
      ClassElement::MethodDefinition(_) => false,
      ClassElement::PropertyDefinition(def) => {
        (match &def.key {
          // FIXME: this is wrong, we should also always check the `def.value`.
          PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => false,
          key @ oxc::ast::match_expression!(PropertyKey) => {
            self.detect_side_effect_of_expr(key.to_expression())
          }
        } || def.value.as_ref().is_some_and(|init| self.detect_side_effect_of_expr(init)))
      }
      ClassElement::AccessorProperty(def) => {
        (match &def.key {
          // FIXME: this is wrong, we should also always check the `def.value`.
          PropertyKey::StaticIdentifier(_) | PropertyKey::PrivateIdentifier(_) => false,
          key @ oxc::ast::match_expression!(PropertyKey) => {
            self.detect_side_effect_of_expr(key.to_expression())
          }
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
      Expression::Identifier(ident) => {
        let object_name = &ident.name;
        // Check if `object_name.prop_name` is pure
        !SIDE_EFFECT_FREE_MEMBER_EXPR_2.contains(&(object_name.as_str(), prop_name.as_str()))
      }
      expr @ oxc::ast::match_member_expression!(Expression) => {
        let mem_expr = expr.to_member_expression();
        let MemberExpression::StaticMemberExpression(mem_expr) = mem_expr else {
          return true;
        };
        let mid_prop = &mem_expr.property.name;
        let Expression::Identifier(obj_ident) = &mem_expr.object else {
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

  #[allow(clippy::too_many_lines)]
  fn detect_side_effect_of_expr(&mut self, expr: &oxc::ast::ast::Expression) -> bool {
    match expr {
      Expression::BooleanLiteral(_)
      | Expression::NullLiteral(_)
      | Expression::NumericLiteral(_)
      | Expression::BigIntLiteral(_)
      | Expression::RegExpLiteral(_)
      | Expression::FunctionExpression(_)
      | Expression::ArrowFunctionExpression(_)
      | Expression::MetaProperty(_)
      | Expression::ThisExpression(_)
      | Expression::StringLiteral(_) => false,
      Expression::ObjectExpression(obj_expr) => {
        obj_expr.properties.iter().any(|obj_prop| match obj_prop {
          oxc::ast::ast::ObjectPropertyKind::ObjectProperty(prop) => {
            let key_side_effect = match &prop.key {
              oxc::ast::ast::PropertyKey::StaticIdentifier(_)
              | oxc::ast::ast::PropertyKey::PrivateIdentifier(_) => false,
              key @ oxc::ast::match_expression!(PropertyKey) => {
                self.detect_side_effect_of_expr(key.to_expression())
              }
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
      oxc::ast::match_member_expression!(Expression) => {
        Self::detect_side_effect_of_member_expr(expr.to_member_expression())
      }
      Expression::ClassExpression(cls) => self.detect_side_effect_of_class(cls),
      // Accessing global variables considered as side effect.
      Expression::Identifier(ident) => self.is_unresolved_reference(ident),
      Expression::TemplateLiteral(literal) => {
        literal.expressions.iter().any(|expr| self.detect_side_effect_of_expr(expr))
      }
      Expression::LogicalExpression(logic_expr) => {
        self.detect_side_effect_of_expr(&logic_expr.left)
          || self.detect_side_effect_of_expr(&logic_expr.right)
      }
      Expression::ParenthesizedExpression(paren_expr) => {
        self.detect_side_effect_of_expr(&paren_expr.expression)
      }
      Expression::SequenceExpression(seq_expr) => {
        seq_expr.expressions.iter().any(|expr| self.detect_side_effect_of_expr(expr))
      }
      Expression::ConditionalExpression(cond_expr) => {
        self.detect_side_effect_of_expr(&cond_expr.test)
          || self.detect_side_effect_of_expr(&cond_expr.consequent)
          || self.detect_side_effect_of_expr(&cond_expr.alternate)
      }
      Expression::TSAsExpression(_)
      | Expression::TSSatisfiesExpression(_)
      | Expression::TSTypeAssertion(_)
      | Expression::TSNonNullExpression(_)
      | Expression::TSInstantiationExpression(_) => unreachable!("ts should be transpiled"),
      Expression::BinaryExpression(binary_expr) => {
        // For binary expressions, both sides could potentially have side effects
        self.detect_side_effect_of_expr(&binary_expr.left)
          || self.detect_side_effect_of_expr(&binary_expr.right)
      }
      Expression::PrivateInExpression(private_in_expr) => {
        self.detect_side_effect_of_expr(&private_in_expr.right)
      }
      // TODO: Implement these
      Expression::Super(_)
      | Expression::ArrayExpression(_)
      | Expression::AssignmentExpression(_)
      | Expression::AwaitExpression(_)
      | Expression::ChainExpression(_)
      | Expression::ImportExpression(_)
      | Expression::NewExpression(_)
      | Expression::TaggedTemplateExpression(_)
      | Expression::UpdateExpression(_)
      | Expression::YieldExpression(_)
      | Expression::JSXElement(_)
      | Expression::JSXFragment(_) => true,
      Expression::CallExpression(expr) => {
        let is_pure = self.is_pure_function_or_constructor_call(expr.span);
        if is_pure {
          expr.arguments.iter().any(|arg| match arg {
            oxc::ast::ast::Argument::SpreadElement(_) => true,
            // TODO: implement this
            _ => false,
          })
        } else {
          true
        }
      }
    }
  }

  fn detect_side_effect_of_var_decl(
    &mut self,
    var_decl: &oxc::ast::ast::VariableDeclaration,
  ) -> bool {
    var_decl.declarations.iter().any(|declarator| {
      // Whether to destructure import.meta
      if let BindingPatternKind::ObjectPattern(ref obj_pat) = declarator.id.kind {
        if !obj_pat.properties.is_empty() {
          if let Some(Expression::MetaProperty(_)) = declarator.init {
            return true;
          }
        }
      }

      declarator.init.as_ref().is_some_and(|init| self.detect_side_effect_of_expr(init))
    })
  }

  fn detect_side_effect_of_decl(&mut self, decl: &oxc::ast::ast::Declaration) -> bool {
    use oxc::ast::ast::Declaration;
    match decl {
      Declaration::VariableDeclaration(var_decl) => self.detect_side_effect_of_var_decl(var_decl),
      Declaration::FunctionDeclaration(_) => false,
      Declaration::ClassDeclaration(cls_decl) => self.detect_side_effect_of_class(cls_decl),
      // Currently, using a fallback value to make the bundle correct,
      // finishing the implementation after we carefully read the spec
      Declaration::UsingDeclaration(_) => true,
      Declaration::TSTypeAliasDeclaration(_)
      | Declaration::TSInterfaceDeclaration(_)
      | Declaration::TSEnumDeclaration(_)
      | Declaration::TSModuleDeclaration(_)
      | Declaration::TSImportEqualsDeclaration(_) => unreachable!("ts should be transpiled"),
    }
  }

  pub fn detect_side_effect_of_stmt(&mut self, stmt: &oxc::ast::ast::Statement) -> bool {
    use oxc::ast::ast::Statement;
    match stmt {
      oxc::ast::match_declaration!(Statement) => {
        self.detect_side_effect_of_decl(stmt.to_declaration())
      }
      Statement::ExpressionStatement(expr) => self.detect_side_effect_of_expr(&expr.expression),
      oxc::ast::match_module_declaration!(Statement) => match stmt.to_module_declaration() {
        oxc::ast::ast::ModuleDeclaration::ExportAllDeclaration(_) => true,
        oxc::ast::ast::ModuleDeclaration::ImportDeclaration(_) => {
          // We consider `import ...` has no side effect. However, `import ...` might be rewritten to other statements by the bundler.
          // In that case, we will mark the statement as having side effect in link stage.
          false
        }
        oxc::ast::ast::ModuleDeclaration::ExportDefaultDeclaration(default_decl) => {
          use oxc::ast::ast::ExportDefaultDeclarationKind;
          match &default_decl.declaration {
            decl @ oxc::ast::match_expression!(ExportDefaultDeclarationKind) => {
              self.detect_side_effect_of_expr(decl.to_expression())
            }
            oxc::ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(_) => false,
            oxc::ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(decl) => {
              self.detect_side_effect_of_class(decl)
            }
            oxc::ast::ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
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
      Statement::BlockStatement(block) => self.detect_side_effect_of_block(block),
      Statement::DoWhileStatement(do_while) => {
        self.detect_side_effect_of_stmt(&do_while.body)
          || self.detect_side_effect_of_expr(&do_while.test)
      }
      Statement::WhileStatement(while_stmt) => {
        self.detect_side_effect_of_expr(&while_stmt.test)
          || self.detect_side_effect_of_stmt(&while_stmt.body)
      }
      Statement::IfStatement(if_stmt) => {
        self.detect_side_effect_of_expr(&if_stmt.test)
          || self.detect_side_effect_of_stmt(&if_stmt.consequent)
          || if_stmt.alternate.as_ref().map_or(false, |stmt| self.detect_side_effect_of_stmt(stmt))
      }
      Statement::ReturnStatement(ret_stmt) => {
        ret_stmt.argument.as_ref().map_or(false, |expr| self.detect_side_effect_of_expr(expr))
      }
      Statement::LabeledStatement(labeled_stmt) => {
        self.detect_side_effect_of_stmt(&labeled_stmt.body)
      }
      Statement::TryStatement(try_stmt) => {
        self.detect_side_effect_of_block(&try_stmt.block)
          || try_stmt
            .handler
            .as_ref()
            .map_or(false, |handler| self.detect_side_effect_of_block(&handler.body))
          || try_stmt
            .finalizer
            .as_ref()
            .map_or(false, |finalizer| self.detect_side_effect_of_block(finalizer))
      }
      Statement::SwitchStatement(switch_stmt) => {
        self.detect_side_effect_of_expr(&switch_stmt.discriminant)
          || switch_stmt.cases.iter().any(|case| {
            case.test.as_ref().map_or(false, |expr| self.detect_side_effect_of_expr(expr))
              || case.consequent.iter().any(|stmt| self.detect_side_effect_of_stmt(stmt))
          })
      }
      Statement::EmptyStatement(_)
      | Statement::ContinueStatement(_)
      | Statement::BreakStatement(_) => false,
      // TODO: Implement these
      Statement::DebuggerStatement(_)
      | Statement::ForInStatement(_)
      | Statement::ForOfStatement(_)
      | Statement::ForStatement(_)
      | Statement::ThrowStatement(_)
      | Statement::WithStatement(_) => true,
    }
  }

  fn detect_side_effect_of_block(&mut self, block: &oxc::ast::ast::BlockStatement) -> bool {
    block.body.iter().any(|stmt| self.detect_side_effect_of_stmt(stmt))
  }
}

#[cfg(test)]
mod test {
  use oxc::span::SourceType;
  use rolldown_common::AstScopes;
  use rolldown_oxc_utils::{OxcAst, OxcCompiler};

  use crate::ast_scanner::side_effect_detector::SideEffectDetector;

  fn get_statements_side_effect(code: &str) -> bool {
    let source_type = SourceType::default()
      .with_always_strict(true)
      .with_module(true)
      .with_jsx(true)
      .with_typescript(false);
    let ast = OxcCompiler::parse(code, source_type).unwrap();
    let ast_scope = {
      let semantic = OxcAst::make_semantic(ast.source(), ast.program(), source_type);
      let (mut symbol_table, scope) = semantic.into_symbol_table_and_scope_tree();
      AstScopes::new(
        scope,
        std::mem::take(&mut symbol_table.references),
        std::mem::take(&mut symbol_table.resolved_references),
      )
    };

    let has_side_effect = ast.program().body.iter().any(|stmt| {
      SideEffectDetector::new(&ast_scope, ast.source(), &ast.trivias)
        .detect_side_effect_of_stmt(stmt)
    });

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

  #[test]
  fn test_logical_expression() {
    assert!(!get_statements_side_effect("true && false"));
    assert!(!get_statements_side_effect("null ?? true"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("true && bar"));
    assert!(get_statements_side_effect("foo ?? true"));
  }

  #[test]
  fn test_parenthesized_expression() {
    assert!(!get_statements_side_effect("(true)"));
    assert!(!get_statements_side_effect("(null)"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("(bar)"));
    assert!(get_statements_side_effect("(foo)"));
  }

  #[test]
  fn test_sequence_expression() {
    assert!(!get_statements_side_effect("true, false"));
    assert!(!get_statements_side_effect("null, true"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("true, bar"));
    assert!(get_statements_side_effect("foo, true"));
  }

  #[test]
  fn test_conditional_expression() {
    assert!(!get_statements_side_effect("true ? false : true"));
    assert!(!get_statements_side_effect("null ? true : false"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("true ? bar : true"));
    assert!(get_statements_side_effect("foo ? true : false"));
    assert!(get_statements_side_effect("true ? bar : true"));
  }

  #[test]
  fn test_block_statement() {
    assert!(!get_statements_side_effect("{ }"));
    assert!(!get_statements_side_effect("{ const a = 1; }"));
    assert!(!get_statements_side_effect("{ const a = 1; const b = 2; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("{ const a = 1; bar; }"));
  }

  #[test]
  fn test_do_while_statement() {
    assert!(!get_statements_side_effect("do { } while (true)"));
    assert!(!get_statements_side_effect("do { const a = 1; } while (true)"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("do { const a = 1; } while (bar)"));
    assert!(get_statements_side_effect("do { const a = 1; bar; } while (true)"));
    assert!(get_statements_side_effect("do { bar; } while (true)"));
  }

  #[test]
  fn test_while_statement() {
    assert!(!get_statements_side_effect("while (true) { }"));
    assert!(!get_statements_side_effect("while (true) { const a = 1; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("while (bar) { const a = 1; }"));
    assert!(get_statements_side_effect("while (true) { const a = 1; bar; }"));
    assert!(get_statements_side_effect("while (true) { bar; }"));
  }

  #[test]
  fn test_if_statement() {
    assert!(!get_statements_side_effect("if (true) { }"));
    assert!(!get_statements_side_effect("if (true) { const a = 1; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("if (bar) { const a = 1; }"));
    assert!(get_statements_side_effect("if (true) { const a = 1; bar; }"));
    assert!(get_statements_side_effect("if (true) { bar; }"));
  }

  #[test]
  fn test_empty_statement() {
    assert!(!get_statements_side_effect(";"));
    assert!(!get_statements_side_effect(";;"));
  }

  #[test]
  fn test_continue_statement() {
    assert!(!get_statements_side_effect("continue;"));
  }

  #[test]
  fn test_break_statement() {
    assert!(!get_statements_side_effect("break;"));
  }

  #[test]
  fn test_return_statement() {
    assert!(!get_statements_side_effect("return;"));
    assert!(!get_statements_side_effect("return 1;"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("return bar;"));
  }

  #[test]
  fn test_labeled_statement() {
    assert!(!get_statements_side_effect("label: { }"));
    assert!(!get_statements_side_effect("label: { const a = 1; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("label: { const a = 1; bar; }"));
    assert!(get_statements_side_effect("label: { bar; }"));
  }

  #[test]
  fn test_try_statement() {
    assert!(!get_statements_side_effect("try { } catch (e) { }"));
    assert!(!get_statements_side_effect("try { const a = 1; } catch (e) { }"));
    assert!(!get_statements_side_effect("try { } catch (e) { const a = 1; }"));
    assert!(!get_statements_side_effect("try { const a = 1; } catch (e) { const a = 1; }"));
    assert!(!get_statements_side_effect("try { const a = 1; } finally { }"));
    assert!(!get_statements_side_effect("try { } catch (e) { const a = 1; } finally { }"));
    assert!(!get_statements_side_effect("try { } catch (e) { } finally { const a = 1; }"));
    assert!(!get_statements_side_effect(
      "try { const a = 1; } catch (e) { const a = 1; } finally { const a = 1; }"
    ));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("try { const a = 1; bar; } catch (e) { }"));
    assert!(get_statements_side_effect("try { } catch (e) { const a = 1; bar; }"));
    assert!(get_statements_side_effect("try { } catch (e) { bar; }"));
    assert!(get_statements_side_effect("try { const a = 1; } catch (e) { bar; }"));
    assert!(get_statements_side_effect("try { bar; } finally { }"));
    assert!(get_statements_side_effect("try { } catch (e) { bar; } finally { }"));
    assert!(get_statements_side_effect("try { } catch (e) { } finally { bar; }"));
    assert!(get_statements_side_effect("try { bar; } catch (e) { bar; } finally { bar; }"));
  }

  #[test]
  fn test_switch_statement() {
    assert!(!get_statements_side_effect("switch (true) { }"));
    assert!(!get_statements_side_effect("switch (true) { case 1: break; }"));
    assert!(!get_statements_side_effect("switch (true) { case 1: break; default: break; }"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("switch (bar) { case 1: break; }"));
    assert!(get_statements_side_effect("switch (true) { case 1: bar; }"));
    assert!(get_statements_side_effect("switch (true) { case bar: break; }"));
    assert!(get_statements_side_effect("switch (true) { case 1: bar; default: bar; }"));
  }

  #[test]
  fn test_binary_expression() {
    assert!(!get_statements_side_effect("1 + 1"));
    assert!(!get_statements_side_effect("const a = 1; const b = 2; a + b"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("1 + foo"));
    assert!(get_statements_side_effect("2 + bar"));
  }

  #[test]
  fn test_private_in_expression() {
    assert!(!get_statements_side_effect("#privateField in this"));
    assert!(!get_statements_side_effect("const obj = {}; #privateField in obj"));
    // accessing global variable may have side effect
    assert!(get_statements_side_effect("#privateField in bar"));
    assert!(get_statements_side_effect("#privateField in foo"));
  }

  #[test]
  fn test_this_expression() {
    assert!(!get_statements_side_effect("this"));
    assert!(get_statements_side_effect("this.a"));
    assert!(get_statements_side_effect("this.a + this.b"));
    assert!(get_statements_side_effect("this.a = 10"));
  }

  #[test]
  fn test_meta_property_expression() {
    assert!(!get_statements_side_effect("import.meta"));
    assert!(!get_statements_side_effect("const meta = import.meta"));
    assert!(get_statements_side_effect("import.meta.url"));
    assert!(get_statements_side_effect("const { url } = import.meta"));
    assert!(get_statements_side_effect("import.meta.url = 'test'"));
  }
}
