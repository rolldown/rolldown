use oxc::{
  allocator::{Allocator, IntoIn},
  ast::ast::{self, IdentifierReference, Statement},
  span::{Atom, SPAN},
};
use rolldown_common::{AstScopes, ImportRecordIdx, Module, OutputFormat, SymbolRef, WrapKind};
use rolldown_ecmascript::{AstSnippet, BindingPatternExt, TakeIn};

mod finalizer_context;
mod impl_visit_mut;
pub use finalizer_context::ScopeHoistingFinalizerContext;
use rolldown_rstr::Rstr;
use rolldown_utils::ecma_script::is_validate_identifier_name;

mod rename;

/// Finalizer for emitting output code with scope hoisting.
pub struct ScopeHoistingFinalizer<'me, 'ast> {
  pub ctx: ScopeHoistingFinalizerContext<'me>,
  pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
}

impl<'me, 'ast> ScopeHoistingFinalizer<'me, 'ast> {
  pub fn is_global_identifier_reference(&self, id_ref: &IdentifierReference) -> bool {
    let Some(reference_id) = id_ref.reference_id.get() else {
      // Some `IdentifierReference`s constructed by bundler don't have a `ReferenceId`. They might be global variables.
      // But we don't care about them in this method. This method is only used to check if a `IdentifierReference` from user code is a global variable.
      return false;
    };
    self.scope.is_unresolved(reference_id)
  }

  pub fn canonical_name_for(&self, symbol: SymbolRef) -> &'me Rstr {
    self.ctx.symbols.canonical_name_for(symbol, self.ctx.canonical_names)
  }

  pub fn canonical_name_for_runtime(&self, name: &str) -> &Rstr {
    let sym_ref = self.ctx.runtime.resolve_symbol(name);
    self.canonical_name_for(sym_ref)
  }

  fn should_remove_import_export_stmt(
    &self,
    stmt: &mut Statement<'ast>,
    rec_id: ImportRecordIdx,
  ) -> bool {
    let rec = &self.ctx.module.import_records[rec_id];
    let Module::Ecma(importee) = &self.ctx.modules[rec.resolved_module] else {
      return true;
    };
    let importee_linking_info = &self.ctx.linking_infos[importee.idx];
    match importee_linking_info.wrap_kind {
      WrapKind::None => {
        // Remove this statement by ignoring it
      }
      WrapKind::Cjs => {
        // Replace the statement with something like `var import_foo = __toESM(require_foo())`
        let to_esm_fn_name = self.canonical_name_for_runtime("__toESM");
        let wrapper_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
        let binding_name_for_wrapper_call_ret = self.canonical_name_for(rec.namespace_ref);
        *stmt = self.snippet.var_decl_stmt(
          binding_name_for_wrapper_call_ret,
          self.snippet.call_expr_with_arg_expr_expr(
            to_esm_fn_name,
            self.snippet.call_expr_expr(wrapper_ref_name),
          ),
        );
        return false;
      }
      // Replace the statement with something like `init_foo()`
      WrapKind::Esm => {
        let wrapper_ref_name = self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
        *stmt = self.snippet.call_expr_stmt(wrapper_ref_name);
        return false;
      }
    }
    true
  }

  fn finalized_expr_for_symbol_ref(
    &self,
    symbol_ref: SymbolRef,
    preserve_this_semantic_if_needed: bool,
  ) -> ast::Expression<'ast> {
    let mut canonical_ref = self.ctx.symbols.par_canonical_ref_for(symbol_ref);
    let mut canonical_symbol = self.ctx.symbols.get(canonical_ref);
    let namespace_alias = &canonical_symbol.namespace_alias;
    if let Some(ns_alias) = namespace_alias {
      canonical_ref = ns_alias.namespace_ref;
      canonical_symbol = self.ctx.symbols.get(canonical_ref);
    }

    let mut expr = match self.ctx.options.format {
      rolldown_common::OutputFormat::Cjs => {
        if canonical_symbol.chunk_id.is_none() {
          // Scoped scopes must belong to its own chunk, so they don't get assigned to a chunk.
          self.snippet.id_ref_expr(self.canonical_name_for(canonical_ref), SPAN)
        } else {
          let chunk_idx_of_canonical_symbol =
            canonical_symbol.chunk_id.unwrap_or_else(|| {
              let symbol_name = self.ctx.symbols.get_original_name(canonical_ref);
              eprintln!(
                "{canonical_ref:?} {symbol_name:?} is not in any chunk, which is unexpected",
              );
              panic!("{canonical_ref:?} {symbol_name:?} is not in any chunk, which is unexpected");
            });
          let cur_chunk_idx = self.ctx.chunk_graph.module_to_chunk[self.ctx.id]
            .expect("This module should be in a chunk");
          let is_symbol_in_other_chunk = cur_chunk_idx != chunk_idx_of_canonical_symbol;
          if is_symbol_in_other_chunk {
            // In cjs output, we need convert the `import { foo } from 'foo'; console.log(foo);`;
            // If `foo` is split into another chunk, we need to convert the code `console.log(foo);` to `console.log(require_xxxx.foo);`
            // instead of keeping `console.log(foo)` as we did in esm output. The reason here is wee need to keep live binding in cjs output.

            let exported_name = &self.ctx.chunk_graph.chunk_table[chunk_idx_of_canonical_symbol]
              .exports_to_other_chunks[&canonical_ref];

            let require_binding = &self.ctx.chunk_graph.chunk_table[cur_chunk_idx]
              .require_binding_names_for_other_chunks[&chunk_idx_of_canonical_symbol];

            self.snippet.literal_prop_access_member_expr_expr(require_binding, exported_name)
          } else {
            self.snippet.id_ref_expr(self.canonical_name_for(canonical_ref), SPAN)
          }
        }
      }
      _ => self.snippet.id_ref_expr(self.canonical_name_for(canonical_ref), SPAN),
    };

    if let Some(ns_alias) = namespace_alias {
      expr = ast::Expression::StaticMemberExpression(
        self.snippet.builder.alloc_static_member_expression(
          SPAN,
          expr,
          self.snippet.id_name(&ns_alias.property_name, SPAN),
          false,
        ),
      );
      if preserve_this_semantic_if_needed {
        expr = self.snippet.seq2_in_paren_expr(self.snippet.number_expr(0.0, "0"), expr);
      }
    }

    expr
  }

  fn convert_decl_to_assignment(
    &self,
    decl: &mut ast::Declaration<'ast>,
    hoisted_names: &mut Vec<Atom<'ast>>,
  ) -> Option<ast::Statement<'ast>> {
    match decl {
      ast::Declaration::VariableDeclaration(var_decl) => {
        let mut seq_expr = ast::SequenceExpression::dummy(self.alloc);
        var_decl.declarations.iter_mut().for_each(|var_decl| {
          var_decl.id.binding_identifiers().iter().for_each(|id| {
            hoisted_names.push(id.name.clone());
          });
          // Turn `var ... = ...` to `... = ...`
          if let Some(init_expr) = &mut var_decl.init {
            let left = var_decl.id.take_in(self.alloc).into_assignment_target(self.alloc);
            seq_expr.expressions.push(ast::Expression::AssignmentExpression(
              ast::AssignmentExpression {
                left,
                right: init_expr.take_in(self.alloc),
                ..TakeIn::dummy(self.alloc)
              }
              .into_in(self.alloc),
            ));
          };
        });
        if seq_expr.expressions.is_empty() {
          None
        } else {
          Some(ast::Statement::ExpressionStatement(
            ast::ExpressionStatement {
              expression: ast::Expression::SequenceExpression(seq_expr.into_in(self.alloc)),
              ..TakeIn::dummy(self.alloc)
            }
            .into_in(self.alloc),
          ))
        }
      }
      ast::Declaration::ClassDeclaration(cls_decl) => {
        let cls_name = cls_decl.id.take().expect("should have a name at this point").name;
        hoisted_names.push(cls_name.clone());
        // Turn `class xxx {}` to `xxx = class {}`
        Some(ast::Statement::ExpressionStatement(
          ast::ExpressionStatement {
            expression: ast::Expression::AssignmentExpression(
              ast::AssignmentExpression {
                left: self.snippet.simple_id_assignment_target(&cls_name, cls_decl.span),
                right: ast::Expression::ClassExpression(cls_decl.take_in(self.alloc)),
                ..TakeIn::dummy(self.alloc)
              }
              .into_in(self.alloc),
            ),
            ..TakeIn::dummy(self.alloc)
          }
          .into_in(self.alloc),
        ))
      }
      ast::Declaration::FunctionDeclaration(_) => {
        // Function declaration itself as a whole will be hoisted, so we don't need to convert it to an assignment.
        None
      }
      _ => unreachable!("TypeScript code should be preprocessed"),
    }
  }

  fn generate_declaration_of_module_namespace_object(&self) -> Vec<ast::Statement<'ast>> {
    let var_name = self.canonical_name_for(self.ctx.module.namespace_object_ref);
    // construct `var ns_name = {}`
    let decl_stmt = self
      .snippet
      .var_decl_stmt(var_name, ast::Expression::ObjectExpression(TakeIn::dummy(self.alloc)));

    let exports_len = self.ctx.linking_info.canonical_exports().count();

    let export_all_externals_rec_ids = &self.ctx.linking_info.star_exports_from_external_modules;

    let mut re_export_external_stmts: Option<_> = None;
    if !export_all_externals_rec_ids.is_empty() {
      // construct `__reExport(exports, foo_ns)`
      let re_export_fn_name = self.canonical_name_for_runtime("__reExport");
      match self.ctx.options.format {
        OutputFormat::Esm => {
          let stmts = export_all_externals_rec_ids.iter().copied().flat_map(|idx| {
            let rec = &self.ctx.module.import_records[idx];
            let importee_namespace_name = self.canonical_name_for(rec.namespace_ref);
            let m = self.ctx.modules.get(rec.resolved_module);
            let Some(Module::External(module)) = m else {
              return vec![];
            };
            // Insert `import * as ns from 'ext'`external module in esm format
            // Insert `__reExport(exports, ns)`
            let importee_name = &module.name;
            vec![
              self.snippet.import_star_stmt(importee_name, importee_namespace_name),
              self.snippet.builder.statement_expression(
                SPAN,
                self.snippet.call_expr_with_2arg_expr(
                  re_export_fn_name,
                  var_name,
                  importee_namespace_name,
                ),
              ),
            ]
          });
          re_export_external_stmts = Some(stmts.collect::<Vec<_>>());
        }
        OutputFormat::Cjs | OutputFormat::Iife => {
          let stmts = export_all_externals_rec_ids.iter().copied().map(|idx| {
            // Insert `__reExport(exports, require('ext'))`
            let importer_namespace_name =
              self.canonical_name_for(self.ctx.module.namespace_object_ref);
            let rec = &self.ctx.module.import_records[idx];
            let importee = &self.ctx.modules[rec.resolved_module];
            let stmt: ast::Statement = self
              .snippet
              .alloc_call_expr_with_2arg_expr_expr(
                re_export_fn_name,
                self.snippet.id_ref_expr(importer_namespace_name, SPAN),
                self.snippet.call_expr_with_arg_expr_expr(
                  "require",
                  self.snippet.string_literal_expr(importee.id(), SPAN),
                ),
              )
              .into_in(self.alloc);

            stmt
          });
          re_export_external_stmts = Some(stmts.collect());
        }
        OutputFormat::App => unreachable!(),
      }
    };

    if exports_len == 0 {
      let mut ret = vec![decl_stmt];
      ret.extend(re_export_external_stmts.unwrap_or_default());
      return ret;
    }

    // construct `{ prop_name: () => returned, ... }`
    let mut arg_obj_expr = ast::ObjectExpression::dummy(self.alloc);
    arg_obj_expr.properties.reserve_exact(exports_len);

    self.ctx.linking_info.canonical_exports().for_each(|(export, resolved_export)| {
      // prop_name: () => returned
      let prop_name = export;
      let returned = self.finalized_expr_for_symbol_ref(resolved_export.symbol_ref, false);
      arg_obj_expr.properties.push(ast::ObjectPropertyKind::ObjectProperty(
        ast::ObjectProperty {
          key: if is_validate_identifier_name(prop_name) {
            ast::PropertyKey::StaticIdentifier(
              self.snippet.id_name(prop_name, SPAN).into_in(self.alloc),
            )
          } else {
            ast::PropertyKey::StringLiteral(self.snippet.alloc_string_literal(prop_name, SPAN))
          },
          value: self.snippet.only_return_arrow_expr(returned),
          ..TakeIn::dummy(self.alloc)
        }
        .into_in(self.alloc),
      ));
    });

    // construct `__export(ns_name, { prop_name: () => returned, ... })`
    let mut export_call_expr = self.snippet.call_expr(self.canonical_name_for_runtime("__export"));
    export_call_expr.arguments.push(ast::Argument::from(self.snippet.id_ref_expr(var_name, SPAN)));
    export_call_expr
      .arguments
      .push(ast::Argument::ObjectExpression(arg_obj_expr.into_in(self.alloc)));
    let export_call_stmt = ast::Statement::ExpressionStatement(
      ast::ExpressionStatement {
        expression: ast::Expression::CallExpression(export_call_expr.into_in(self.alloc)),
        ..TakeIn::dummy(self.alloc)
      }
      .into_in(self.alloc),
    );
    let mut ret = vec![decl_stmt, export_call_stmt];
    ret.extend(re_export_external_stmts.unwrap_or_default());

    ret
  }
}
