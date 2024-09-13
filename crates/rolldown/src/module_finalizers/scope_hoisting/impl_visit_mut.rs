// cSpell:disable

use oxc::{
  allocator::{self, IntoIn},
  ast::{
    ast::{self, Expression, SimpleAssignmentTarget},
    visit::walk_mut,
    VisitMut,
  },
  span::{GetSpan, Span, SPAN},
};
use rolldown_common::{ExportsKind, Module, ModuleType, StmtInfoIdx, SymbolRef, WrapKind};
use rolldown_ecmascript::{AllocatorExt, ExpressionExt, StatementExt, TakeIn};

use crate::utils::call_expression_ext::CallExpressionExt;

use super::ScopeHoistingFinalizer;

impl<'me, 'ast> VisitMut<'ast> for ScopeHoistingFinalizer<'me, 'ast> {
  #[allow(clippy::too_many_lines)]
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    let old_body = self.alloc.take(&mut program.body);

    let is_namespace_referenced = matches!(self.ctx.module.exports_kind, ExportsKind::Esm)
      && self.ctx.module.stmt_infos[StmtInfoIdx::new(0)].is_included;

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
            match &self.ctx.modules[rec.resolved_module] {
              Module::Ecma(importee) => {
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];
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
                        self.canonical_name_for(self.ctx.module.namespace_object_ref);
                      // __reExport(exports, otherExports)
                      let importee_namespace_name =
                        self.canonical_name_for(importee.namespace_object_ref);
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
                      self.canonical_name_for(self.ctx.module.namespace_object_ref);
                    // __reExport(exports, __toESM(require_xxxx()))
                    let to_esm_fn_name = self.canonical_name_for_runtime("__toESM");
                    let importee_wrapper_ref_name =
                      self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
                    program.body.push(
                      self
                        .snippet
                        .alloc_call_expr_with_2arg_expr_expr(
                          re_export_fn_name,
                          self.snippet.id_ref_expr(importer_namespace_name, SPAN),
                          self.snippet.to_esm_call_with_interop(
                            to_esm_fn_name,
                            self.snippet.call_expr_expr(importee_wrapper_ref_name),
                            importee.interop(),
                          ),
                        )
                        .into_in(self.alloc),
                    );
                  }
                  ExportsKind::None => {}
                }
              }
              Module::External(_importee) => {
                match self.ctx.options.format {
                  rolldown_common::OutputFormat::Esm
                  | rolldown_common::OutputFormat::Iife
                  | rolldown_common::OutputFormat::Cjs => {
                    // Just remove the statement
                    return;
                  }
                  rolldown_common::OutputFormat::App => {
                    unreachable!()
                  }
                }
              }
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

    if is_namespace_referenced {
      let mut stmts = self.generate_declaration_of_module_namespace_object();
      stmts.extend(program.body.take_in(self.alloc));
      program.body.extend(stmts);
    }

    let mut shimmed_exports =
      self.ctx.linking_info.shimmed_missing_exports.iter().collect::<Vec<_>>();
    shimmed_exports.sort_unstable_by_key(|(name, _)| name.as_str());
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

    walk_mut::walk_program(self, program);

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
            ast::match_module_declaration!(Statement) => {
              if stmt.is_typescript_syntax() {
                unreachable!(
                  "At this point, typescript module declarations should have been removed or transformed"
                )
              }
              program.body.push(stmt);
            }
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

    walk_mut::walk_call_expression(self, expr);
  }

  #[allow(clippy::collapsible_else_if, clippy::too_many_lines)]
  fn visit_expression(&mut self, expr: &mut ast::Expression<'ast>) {
    if let Some(call_expr) = expr.as_call_expression_mut() {
      if call_expr.is_global_require_call(self.scope) && !call_expr.span.is_empty() {
        //  `require` calls that can't be recognized by rolldown are ignored in scanning, so they were not stored in `NomralModule#imports`.
        //  we just keep these `require` calls as it is
        if let Some(rec_id) = self.ctx.module.imports.get(&call_expr.span).copied() {
          let rec = &self.ctx.module.import_records[rec_id];
          match &self.ctx.modules[rec.resolved_module] {
            Module::Ecma(importee) => {
              match importee.module_type {
                ModuleType::Json => {
                  // Nodejs treats json files as an esm module with a default export and rolldown follows this behavior.
                  // And to make sure the runtime behavior is correct, we need to rewrite `require('xxx.json')` to `require('xxx.json').default` to align with the runtime behavior of nodejs.

                  // Rewrite `require(...)` to `require_xxx(...)` or `(init_xxx(), __toCommonJS(xxx_exports).default)`
                  let importee_linking_info = &self.ctx.linking_infos[importee.idx];
                  let wrap_ref_name =
                    self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
                  if matches!(importee.exports_kind, ExportsKind::CommonJs) {
                    *expr = self.snippet.call_expr_expr(wrap_ref_name);
                  } else {
                    let ns_name = self.canonical_name_for(importee.namespace_object_ref);
                    let to_commonjs_ref_name = self.canonical_name_for_runtime("__toCommonJS");
                    *expr = self.snippet.seq2_in_paren_expr(
                      self.snippet.call_expr_expr(wrap_ref_name),
                      ast::Expression::StaticMemberExpression(
                        ast::StaticMemberExpression {
                          object: self
                            .snippet
                            .call_expr_with_arg_expr(to_commonjs_ref_name, ns_name),
                          property: self.snippet.id_name("default", SPAN),
                          ..TakeIn::dummy(self.alloc)
                        }
                        .into_in(self.alloc),
                      ),
                    );
                  }
                }
                _ => {
                  // Rewrite `require(...)` to `require_xxx(...)` or `(init_xxx(), __toCommonJS(xxx_exports))`
                  let importee_linking_info = &self.ctx.linking_infos[importee.idx];
                  let wrap_ref_name =
                    self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
                  if matches!(importee.exports_kind, ExportsKind::CommonJs) {
                    *expr = self.snippet.call_expr_expr(wrap_ref_name);
                  } else {
                    let ns_name = self.canonical_name_for(importee.namespace_object_ref);
                    let to_commonjs_ref_name = self.canonical_name_for_runtime("__toCommonJS");
                    *expr = self.snippet.seq2_in_paren_expr(
                      self.snippet.call_expr_expr(wrap_ref_name),
                      self.snippet.call_expr_with_arg_expr(to_commonjs_ref_name, ns_name),
                    );
                  }
                }
              }
            }
            Module::External(importee) => {
              let request_path =
                call_expr.arguments.get_mut(0).expect("require should have an argument");

              // Rewrite `require('xxx')` to `require('fs')`, if there is an alias that maps 'xxx' to 'fs'
              *request_path = ast::Argument::StringLiteral(
                self.snippet.alloc_string_literal(&importee.name, request_path.span()),
              );
            }
          }
        }
      }
    }

    self.try_rewrite_identifier_reference_expr(expr, false);

    // rewrite `foo_ns.bar` to `bar` directly
    match expr {
      Expression::StaticMemberExpression(ref inner_expr) => {
        if let Some(resolved) =
          self.ctx.linking_info.resolved_member_expr_refs.get(&inner_expr.span)
        {
          match resolved {
            Some((object_ref, props)) => {
              let object_ref_expr = self.finalized_expr_for_symbol_ref(*object_ref, false);

              let replaced_expr =
                self.snippet.member_expr_or_ident_ref(object_ref_expr, props, inner_expr.span);
              *expr = replaced_expr;
            }
            None => {
              *expr = self.snippet.void_zero();
            }
          }
        };
      }
      _ => {}
    };

    // inline dynamic import
    if self.ctx.options.inline_dynamic_imports {
      if let Expression::ImportExpression(import_expr) = expr {
        let rec_id = self.ctx.module.imports[&import_expr.span];
        let rec = &self.ctx.module.import_records[rec_id];
        let importee_id = rec.resolved_module;
        match &self.ctx.modules[importee_id] {
          Module::Ecma(importee) => {
            let importee_linking_info = &self.ctx.linking_infos[importee_id];
            match importee_linking_info.wrap_kind {
              WrapKind::Esm => {
                // `(init_foo(), j)`
                let importee_linking_info = &self.ctx.linking_infos[importee_id];
                let importee_wrapper_ref_name =
                  self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
                let importee_namespace_name =
                  self.canonical_name_for(importee.namespace_object_ref);
                *expr = self.snippet.promise_resolve_then_call_expr(
                  expr.span(),
                  self.snippet.builder.vec1(self.snippet.return_stmt(
                    self.snippet.seq2_in_paren_expr(
                      self.snippet.call_expr_expr(importee_wrapper_ref_name),
                      self.snippet.id_ref_expr(importee_namespace_name, SPAN),
                    ),
                  )),
                );
              }
              WrapKind::Cjs => {
                //  `__toESM(require_foo())`
                let to_esm_fn_name = self.canonical_name_for_runtime("__toESM");
                let importee_wrapper_ref_name =
                  self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());

                *expr = self.snippet.promise_resolve_then_call_expr(
                  expr.span(),
                  self.snippet.builder.vec1(self.snippet.return_stmt(
                    self.snippet.to_esm_call_with_interop(
                      to_esm_fn_name,
                      self.snippet.call_expr_expr(importee_wrapper_ref_name),
                      importee.interop(),
                    ),
                  )),
                );
              }
              WrapKind::None => {}
            }
          }
          Module::External(_) => {
            // iife format doesn't support external module
          }
        }
        return;
      }
    }

    walk_mut::walk_expression(self, expr);
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

    walk_mut::walk_object_property(self, prop);
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

    walk_mut::walk_object_pattern(self, pat);
  }

  fn visit_import_expression(&mut self, expr: &mut ast::ImportExpression<'ast>) {
    // Make sure the import expression is in correct form. If it's not, we should leave it as it is.
    match &mut expr.source {
      ast::Expression::StringLiteral(str) if expr.arguments.len() == 0 => {
        let rec_id = self.ctx.module.imports[&expr.span];
        let rec = &self.ctx.module.import_records[rec_id];
        let importee_id = rec.resolved_module;
        match &self.ctx.modules[importee_id] {
          Module::Ecma(_importee) => {
            let importer_chunk_id = self.ctx.chunk_graph.module_to_chunk[self.ctx.module.idx]
              .expect("Normal module should belong to a chunk");
            let importer_chunk = &self.ctx.chunk_graph.chunk_table[importer_chunk_id];

            let importee_chunk_id = self.ctx.chunk_graph.entry_module_to_entry_chunk[&importee_id];
            let importee_chunk = &self.ctx.chunk_graph.chunk_table[importee_chunk_id];

            let import_path = importer_chunk.import_path_for(importee_chunk);

            str.value = self.snippet.atom(&import_path);
          }
          Module::External(importee) => {
            if str.value != importee.name {
              str.value = self.snippet.atom(&importee.name);
            }
          }
        }
      }
      _ => {}
    }

    walk_mut::walk_import_expression(self, expr);
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

    walk_mut::walk_assignment_target_property(self, property);
  }

  fn visit_simple_assignment_target(&mut self, target: &mut SimpleAssignmentTarget<'ast>) {
    self.rewrite_simple_assignment_target(target);

    walk_mut::walk_simple_assignment_target(self, target);
  }
}
