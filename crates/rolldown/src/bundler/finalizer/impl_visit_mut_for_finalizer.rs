use oxc::{
  allocator::{self},
  ast::{
    ast::{self},
    VisitMut,
  },
};
use rolldown_common::{ExportsKind, SymbolRef, WrapKind};
use rolldown_oxc::{Dummy, ExpressionExt, IntoIn, StatementExt, TakeIn};

use crate::bundler::module::Module;

use super::Finalizer;

impl<'ast, 'me: 'ast> Finalizer<'me, 'ast> {
  fn visit_top_level_statement_mut(&mut self, stmt: &mut ast::Statement<'ast>) {
    self.visit_statement(stmt);
  }
}

impl<'ast, 'me: 'ast> VisitMut<'ast> for Finalizer<'me, 'ast> {
  #[allow(clippy::too_many_lines)]
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    for directive in program.directives.iter_mut() {
      self.visit_directive(directive);
    }

    let old_body = program.body.take_in(self.alloc);
    let is_namespace_referenced = matches!(self.ctx.module.exports_kind, ExportsKind::Esm)
      && self.ctx.module.id != self.ctx.runtime.id();
    if is_namespace_referenced {
      program.body.extend(self.generate_namespace_variable_declaration());
    }

    old_body.into_iter().for_each(|mut top_stmt| {
      if let Some(import_decl) = top_stmt.as_import_declaration() {
        let rec_id = self.ctx.module.imports[&import_decl.span];
        if self.should_remove_import_export_stmt(&mut top_stmt, rec_id) {
          return;
        }
      } else if let Some(export_all_decl) = top_stmt.as_export_all_declaration() {
        let rec_id = self.ctx.module.imports[&export_all_decl.span];
        // "export * as ns from 'path'"
        if let Some(_alias) = &export_all_decl.exported {
          if self.should_remove_import_export_stmt(&mut top_stmt, rec_id) {
            return;
          }
        } else {
          // "export * from 'path'"
          let rec = &self.ctx.module.import_records[rec_id];
          let importee_id = rec.resolved_module;
          let importee_linking_info = &self.ctx.linking_infos[importee_id];
          let importee = &self.ctx.modules[importee_id];
          if matches!(importee_linking_info.wrap_kind, WrapKind::Esm) {
            let wrapper_ref_name =
              self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
            program.body.push(self.snippet.call_expr_stmt(wrapper_ref_name.clone()));
          }

          match importee {
            Module::Normal(importee) => {
              match importee.exports_kind {
                ExportsKind::Esm => {
                  // __reExport(exports, otherExports)
                  // TODO: only should do this if the importee must be re-exported dynamically
                  // let importee_namespace_name = self.canonical_name_for(importee.namespace_symbol);
                  // program.body.push(
                  //   self
                  //     .snippet
                  //     .call_expr_with_2arg_expr(
                  //       re_export_fn_name.clone(),
                  //       importer_namespace_name.clone(),
                  //       importee_namespace_name.clone(),
                  //     )
                  //     .into_in(self.alloc),
                  // );
                }
                ExportsKind::CommonJs => {
                  let re_export_fn_name = self.canonical_name_for_runtime("__reExport");
                  let importer_namespace_name =
                    self.canonical_name_for(self.ctx.module.namespace_symbol);
                  // __reExport(exports, __toESM(require_xxxx()))
                  let to_esm_fn_name = self.canonical_name_for_runtime("__toESM");
                  let importee_wrapper_ref_name =
                    self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
                  program.body.push(
                    self
                      .snippet
                      .call_expr_with_2arg_expr_expr(
                        re_export_fn_name.clone(),
                        self.snippet.id_ref_expr(importer_namespace_name.clone()),
                        self.snippet.call_expr_with_arg_expr_expr(
                          to_esm_fn_name.clone(),
                          self.snippet.call_expr_expr(importee_wrapper_ref_name.clone()),
                        ),
                      )
                      .into_in(self.alloc),
                  );
                }
                ExportsKind::None => {}
              }
            }
            Module::External(_) => {}
          }
          // TODO handle this
          return;
        }
      } else if let Some(default_decl) = top_stmt.as_export_default_declaration_mut() {
        match &mut default_decl.declaration {
          ast::ExportDefaultDeclarationKind::Expression(expr) => {
            // "export default foo;" => "var default = foo;"
            let canonical_name_for_default_export_ref =
              self.canonical_name_for(self.ctx.module.default_export_ref);
            top_stmt = self.snippet.var_decl_stmt(
              canonical_name_for_default_export_ref.clone(),
              expr.take_in(self.alloc),
            );
          }
          ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
            // "export default function() {}" => "function default() {}"
            // "export default function foo() {}" => "function foo() {}"
            if func.id.is_none() {
              let canonical_name_for_default_export_ref =
                self.canonical_name_for(self.ctx.module.default_export_ref);
              func.id = Some(self.snippet.id(canonical_name_for_default_export_ref.clone()));
            }
            top_stmt = ast::Statement::Declaration(ast::Declaration::FunctionDeclaration(
              func.take_in(self.alloc),
            ));
          }
          ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            // "export default class {}" => "class default {}"
            // "export default class Foo {}" => "class Foo {}"
            if class.id.is_none() {
              let canonical_name_for_default_export_ref =
                self.canonical_name_for(self.ctx.module.default_export_ref);
              class.id = Some(self.snippet.id(canonical_name_for_default_export_ref.clone()));
            }
            top_stmt = ast::Statement::Declaration(ast::Declaration::ClassDeclaration(
              class.take_in(self.alloc),
            ));
          }
          _ => {}
        }
      } else if let Some(named_decl) = top_stmt.as_export_named_declaration_mut() {
        if named_decl.source.is_none() {
          if let Some(decl) = &mut named_decl.declaration {
            // `export var foo = 1` => `var foo = 1`
            // `export function foo() {}` => `function foo() {}`
            // `export class Foo {}` => `class Foo {}`
            top_stmt = ast::Statement::Declaration(decl.take_in(self.alloc));
          } else {
            // `export { foo }`
            // Remove this statement by ignoring it
            return;
          }
        } else {
          // `export { foo } from 'path'`
          let rec_id = self.ctx.module.imports[&named_decl.span];
          if self.should_remove_import_export_stmt(&mut top_stmt, rec_id) {
            return;
          }
        }
      }
      program.body.push(top_stmt);
    });

    for stmt in program.body.iter_mut() {
      self.visit_top_level_statement_mut(stmt);
    }

    // check if we need to add wrapper
    match self.ctx.linking_info.wrap_kind {
      WrapKind::Cjs => {
        let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());
        let commonjs_ref_name = self.canonical_name_for_runtime("__commonJSMin");
        let old_body = program.body.take_in(self.alloc);

        program.body.push(self.snippet.commonjs_wrapper_stmt(
          wrap_ref_name.clone(),
          commonjs_ref_name.clone(),
          old_body,
        ));
      }
      WrapKind::Esm => {
        let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());
        let esm_ref_name = self.canonical_name_for_runtime("__esmMin");
        let old_body = program.body.take_in(self.alloc);

        let mut fn_stmts = allocator::Vec::new_in(self.alloc);
        let mut hoisted_names = vec![];
        let mut stmts_inside_closure = allocator::Vec::new_in(self.alloc);

        // Hoist all top-level "var" and "function" declarations out of the closure
        old_body.into_iter().for_each(|mut stmt| match &mut stmt {
          ast::Statement::Declaration(decl) => match decl {
            ast::Declaration::VariableDeclaration(_) | ast::Declaration::ClassDeclaration(_) => {
              if let Some(converted) = self.convert_decl_to_assignment(decl, &mut hoisted_names) {
                stmts_inside_closure.push(converted);
              }
            }
            ast::Declaration::FunctionDeclaration(_) => {
              fn_stmts.push(stmt);
            }
            ast::Declaration::UsingDeclaration(_) => unimplemented!(),
            _ => {}
          },
          ast::Statement::ModuleDeclaration(_) => unreachable!(
            "At this point, all module declarations should have been removed or transformed"
          ),
          _ => {
            stmts_inside_closure.push(stmt);
          }
        });
        program.body.extend(fn_stmts);
        if !hoisted_names.is_empty() {
          let mut declarators = allocator::Vec::new_in(self.alloc);
          declarators.reserve_exact(hoisted_names.len());
          hoisted_names.into_iter().for_each(|var_name| {
            declarators.push(ast::VariableDeclarator {
              id: ast::BindingPattern {
                kind: ast::BindingPatternKind::BindingIdentifier(
                  self.snippet.id(var_name).into_in(self.alloc),
                ),
                ..Dummy::dummy(self.alloc)
              },
              kind: ast::VariableDeclarationKind::Var,
              ..Dummy::dummy(self.alloc)
            });
          });
          program.body.push(ast::Statement::Declaration(ast::Declaration::VariableDeclaration(
            ast::VariableDeclaration {
              declarations: declarators,
              kind: ast::VariableDeclarationKind::Var,
              ..Dummy::dummy(self.alloc)
            }
            .into_in(self.alloc),
          )));
        }
        program.body.push(self.snippet.esm_wrapper_stmt(
          wrap_ref_name.clone(),
          esm_ref_name.clone(),
          stmts_inside_closure,
        ));
      }
      WrapKind::None => {}
    }
  }

  fn visit_binding_identifier(&mut self, ident: &mut ast::BindingIdentifier) {
    if let Some(symbol_id) = ident.symbol_id.get() {
      let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();

      let canonical_ref = self.ctx.symbols.par_canonical_ref_for(symbol_ref);
      let symbol = self.ctx.symbols.get(canonical_ref);
      assert!(symbol.namespace_alias.is_none());
      let canonical_name = self.canonical_name_for(symbol_ref);
      if ident.name != canonical_name {
        ident.name = canonical_name.clone();
      }
    } else {
      // Some `BindingIdentifier`s constructed by bundler don't have `SymbolId` and we just ignore them.
    }
  }

  fn visit_call_expression(&mut self, expr: &mut ast::CallExpression<'ast>) {
    if let ast::Expression::Identifier(id_ref) = &mut expr.callee {
      if let Some(new_name) = self.generate_finalized_expr_for_reference(id_ref, true) {
        expr.callee = new_name;
      }
    }

    // visit children
    for arg in expr.arguments.iter_mut() {
      self.visit_argument(arg);
    }
    self.visit_expression(&mut expr.callee);
    if let Some(parameters) = &mut expr.type_parameters {
      self.visit_ts_type_parameter_instantiation(parameters);
    }
  }

  #[allow(clippy::collapsible_else_if)]
  fn visit_expression(&mut self, expr: &mut ast::Expression<'ast>) {
    if let Some(call_expr) = expr.as_call_expression() {
      if let ast::Expression::Identifier(callee) = &call_expr.callee {
        if callee.name == "require" && self.is_global_identifier_reference(callee) {
          let rec_id = self.ctx.module.imports[&call_expr.span];
          let rec = &self.ctx.module.import_records[rec_id];
          if let Module::Normal(importee) = &self.ctx.modules[rec.resolved_module] {
            let importee_linking_info = &self.ctx.linking_infos[importee.id];
            let wrap_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
            if matches!(importee.exports_kind, ExportsKind::CommonJs) {
              *expr = self.snippet.call_expr_expr(wrap_ref_name.clone());
            } else {
              let ns_name = self.canonical_name_for(importee.namespace_symbol);
              let to_commonjs_ref_name = self.canonical_name_for_runtime("__toCommonJS");
              *expr = self.snippet.seq2_in_paren_expr(
                self.snippet.call_expr_expr(wrap_ref_name.clone()),
                self.snippet.call_expr_with_arg_expr(to_commonjs_ref_name.clone(), ns_name.clone()),
              );
            }
          }
        }
      }
    }

    if let Some(id_ref) = expr.as_identifier() {
      if let Some(new_expr) = self.generate_finalized_expr_for_reference(id_ref, false) {
        *expr = new_expr;
      }
    }

    // visit children
    self.visit_expression_match(expr);
  }

  fn visit_object_property(&mut self, prop: &mut ast::ObjectProperty<'ast>) {
    // rewrite `const val = { a };` to `const val = { a: a.xxx }`
    match prop.value {
      ast::Expression::Identifier(ref id_ref) if prop.shorthand => {
        if let Some(expr) = self.generate_finalized_expr_for_reference(id_ref, true) {
          prop.value = expr;
          prop.shorthand = false;
        }
      }
      _ => {}
    }

    // visit children
    self.visit_property_key(&mut prop.key);
    self.visit_expression(&mut prop.value);
    if let Some(init) = &mut prop.init {
      self.visit_expression(init);
    }
  }

  fn visit_object_pattern(&mut self, pat: &mut ast::ObjectPattern<'ast>) {
    // visit children
    for prop in pat.properties.iter_mut() {
      match &mut prop.value.kind {
        // Rewrite `const { a } = obj;`` to `const { a: a$1 } = obj;`
        ast::BindingPatternKind::BindingIdentifier(ident) if prop.shorthand => {
          if let Some(symbol_id) = ident.symbol_id.get() {
            let canonical_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
            if ident.name != canonical_name {
              ident.name = canonical_name.clone();
              prop.shorthand = false;
            }
          }
        }
        // Rewrite `const { a = 1 } = obj;`` to `const { a: a$1 = 1 } = obj;`
        ast::BindingPatternKind::AssignmentPattern(assign_pat)
          if prop.shorthand
            && matches!(assign_pat.left.kind, ast::BindingPatternKind::BindingIdentifier(_)) =>
        {
          let ast::BindingPatternKind::BindingIdentifier(ident) = &mut assign_pat.left.kind else {
            unreachable!()
          };
          if let Some(symbol_id) = ident.symbol_id.get() {
            let canonical_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
            if ident.name != canonical_name {
              ident.name = canonical_name.clone();
              prop.shorthand = false;
            }
          }
        }
        _ => {}
      }
    }

    // visit children
    for prop in pat.properties.iter_mut() {
      self.visit_binding_property(prop);
    }
    if let Some(rest) = &mut pat.rest {
      self.visit_rest_element(rest);
    }
  }

  fn visit_import_expression(&mut self, expr: &mut ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(str) = &mut expr.source {
      let rec_id = self.ctx.module.imports[&expr.span];
      let rec = &self.ctx.module.import_records[rec_id];
      let importee_id = rec.resolved_module;
      match self.ctx.modules[importee_id] {
        Module::Normal(_) => {
          let chunk_id = self.ctx.chunk_graph.module_to_chunk[importee_id]
            .expect("Normal module should belong to a chunk");
          let chunk = &self.ctx.chunk_graph.chunks[chunk_id];
          str.value = format!("'./{}'", chunk.file_name.as_ref().unwrap()).into();
        }
        Module::External(_) => {
          // external module doesn't belong to any chunk, just keep this as it is
        }
      }
    }
  }
}
