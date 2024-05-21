// cSpell:disable

use oxc::{
  allocator,
  ast::{
    ast::{self, SimpleAssignmentTarget},
    visit::walk_mut,
    VisitMut,
  },
  span::{Span, SPAN},
};
use rolldown_common::{ExportsKind, ModuleId, SymbolRef, WrapKind};
use rolldown_oxc_utils::{ExpressionExt, IntoIn, StatementExt, TakeIn};

use super::Finalizer;

impl<'me, 'ast> VisitMut<'ast> for Finalizer<'me, 'ast> {
  #[allow(clippy::too_many_lines, clippy::match_same_arms)]
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    let old_body = program.body.take_in(self.alloc);

    let is_namespace_referenced = matches!(self.ctx.module.exports_kind, ExportsKind::Esm)
      && self.ctx.module.stmt_infos[0].is_included;

    if is_namespace_referenced {
      program.body.extend(self.generate_namespace_variable_declaration());
    }

    let mut stmt_infos = self.ctx.module.stmt_infos.iter();
    // Skip the first statement info, which is the namespace variable declaration
    stmt_infos.next();

    old_body.into_iter().enumerate().zip(stmt_infos).for_each(
      |((_top_stmt_idx, mut top_stmt), stmt_info)| {
        debug_assert!(matches!(stmt_info.stmt_idx, Some(_top_stmt_idx)));
        if !stmt_info.is_included {
          return;
        }

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
            let ModuleId::Normal(importee_id) = rec.resolved_module else {
              // TODO: handle re-exporting all from external module
              return;
            };
            let importee_linking_info = &self.ctx.linking_infos[importee_id];
            let importee = &self.ctx.modules[importee_id];
            if matches!(importee_linking_info.wrap_kind, WrapKind::Esm) {
              let wrapper_ref_name =
                self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
              program.body.push(self.snippet.call_expr_stmt(wrapper_ref_name));
            }

            match importee.exports_kind {
              ExportsKind::Esm => {
                if importee_linking_info.has_dynamic_exports {
                  let re_export_fn_name = self.canonical_name_for_runtime("__reExport");
                  let importer_namespace_name =
                    self.canonical_name_for(self.ctx.module.namespace_symbol);
                  // __reExport(exports, otherExports)
                  let importee_namespace_name = self.canonical_name_for(importee.namespace_symbol);
                  program.body.push(
                    self
                      .snippet
                      .call_expr_with_2arg_expr(
                        re_export_fn_name,
                        importer_namespace_name,
                        importee_namespace_name,
                      )
                      .into_in(self.alloc),
                  );
                }
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
                      re_export_fn_name,
                      self.snippet.id_ref_expr(importer_namespace_name, SPAN),
                      self.snippet.call_expr_with_arg_expr_expr(
                        to_esm_fn_name,
                        self.snippet.call_expr_expr(importee_wrapper_ref_name),
                      ),
                    )
                    .into_in(self.alloc),
                );
              }
              ExportsKind::None => {}
            }
            return;
          }
        } else if let Some(default_decl) = top_stmt.as_export_default_declaration_mut() {
          use ast::ExportDefaultDeclarationKind;
          match &mut default_decl.declaration {
            decl @ ast::match_expression!(ExportDefaultDeclarationKind) => {
              let expr = decl.to_expression_mut();
              // "export default foo;" => "var default = foo;"
              let canonical_name_for_default_export_ref =
                self.canonical_name_for(self.ctx.module.default_export_ref);
              top_stmt = self
                .snippet
                .var_decl_stmt(canonical_name_for_default_export_ref, expr.take_in(self.alloc));
            }
            ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
              // "export default function() {}" => "function default() {}"
              // "export default function foo() {}" => "function foo() {}"
              if func.id.is_none() {
                let canonical_name_for_default_export_ref =
                  self.canonical_name_for(self.ctx.module.default_export_ref);
                func.id = Some(self.snippet.id(canonical_name_for_default_export_ref, SPAN));
              }
              top_stmt = ast::Statement::FunctionDeclaration(func.take_in(self.alloc));
            }
            ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
              // "export default class {}" => "class default {}"
              // "export default class Foo {}" => "class Foo {}"
              if class.id.is_none() {
                let canonical_name_for_default_export_ref =
                  self.canonical_name_for(self.ctx.module.default_export_ref);
                class.id = Some(self.snippet.id(canonical_name_for_default_export_ref, SPAN));
              }
              top_stmt = ast::Statement::ClassDeclaration(class.take_in(self.alloc));
            }
            _ => {}
          }
        } else if let Some(named_decl) = top_stmt.as_export_named_declaration_mut() {
          if named_decl.source.is_none() {
            if let Some(decl) = &mut named_decl.declaration {
              // `export var foo = 1` => `var foo = 1`
              // `export function foo() {}` => `function foo() {}`
              // `export class Foo {}` => `class Foo {}`
              top_stmt = ast::Statement::from(decl.take_in(self.alloc));
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
      },
    );

    let mut shimmed_exports =
      self.ctx.linking_info.shimmed_missing_exports.iter().collect::<Vec<_>>();
    shimmed_exports.sort_by_key(|(name, _)| name.as_str());
    shimmed_exports.into_iter().for_each(|(_name, symbol_ref)| {
      debug_assert!(!self.ctx.module.stmt_infos.declared_stmts_by_symbol(symbol_ref).is_empty());
      let is_included: bool = self
        .ctx
        .module
        .stmt_infos
        .declared_stmts_by_symbol(symbol_ref)
        .iter()
        .any(|id| self.ctx.module.stmt_infos[*id].is_included);
      if is_included {
        let canonical_name = self.canonical_name_for(*symbol_ref);
        program.body.push(self.snippet.var_decl_stmt(canonical_name, self.snippet.void_zero()));
      }
    });

    walk_mut::walk_program_mut(self, program);

    // check if we need to add wrapper
    let needs_wrapper = self
      .ctx
      .linking_info
      .wrapper_stmt_info
      .is_some_and(|idx| self.ctx.module.stmt_infos[idx].is_included);

    if needs_wrapper {
      match self.ctx.linking_info.wrap_kind {
        WrapKind::Cjs => {
          let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());
          let commonjs_ref_name = self.canonical_name_for_runtime("__commonJSMin");
          let old_body = program.body.take_in(self.alloc);

          program.body.push(self.snippet.commonjs_wrapper_stmt(
            wrap_ref_name,
            commonjs_ref_name,
            old_body,
          ));
        }
        WrapKind::Esm => {
          use ast::Statement;
          let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());
          let esm_ref_name = self.canonical_name_for_runtime("__esmMin");
          let old_body = program.body.take_in(self.alloc);

          let mut fn_stmts = allocator::Vec::new_in(self.alloc);
          let mut hoisted_names = vec![];
          let mut stmts_inside_closure = allocator::Vec::new_in(self.alloc);

          // Hoist all top-level "var" and "function" declarations out of the closure
          old_body.into_iter().for_each(|mut stmt| match &mut stmt {
            ast::Statement::VariableDeclaration(_) | ast::Statement::ClassDeclaration(_) => {
              if let Some(converted) =
                self.convert_decl_to_assignment(stmt.to_declaration_mut(), &mut hoisted_names)
              {
                stmts_inside_closure.push(converted);
              }
            }
            ast::Statement::FunctionDeclaration(_) => {
              fn_stmts.push(stmt);
            }
            ast::Statement::UsingDeclaration(_) => unimplemented!(),
            ast::match_module_declaration!(Statement) => unreachable!(
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
                    self.snippet.id(&var_name, SPAN).into_in(self.alloc),
                  ),
                  ..TakeIn::dummy(self.alloc)
                },
                kind: ast::VariableDeclarationKind::Var,
                ..TakeIn::dummy(self.alloc)
              });
            });
            program.body.push(ast::Statement::VariableDeclaration(
              ast::VariableDeclaration {
                declarations: declarators,
                kind: ast::VariableDeclarationKind::Var,
                ..TakeIn::dummy(self.alloc)
              }
              .into_in(self.alloc),
            ));
          }
          program.body.push(self.snippet.esm_wrapper_stmt(
            wrap_ref_name,
            esm_ref_name,
            stmts_inside_closure,
          ));
        }
        WrapKind::None => {}
      }
    }
  }

  fn visit_binding_identifier(&mut self, ident: &mut ast::BindingIdentifier<'ast>) {
    if let Some(symbol_id) = ident.symbol_id.get() {
      let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();

      let canonical_ref = self.ctx.symbols.par_canonical_ref_for(symbol_ref);
      let symbol = self.ctx.symbols.get(canonical_ref);
      assert!(symbol.namespace_alias.is_none());
      let canonical_name = self.canonical_name_for(symbol_ref);
      if ident.name != canonical_name.as_str() {
        ident.name = self.snippet.atom(canonical_name);
      }
      ident.symbol_id.get_mut().take();
    } else {
      // Some `BindingIdentifier`s constructed by bundler don't have `SymbolId` and we just ignore them.
    }
  }

  fn visit_identifier_reference(&mut self, ident: &mut ast::IdentifierReference) {
    // This ensure all `IdentifierReference`s are processed
    debug_assert!(
      self.is_global_identifier_reference(ident) || ident.reference_id.get().is_none(),
      "{} doesn't get processed in {}",
      ident.name,
      self.ctx.module.repr_name
    );
  }

  fn visit_call_expression(&mut self, expr: &mut ast::CallExpression<'ast>) {
    self.try_rewrite_identifier_reference_expr(&mut expr.callee, true);

    walk_mut::walk_call_expression_mut(self, expr);
  }

  #[allow(clippy::collapsible_else_if)]
  fn visit_expression(&mut self, expr: &mut ast::Expression<'ast>) {
    if let Some(call_expr) = expr.as_call_expression() {
      // Rewrite `require(...)` to `require_xxx(...)` or `(init_xxx(), __toCommonJS(xxx_exports))`
      if let ast::Expression::Identifier(callee) = &call_expr.callee {
        if callee.name == "require" && self.is_global_identifier_reference(callee) {
          let rec_id = self.ctx.module.imports[&call_expr.span];
          let rec = &self.ctx.module.import_records[rec_id];
          if let ModuleId::Normal(importee_id) = rec.resolved_module {
            let importee = &self.ctx.modules[importee_id];
            let importee_linking_info = &self.ctx.linking_infos[importee.id];
            let wrap_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
            if matches!(importee.exports_kind, ExportsKind::CommonJs) {
              *expr = self.snippet.call_expr_expr(wrap_ref_name);
            } else {
              let ns_name = self.canonical_name_for(importee.namespace_symbol);
              let to_commonjs_ref_name = self.canonical_name_for_runtime("__toCommonJS");
              *expr = self.snippet.seq2_in_paren_expr(
                self.snippet.call_expr_expr(wrap_ref_name),
                self.snippet.call_expr_with_arg_expr(to_commonjs_ref_name, ns_name),
              );
            }
          }
        }
      }
    }

    self.try_rewrite_identifier_reference_expr(expr, false);

    walk_mut::walk_expression_mut(self, expr);
  }

  fn visit_object_property(&mut self, prop: &mut ast::ObjectProperty<'ast>) {
    // Ensure `{ a }` would be rewritten to `{ a: a$1 }` instead of `{ a$1 }`
    match &mut prop.value {
      ast::Expression::Identifier(id_ref) if prop.shorthand => {
        if let Some(expr) = self.generate_finalized_expr_for_reference(id_ref, false) {
          prop.value = expr;
          prop.shorthand = false;
        } else {
          id_ref.reference_id.get_mut().take();
        }
      }
      _ => {}
    }

    walk_mut::walk_object_property_mut(self, prop);
  }

  fn visit_object_pattern(&mut self, pat: &mut ast::ObjectPattern<'ast>) {
    for prop in pat.properties.iter_mut() {
      match &mut prop.value.kind {
        // Ensure `const { a } = ...;` will be rewritten to `const { a: a$1 } = ...` instead of `const { a$1 } = ...`
        // Ensure `function foo({ a }) {}` will be rewritten to `function foo({ a: a$1 }) {}` instead of `function foo({ a$1 }) {}`
        ast::BindingPatternKind::BindingIdentifier(ident) if prop.shorthand => {
          if let Some(symbol_id) = ident.symbol_id.get() {
            let canonical_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
            if ident.name != canonical_name.as_str() {
              ident.name = self.snippet.atom(canonical_name);
              prop.shorthand = false;
            }
            ident.symbol_id.get_mut().take();
          }
        }
        // Ensure `const { a = 1 } = ...;` will be rewritten to `const { a: a$1 = 1 } = ...` instead of `const { a$1 = 1 } = ...`
        // Ensure `function foo({ a = 1 }) {}` will be rewritten to `function foo({ a: a$1 = 1 }) {}` instead of `function foo({ a$1 = 1 }) {}`
        ast::BindingPatternKind::AssignmentPattern(assign_pat)
          if prop.shorthand
            && matches!(assign_pat.left.kind, ast::BindingPatternKind::BindingIdentifier(_)) =>
        {
          let ast::BindingPatternKind::BindingIdentifier(ident) = &mut assign_pat.left.kind else {
            unreachable!()
          };
          if let Some(symbol_id) = ident.symbol_id.get() {
            let canonical_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
            if ident.name != canonical_name.as_str() {
              ident.name = self.snippet.atom(canonical_name);
              prop.shorthand = false;
            }
            ident.symbol_id.get_mut().take();
          }
        }
        _ => {
          // For other patterns:
          // - `const [a] = ...` or `function foo([a]) {}`
          // - `const { a: b } = ...` or `function foo({ a: b }) {}`
          // - `const { a: b = 1 } = ...` or `function foo({ a: b = 1 }) {}`
          // They could keep correct semantics after renaming, so we don't need to do anything special.
        }
      }
    }

    walk_mut::walk_object_pattern_mut(self, pat);
  }

  fn visit_import_expression(&mut self, expr: &mut ast::ImportExpression<'ast>) {
    // Make sure the import expression is in correct form. If it's not, we should leave it as it is.
    match &mut expr.source {
      ast::Expression::StringLiteral(str) if expr.arguments.len() == 0 => {
        let rec_id = self.ctx.module.imports[&expr.span];
        let rec = &self.ctx.module.import_records[rec_id];
        let importee_id = rec.resolved_module;
        match importee_id {
          ModuleId::Normal(importee_id) => {
            let importer_chunk_id = self.ctx.chunk_graph.module_to_chunk[self.ctx.module.id]
              .expect("Normal module should belong to a chunk");
            let importer_chunk = &self.ctx.chunk_graph.chunks[importer_chunk_id];

            let importee_chunk_id = self.ctx.chunk_graph.entry_module_to_entry_chunk[&importee_id];
            let importee_chunk = &self.ctx.chunk_graph.chunks[importee_chunk_id];

            let import_path = importer_chunk.import_path_for(importee_chunk);

            str.value = self.snippet.atom(&import_path);
          }
          ModuleId::External(_) => {
            // external module doesn't belong to any chunk, just keep this as it is
          }
        }
      }
      _ => {}
    }

    walk_mut::walk_import_expression_mut(self, expr);
  }

  fn visit_assignment_target_property(
    &mut self,
    property: &mut ast::AssignmentTargetProperty<'ast>,
  ) {
    if let ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(prop) = property {
      if let Some(target) =
        self.generate_finalized_simple_assignment_target_for_reference(&prop.binding)
      {
        *property = ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
          ast::AssignmentTargetPropertyProperty {
            name: ast::PropertyKey::StaticIdentifier(
              self.snippet.id_name(&prop.binding.name, prop.span).into_in(self.alloc),
            ),
            binding: if let Some(init) = prop.init.take() {
              ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                ast::AssignmentTargetWithDefault {
                  binding: ast::AssignmentTarget::from(target),
                  init,
                  span: Span::default(),
                }
                .into_in(self.alloc),
              )
            } else {
              ast::AssignmentTargetMaybeDefault::from(target)
            },
            span: Span::default(),
          }
          .into_in(self.alloc),
        );
      } else {
        prop.binding.reference_id.get_mut().take();
      }
    }

    walk_mut::walk_assignment_target_property_mut(self, property);
  }

  fn visit_simple_assignment_target(&mut self, target: &mut SimpleAssignmentTarget<'ast>) {
    self.rewrite_simple_assignment_target(target);

    walk_mut::walk_simple_assignment_target_mut(self, target);
  }
}
