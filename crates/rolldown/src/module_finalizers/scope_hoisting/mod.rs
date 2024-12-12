use oxc::{
  allocator::{Allocator, IntoIn},
  ast::{
    ast::{self, BindingIdentifier, Expression, IdentifierReference, MemberExpression, Statement},
    Comment, NONE,
  },
  semantic::SymbolId,
  span::{Atom, GetSpan, SPAN},
};
use rolldown_common::{
  AstScopes, ImportRecordIdx, ImportRecordMeta, Module, OutputFormat, Platform, SymbolRef, WrapKind,
};
use rolldown_ecmascript_utils::{AstSnippet, BindingPatternExt, ExpressionExt, TakeIn};

mod finalizer_context;
mod impl_visit_mut;
pub use finalizer_context::ScopeHoistingFinalizerContext;
use rolldown_rstr::Rstr;
use rolldown_std_utils::PathExt;
use rolldown_utils::ecmascript::is_validate_identifier_name;
use sugar_path::SugarPath;

mod rename;

/// Finalizer for emitting output code with scope hoisting.
pub struct ScopeHoistingFinalizer<'me, 'ast> {
  pub ctx: ScopeHoistingFinalizerContext<'me>,
  pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
  pub comments: oxc::allocator::Vec<'ast, Comment>,
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
    self.ctx.symbol_db.canonical_name_for(symbol, self.ctx.canonical_names)
  }

  pub fn canonical_name_for_runtime(&self, name: &str) -> &Rstr {
    let sym_ref = self.ctx.runtime.resolve_symbol(name);
    self.canonical_name_for(sym_ref)
  }

  pub fn canonical_ref_for_runtime(&self, name: &str) -> SymbolRef {
    self.ctx.runtime.resolve_symbol(name)
  }

  /// If return true the import stmt should be removed,
  /// or transform the import stmt to target form.
  fn transform_or_remove_import_export_stmt(
    &self,
    stmt: &mut Statement<'ast>,
    rec_id: ImportRecordIdx,
  ) -> bool {
    let rec = &self.ctx.module.import_records[rec_id];
    let Module::Normal(importee) = &self.ctx.modules[rec.resolved_module] else {
      return true;
    };
    let importee_linking_info = &self.ctx.linking_infos[importee.idx];
    match importee_linking_info.wrap_kind {
      WrapKind::None => {
        // Remove this statement by ignoring it
      }
      WrapKind::Cjs => {
        // Replace the statement with something like `var import_foo = __toESM(require_foo())`

        // `__toESM`
        let to_esm_fn_name =
          self.finalized_expr_for_symbol_ref(self.canonical_ref_for_runtime("__toESM"), false);

        // `require_foo`
        let importee_wrapper_ref_name =
          self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());

        // `import_foo`
        let binding_name_for_wrapper_call_ret = self.canonical_name_for(rec.namespace_ref);

        // If the module is an ESM module that follows the Node.js ESM spec, such as
        // - extension is `.mjs`
        // - `package.json` has `"type": "module"`
        // , we need to consider to stimulate the Node.js ESM behavior for maximum compatibility.
        let should_consider_node_esm_spec = self.ctx.module.ecma_view.def_format.is_esm();

        *stmt = self.snippet.var_decl_stmt(
          binding_name_for_wrapper_call_ret,
          self.snippet.wrap_with_to_esm(
            to_esm_fn_name,
            self.snippet.call_expr_expr(importee_wrapper_ref_name),
            should_consider_node_esm_spec,
          ),
        );
        return false;
      }
      // Replace the import statement with `init_foo()` if `ImportDeclaration` is not a plain import
      // or the importee have side effects.
      WrapKind::Esm => {
        if rec.meta.contains(ImportRecordMeta::IS_PLAIN_IMPORT)
          && !importee.side_effects.has_side_effects()
        {
          return true;
        };
        // `init_foo`
        let wrapper_ref_expr =
          self.finalized_expr_for_symbol_ref(importee_linking_info.wrapper_ref.unwrap(), false);

        // `init_foo()`
        *stmt = self.snippet.builder.statement_expression(
          SPAN,
          ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
            stmt.span(),
            wrapper_ref_expr,
            NONE,
            self.snippet.builder.vec(),
            false,
          )),
        );
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
    if !symbol_ref.is_declared_in_root_scope(self.ctx.symbol_db) {
      // No fancy things on none root scope symbols
      return self.snippet.id_ref_expr(self.canonical_name_for(symbol_ref), SPAN);
    }

    let mut canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);
    let mut canonical_symbol = self.ctx.symbol_db.get(canonical_ref);
    let namespace_alias = &canonical_symbol.namespace_alias;
    if let Some(ns_alias) = namespace_alias {
      canonical_ref = ns_alias.namespace_ref;
      canonical_symbol = self.ctx.symbol_db.get(canonical_ref);
    }

    let mut expr = if self.ctx.modules[canonical_ref.owner].is_external() {
      self.snippet.id_ref_expr(self.canonical_name_for(canonical_ref), SPAN)
    } else {
      match self.ctx.options.format {
        rolldown_common::OutputFormat::Cjs => {
          let chunk_idx_of_canonical_symbol =
            canonical_symbol.chunk_id.unwrap_or_else(|| {
              // Scoped symbols don't get assigned a `ChunkId`. There are skipped for performance reason, because they are surely
              // belong to the chunk they are declared in and won't link to other chunks.
              let symbol_name = canonical_ref.name(self.ctx.symbol_db);
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
        _ => self.snippet.id_ref_expr(self.canonical_name_for(canonical_ref), SPAN),
      }
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
      // construct `__reExport(exports, foo_exports)`
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
        OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd => {
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

  // Handle `import.meta.xxx` expression
  pub fn try_rewrite_import_meta_prop_expr(
    &self,
    member_expr: &ast::StaticMemberExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    if member_expr.object.is_import_meta() {
      let original_expr_span = member_expr.span;
      let is_node_cjs = matches!(
        (self.ctx.options.platform, &self.ctx.options.format),
        (Platform::Node, OutputFormat::Cjs)
      );

      let property_name = member_expr.property.name.as_str();
      match property_name {
        // Try to polyfill `import.meta.url`
        "url" => {
          let new_expr = if is_node_cjs {
            // Replace it with `require('url').pathToFileURL(__filename).href`

            // require('url')
            let require_call = self.snippet.builder.alloc_call_expression(
              SPAN,
              self.snippet.builder.expression_identifier_reference(SPAN, "require"),
              oxc::ast::NONE,
              self.snippet.builder.vec1(ast::Argument::StringLiteral(
                self.snippet.builder.alloc_string_literal(SPAN, "url", None),
              )),
              false,
            );

            // require('url').pathToFileURL
            let require_path_to_file_url = self.snippet.builder.alloc_static_member_expression(
              SPAN,
              ast::Expression::CallExpression(require_call),
              self.snippet.builder.identifier_name(SPAN, "pathToFileURL"),
              false,
            );

            // require('url').pathToFileURL(__filename)
            let require_path_to_file_url_call = self.snippet.builder.alloc_call_expression(
              SPAN,
              ast::Expression::StaticMemberExpression(require_path_to_file_url),
              oxc::ast::NONE,
              self.snippet.builder.vec1(ast::Argument::Identifier(
                self.snippet.builder.alloc_identifier_reference(SPAN, "__filename"),
              )),
              false,
            );

            // require('url').pathToFileURL(__filename).href
            let require_path_to_file_url_href =
              self.snippet.builder.alloc_static_member_expression(
                original_expr_span,
                ast::Expression::CallExpression(require_path_to_file_url_call),
                self.snippet.builder.identifier_name(SPAN, "href"),
                false,
              );
            Some(ast::Expression::StaticMemberExpression(require_path_to_file_url_href))
          } else {
            // If we don't support polyfill `import.meta.url` in this platform and format, we just keep it as it is
            // so users may handle it in their own way.
            None
          };
          return new_expr;
        }
        "dirname" | "filename" => {
          return is_node_cjs.then_some(ast::Expression::Identifier(
            self.snippet.builder.alloc_identifier_reference(SPAN, format!("__{property_name}")),
          ));
        }
        _ => {}
      }
    }
    None
  }

  pub fn handle_new_url_with_string_literal_and_import_meta_url(
    &self,
    expr: &mut ast::NewExpression<'ast>,
  ) -> Option<()> {
    let is_callee_global_url = matches!(expr.callee.as_identifier(), Some(ident) if ident.name == "URL" && self.is_global_identifier_reference(ident));

    if !is_callee_global_url {
      return None;
    }

    let is_second_arg_import_meta_url = expr
      .arguments
      .get(1)
      .map_or(false, |arg| arg.as_expression().is_some_and(ExpressionExt::is_import_meta_url));

    if !is_second_arg_import_meta_url {
      return None;
    }

    let span = expr.span();
    let first_arg_string_literal =
      expr.arguments.first_mut().and_then(|arg| arg.as_expression_mut()).and_then(
        |item| match item {
          ast::Expression::StringLiteral(lit) => Some(lit),
          _ => None,
        },
      )?;

    let &rec_idx = self.ctx.module.new_url_references.get(&span)?;
    let rec = &self.ctx.module.import_records[rec_idx];

    let importee = &self.ctx.modules[rec.resolved_module].as_normal()?;
    let chunk_idx = &self.ctx.chunk_graph.module_to_chunk[importee.idx]?;
    let chunk = &self.ctx.chunk_graph.chunk_table[*chunk_idx];
    let asset_filename = &chunk.asset_absolute_preliminary_filenames[&importee.idx];
    let cur_chunk_idx =
      self.ctx.chunk_graph.module_to_chunk[self.ctx.id].expect("This module should be in a chunk");
    let current_chunk_filename = &self.ctx.chunk_graph.chunk_table[cur_chunk_idx]
      .absolute_preliminary_filename
      .as_ref()
      .expect("This chunk should have a filename");

    let importer_dir = current_chunk_filename.as_path().parent().unwrap();
    let importee_filename = asset_filename;
    let import_path = importee_filename.relative(importer_dir).as_path().expect_to_slash();

    first_arg_string_literal.value = self.snippet.atom(&import_path);
    None
  }

  /// try rewrite `foo_exports.bar` or `foo_exports['bar']`  to `bar` directly
  /// try rewrite `import.meta`
  fn try_rewrite_member_expr(
    &mut self,
    member_expr: &ast::MemberExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    match member_expr {
      MemberExpression::ComputedMemberExpression(inner_expr) => {
        if let Some((object_ref, props)) =
          self.ctx.linking_info.resolved_member_expr_refs.get(&inner_expr.span)
        {
          match object_ref {
            Some(object_ref) => {
              let object_ref_expr = self.finalized_expr_for_symbol_ref(*object_ref, false);

              let replaced_expr =
                self.snippet.member_expr_or_ident_ref(object_ref_expr, props, inner_expr.span);
              return Some(replaced_expr);
            }
            None => {
              return Some(self.snippet.member_expr_with_void_zero_object(props, inner_expr.span));
            }
          }
        }
        None
      }
      MemberExpression::StaticMemberExpression(inner_expr) => {
        if let Some((object_ref, props)) =
          self.ctx.linking_info.resolved_member_expr_refs.get(&inner_expr.span)
        {
          match object_ref {
            Some(object_ref) => {
              let object_ref_expr = self.finalized_expr_for_symbol_ref(*object_ref, false);

              let replaced_expr =
                self.snippet.member_expr_or_ident_ref(object_ref_expr, props, inner_expr.span);
              return Some(replaced_expr);
            }
            None => {
              return Some(self.snippet.member_expr_with_void_zero_object(props, inner_expr.span));
            }
          }
          // these two branch are exclusive since `import.meta` is a global member_expr
        } else if let Some(new_expr) = self.try_rewrite_import_meta_prop_expr(inner_expr) {
          return Some(new_expr);
        }
        None
      }
      MemberExpression::PrivateFieldExpression(_) => None,
    }
  }

  fn get_conflicted_info(
    &self,
    id: &BindingIdentifier<'ast>,
  ) -> Option<(SymbolId, &str, &rolldown_rstr::Rstr)> {
    let symbol_id = id.symbol_id.get()?;
    let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();
    let original_name = symbol_ref.name(self.ctx.symbol_db);
    let canonical_name = self.canonical_name_for(symbol_ref);
    (original_name != canonical_name.as_str()).then_some((symbol_id, original_name, canonical_name))
  }
}
