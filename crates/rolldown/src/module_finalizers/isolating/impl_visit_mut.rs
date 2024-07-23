use oxc::ast::ast::{self, Statement};
use oxc::ast::VisitMut;
use rolldown_common::Module;
use rolldown_ecmascript::TakeIn;

use super::IsolatingModuleFinalizer;

impl<'me, 'ast> VisitMut<'ast> for IsolatingModuleFinalizer<'me, 'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    let original_body = program.body.take_in(self.alloc);

    for stmt in original_body {
      match &stmt {
        // // rewrite:
        // - `import { default, a, b as b2 } from 'xxx'` to `const { default, a, b: b2 } = __static_import('xxx')`
        // - `import foo from 'xxx'` to `const { default: foo } = __static_import('xxx')`
        // - `import * as star from 'xxx'` to `const star = __static_import_star('xxx')`
        Statement::ImportDeclaration(import_decl) => {
          let rec_id = self.ctx.module.imports[&import_decl.span];
          let rec = &self.ctx.module.import_records[rec_id];
          let mut named_specifiers = vec![];
          let mut star_specifier = None;
          match &self.ctx.modules[rec.resolved_module] {
            Module::Ecma(importee) => {
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
              let importee = &self.ctx.modules[importee.idx];
              if is_plain_import {
                program.body.push(
                  self
                    .snippet
                    .app_static_import_call_multiple_specifiers_stmt(&[], importee.stable_id()),
                );
                continue;
              } else if let Some(star_spec) = star_specifier {
                program.body.push(
                  self
                    .snippet
                    .app_static_import_star_call_stmt(&star_spec.local.name, importee.stable_id()),
                );
                continue;
              }
              program.body.push(self.snippet.app_static_import_call_multiple_specifiers_stmt(
                &named_specifiers,
                importee.stable_id(),
              ));
              continue;
            }
            Module::External(_) => unimplemented!(),
          }
        }
        // TODO: rewrite `export default xxx` to `var __rolldown_default_export__ = xxx`
        ast::Statement::ExportDefaultDeclaration(_default_decl) => {}
        _ => {}
      }
      program.body.push(stmt);
    }
  }
}
