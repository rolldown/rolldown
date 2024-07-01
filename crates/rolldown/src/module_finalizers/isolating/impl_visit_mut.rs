use itertools::Itertools;
use oxc::ast::ast::{self, ExportDefaultDeclarationKind, Statement};
use oxc::ast::VisitMut;
use oxc::span::SPAN;
use rolldown_common::ExportsKind;
use rolldown_oxc_utils::quote::quote_expr;
use rolldown_oxc_utils::{AllocatorExt, IntoIn, TakeIn};

use super::IsolatingModuleFinalizer;

impl<'me, 'ast> VisitMut<'ast> for IsolatingModuleFinalizer<'me, 'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    let original_body = program.body.take_in(self.alloc);

    for mut stmt in original_body {
      match &mut stmt {
        // // rewrite:
        // - `import { default, a, b as b2 } from 'xxx'` to `const { default, a, b: b2 } = __static_import('xxx')`
        // - `import foo from 'xxx'` to `const { default: foo } = __static_import('xxx')`
        // - `import * as star from 'xxx'` to `const star = __static_import_star('xxx')`
        Statement::ImportDeclaration(import_decl) => {
          let rec_id = self.ctx.module.imports[&import_decl.span];
          let rec = &self.ctx.module.import_records[rec_id];
          let mut named_specifiers = vec![];
          let mut star_specifier = None;
          match rec.resolved_module {
            rolldown_common::ModuleId::Normal(importee_id) => {
              if let Some(specifiers) = &import_decl.specifiers {
                for specifier in specifiers {
                  match specifier {
                    ast::ImportDeclarationSpecifier::ImportSpecifier(s) => {
                      named_specifiers.push((s.imported.name().as_str(), s.local.name.as_str()));
                    }
                    ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                      named_specifiers.push(("default", s.local.name.as_str()));
                    }
                    ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                      star_specifier = Some(s);
                    }
                  }
                }
              }
              let is_plain_import =
                import_decl.specifiers.as_ref().map_or(false, |specifiers| specifiers.is_empty());
              let importee = &self.ctx.modules[importee_id];
              if is_plain_import {
                program.body.push(self.snippet.app_static_import_call_multiple_specifiers_stmt(
                  &[],
                  &importee.stable_resource_id,
                ));
                continue;
              } else if let Some(star_spec) = star_specifier {
                program.body.push(self.snippet.app_static_import_star_call_stmt(
                  &star_spec.local.name,
                  &importee.stable_resource_id,
                ));
                continue;
              }
              program.body.push(self.snippet.app_static_import_call_multiple_specifiers_stmt(
                &named_specifiers,
                &importee.stable_resource_id,
              ));
              continue;
            }
            rolldown_common::ModuleId::External(_) => unimplemented!(),
          }
        }
        // TODO: rewrite `export default xxx` to `var __rolldown_default_export__ = xxx`
        ast::Statement::ExportDefaultDeclaration(default_decl) => {
          program.body.push(ast::Statement::VariableDeclaration(self.alloc.boxed(
            ast::VariableDeclaration {
              kind: ast::VariableDeclarationKind::Var,
              declarations: self.alloc.new_vec_with(ast::VariableDeclarator {
                id: ast::BindingPattern::new_with_kind(ast::BindingPatternKind::BindingIdentifier(
                  self.alloc.boxed(ast::BindingIdentifier {
                    name: "__rolldown_default_export__".into(),
                    ..TakeIn::dummy(self.alloc)
                  }),
                )),
                init: Some(match &mut default_decl.declaration {
                  ast::ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => unreachable!(),
                  ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                    ast::Expression::FunctionExpression(self.alloc.take(func))
                  }
                  ast::ExportDefaultDeclarationKind::ClassDeclaration(cls) => {
                    ast::Expression::ClassExpression(self.alloc.take(cls))
                  }
                  decl @ ast::match_expression!(ExportDefaultDeclarationKind) => {
                    let expr = decl.to_expression_mut();
                    self.alloc.take(expr)
                  }
                }),
                ..TakeIn::dummy(self.alloc)
              }),
              ..TakeIn::dummy(self.alloc)
            },
          )));
          continue;
        }
        _ => {}
      }
      program.body.push(stmt);
    }

    if matches!(self.ctx.module.exports_kind, ExportsKind::Esm) {
      let exports = self
        .ctx
        .module
        .named_exports
        .iter()
        .map(|(exported_name, export)| {
          (exported_name, self.ctx.symbols.get_original_name(export.referenced))
        })
        .map(|(exported_name, local_name)| {
          if exported_name.as_str() == "default" {
            format!("{exported_name}: __rolldown_default_export__")
          } else {
            format!("{exported_name}: {local_name}")
          }
        })
        .join(", ");
      let expr = quote_expr(&self.alloc, &format!("module.exports = {{ {} }}", exports));
      program.body.push(ast::Statement::ExpressionStatement(
        self
          .alloc
          .boxed(ast::ExpressionStatement { expression: expr, ..TakeIn::dummy(self.alloc) }),
      ));
    }

    // (module, exports) => { ... }
    let mut wrapper_fn = ast::ArrowFunctionExpression {
      params: self.alloc.boxed(ast::FormalParameters {
        items: self.alloc.new_vec_with2(
          ast::FormalParameter {
            pattern: ast::BindingPattern::new_with_kind(
              ast::BindingPatternKind::BindingIdentifier(self.alloc.boxed(
                ast::BindingIdentifier {
                  name: self.alloc.atom("module"),
                  ..TakeIn::dummy(self.alloc)
                },
              )),
            ),
            ..TakeIn::dummy(self.alloc)
          },
          ast::FormalParameter {
            pattern: ast::BindingPattern::new_with_kind(
              ast::BindingPatternKind::BindingIdentifier(self.alloc.boxed(
                ast::BindingIdentifier {
                  name: self.alloc.atom("exports"),
                  ..TakeIn::dummy(self.alloc)
                },
              )),
            ),
            ..TakeIn::dummy(self.alloc)
          },
        ),
        ..TakeIn::dummy(self.alloc)
      }),
      ..TakeIn::dummy(self.alloc)
    };
    wrapper_fn.body = ast::FunctionBody {
      statements: self.alloc.take(&mut program.body),
      ..TakeIn::dummy(self.alloc)
    }
    .into_in(&self.alloc);

    // wrap with `__rolldown_define__('id', (module, exports, require) => { ... })`
    let arguments = self.alloc.new_vec_with2(
      ast::Argument::StringLiteral(self.alloc.boxed(ast::StringLiteral::new(
        SPAN,
        self.alloc.atom(&self.ctx.module.stable_resource_id),
      ))),
      ast::Argument::ArrowFunctionExpression(self.alloc.boxed(wrapper_fn)),
    );
    let define_call_expr = ast::Expression::CallExpression(
      self.alloc.boxed(ast::CallExpression {
        callee: ast::Expression::Identifier(
          self
            .alloc
            .boxed(ast::IdentifierReference::new(SPAN, self.alloc.atom("__rolldown_define__"))),
        ),
        arguments,
        ..TakeIn::dummy(self.alloc)
      }),
    );

    program.body.push(ast::Statement::ExpressionStatement(self.alloc.boxed(
      ast::ExpressionStatement { expression: define_call_expr, ..TakeIn::dummy(self.alloc) },
    )));
  }
}
