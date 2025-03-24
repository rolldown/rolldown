use oxc::{
  allocator::{self, Allocator, CloneIn, IntoIn},
  ast::{
    Comment, NONE,
    ast::{
      self, BindingIdentifier, ClassElement, Expression, IdentifierReference, ImportExpression,
      MemberExpression, Statement, VariableDeclarationKind,
    },
  },
  semantic::{ReferenceId, SymbolId},
  span::{Atom, GetSpan, SPAN},
};
use rolldown_common::{
  AstScopes, ExportsKind, ImportRecordIdx, ImportRecordMeta, Module, ModuleIdx, ModuleType,
  OutputFormat, Platform, SymbolRef, WrapKind,
};
use rolldown_ecmascript_utils::{
  AllocatorExt, AstSnippet, BindingPatternExt, CallExpressionExt, ExpressionExt, StatementExt,
  TakeIn,
};

mod finalizer_context;
mod impl_visit_mut;
pub use finalizer_context::ScopeHoistingFinalizerContext;
use rolldown_rstr::Rstr;
use rolldown_std_utils::OptionExt;
use rolldown_utils::ecmascript::is_validate_identifier_name;
use rustc_hash::FxHashSet;
use sugar_path::SugarPath;

mod hmr;
mod rename;

/// Finalizer for emitting output code with scope hoisting.
pub struct ScopeHoistingFinalizer<'me, 'ast> {
  pub ctx: ScopeHoistingFinalizerContext<'me>,
  pub scope: &'me AstScopes,
  pub alloc: &'ast Allocator,
  pub snippet: AstSnippet<'ast>,
  pub comments: oxc::allocator::Vec<'ast, Comment>,
  /// `SymbolRef` imported from a cjs module which has `namespace_alias`
  /// more details please refer [`rolldown_common::types::symbol_ref_db::SymbolRefDataClassic`].
  pub namespace_alias_symbol_id: FxHashSet<SymbolId>,
  /// All `ReferenceId` of `IdentifierReference` we are interested, the `IdentifierReference` should be the object of `MemberExpression` and the property is not
  /// a `"default"` property access
  pub interested_namespace_alias_ref_id: FxHashSet<ReferenceId>,
  pub generated_init_esm_importee_ids: FxHashSet<ModuleIdx>,
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

  pub fn finalized_expr_for_runtime_symbol(&self, name: &str) -> ast::Expression<'ast> {
    self.finalized_expr_for_symbol_ref(self.ctx.runtime.resolve_symbol(name), false, None)
  }

  fn try_get_valid_namespace_alias_ref_id_from_member_expr(
    &self,
    member_expr: &MemberExpression<'ast>,
  ) -> Option<ReferenceId> {
    let property_name = member_expr.static_property_name()?;
    if property_name == "default" {
      return None;
    }
    let ident_ref = match member_expr {
      MemberExpression::ComputedMemberExpression(expr) => expr.object.as_identifier()?,
      MemberExpression::StaticMemberExpression(expr) => expr.object.as_identifier()?,
      MemberExpression::PrivateFieldExpression(_) => return None,
    };

    let reference_id = ident_ref.reference_id.get()?;
    let symbol_id = self.scope.symbol_id_for(reference_id)?;
    if !self.namespace_alias_symbol_id.contains(&symbol_id) {
      return None;
    }
    Some(reference_id)
  }
  /// If return true the import stmt should be removed,
  /// or transform the import stmt to target form.
  fn transform_or_remove_import_export_stmt(
    &mut self,
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
        // Remove this statement

        return true;
      }
      // Replace the import statement with `init_foo()` if `ImportDeclaration` is not a plain import
      // or the importee have side effects.
      WrapKind::Esm => {
        if (rec.meta.contains(ImportRecordMeta::IS_PLAIN_IMPORT)
          && !importee.side_effects.has_side_effects())
          || self.generated_init_esm_importee_ids.contains(&importee.idx)
        {
          return true;
        };
        self.generated_init_esm_importee_ids.insert(importee.idx);
        // `init_foo`
        let wrapper_ref_expr = self.finalized_expr_for_symbol_ref(
          importee_linking_info.wrapper_ref.unwrap(),
          false,
          None,
        );

        // `init_foo()`
        let init_call =
          ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
            stmt.span(),
            wrapper_ref_expr,
            NONE,
            self.snippet.builder.vec(),
            false,
          ));

        if self.ctx.linking_info.is_tla_or_contains_tla_dependency {
          // `await init_foo()`
          *stmt = self.snippet.builder.statement_expression(
            SPAN,
            ast::Expression::AwaitExpression(
              self.snippet.builder.alloc_await_expression(SPAN, init_call),
            ),
          );
        } else {
          // `init_foo()`
          *stmt = self.snippet.builder.statement_expression(SPAN, init_call);
        }

        return false;
      }
    }
    true
  }

  fn finalized_expr_for_symbol_ref(
    &self,
    symbol_ref: SymbolRef,
    preserve_this_semantic_if_needed: bool,
    original_reference_id: Option<ReferenceId>,
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
            // instead of keeping `console.log(foo)` as we did in esm output. The reason here is we need to keep live binding in cjs output.

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
      let meta = &self.ctx.linking_infos[ns_alias.namespace_ref.owner];
      expr = if meta.safe_cjs_to_eliminate_interop_default
        && original_reference_id
          .is_some_and(|item| self.interested_namespace_alias_ref_id.contains(&item))
      {
        expr
      } else {
        ast::Expression::StaticMemberExpression(
          self.snippet.builder.alloc_static_member_expression(
            SPAN,
            expr,
            self.snippet.id_name(&ns_alias.property_name, SPAN),
            false,
          ),
        )
      };

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
            hoisted_names.push(id.name);
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
    let binding_name_for_namespace_object_ref =
      self.canonical_name_for(self.ctx.module.namespace_object_ref);
    // construct `var [binding_name_for_namespace_object_ref] = {}`
    let decl_stmt = self.snippet.var_decl_stmt(
      binding_name_for_namespace_object_ref,
      ast::Expression::ObjectExpression(TakeIn::dummy(self.alloc)),
    );

    let exports_len = self.ctx.linking_info.canonical_exports().count();

    let export_all_externals_rec_ids = &self.ctx.linking_info.star_exports_from_external_modules;

    let mut re_export_external_stmts: Option<_> = None;
    if !export_all_externals_rec_ids.is_empty() {
      // construct `__reExport(importer_exports, importee_exports)`
      let re_export_fn_ref = self.finalized_expr_for_runtime_symbol("__reExport");
      match self.ctx.options.format {
        OutputFormat::Esm => {
          let stmts = export_all_externals_rec_ids.iter().copied().flat_map(|idx| {
            let rec = &self.ctx.module.import_records[idx];
            // importee_exports
            let importee_namespace_name = self.canonical_name_for(rec.namespace_ref);
            let m = self.ctx.modules.get(rec.resolved_module);
            let Some(Module::External(module)) = m else {
              return vec![];
            };
            let importer_chunk = &self.ctx.chunk_graph.chunk_table[self.ctx.chunk_id];
            let importee_name = &module.get_import_path(importer_chunk);
            vec![
              // Insert `import * as ns from 'ext'`external module in esm format
              self.snippet.import_star_stmt(importee_name, importee_namespace_name),
              // Insert `__reExport(foo_exports, ns)`
              self.snippet.builder.statement_expression(
                SPAN,
                self.snippet.call_expr_with_2arg_expr(
                  re_export_fn_ref.clone_in(self.alloc),
                  self.snippet.id_ref_expr(binding_name_for_namespace_object_ref, SPAN),
                  self.snippet.id_ref_expr(importee_namespace_name, SPAN),
                ),
              ),
            ]
          });
          re_export_external_stmts = Some(stmts.collect::<Vec<_>>());
        }
        OutputFormat::Cjs | OutputFormat::Iife | OutputFormat::Umd => {
          let stmts = export_all_externals_rec_ids.iter().copied().map(|idx| {
            // Insert `__reExport(importer_exports, require('ext'))`
            let re_export_fn_ref = self.finalized_expr_for_runtime_symbol("__reExport");
            // importer_exports
            let importer_namespace_ref_expr =
              self.finalized_expr_for_symbol_ref(self.ctx.module.namespace_object_ref, false, None);
            let rec = &self.ctx.module.import_records[idx];
            let importee = &self.ctx.modules[rec.resolved_module];
            let expression = self.snippet.call_expr_with_2arg_expr(
              re_export_fn_ref,
              importer_namespace_ref_expr,
              self.snippet.call_expr_with_arg_expr_expr(
                "require",
                self.snippet.string_literal_expr(importee.id(), SPAN),
              ),
            );
            ast::Statement::ExpressionStatement(
              ast::ExpressionStatement { span: expression.span(), expression }.into_in(self.alloc),
            )
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
      let returned = self.finalized_expr_for_symbol_ref(resolved_export.symbol_ref, false, None);
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
    let export_call_expr = self.snippet.builder.expression_call(
      SPAN,
      self.finalized_expr_for_runtime_symbol("__export"),
      NONE,
      self.snippet.builder.vec_from_array([
        ast::Argument::from(self.snippet.id_ref_expr(binding_name_for_namespace_object_ref, SPAN)),
        ast::Argument::ObjectExpression(arg_obj_expr.into_in(self.alloc)),
      ]),
      false,
    );
    let export_call_stmt = self.snippet.builder.statement_expression(SPAN, export_call_expr);
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
              self.snippet.builder.expression_identifier(SPAN, "require"),
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
      return self.rewrite_rollup_file_url(property_name);
    }
    None
  }

  fn rewrite_rollup_file_url(&self, property_name: &str) -> Option<Expression<'ast>> {
    // rewrite `import.meta.ROLLUP_FILE_URL_<referenceId>`
    if let Some(reference_id) = property_name.strip_prefix("ROLLUP_FILE_URL_") {
      // compute relative path from chunk to asset
      let Ok(asset_file_name) = self.ctx.file_emitter.get_file_name(reference_id) else {
        return None;
      };
      let absolute_asset_file_name = asset_file_name
        .absolutize_with(self.ctx.options.cwd.as_path().join(&self.ctx.options.out_dir));
      let relative_asset_path = &self.ctx.chunk_graph.chunk_table[self.ctx.chunk_id]
        .relative_path_for(&absolute_asset_file_name);

      // new URL({relative_asset_path}, import.meta.url).href
      // TODO: needs import.meta.url polyfill for non esm
      let new_expr = ast::Expression::StaticMemberExpression(
        self.snippet.builder.alloc_static_member_expression(
          SPAN,
          self.snippet.builder.expression_new(
            SPAN,
            self.snippet.builder.expression_identifier(SPAN, "URL"),
            self.snippet.builder.vec_from_array([
              ast::Argument::StringLiteral(self.snippet.builder.alloc_string_literal(
                SPAN,
                relative_asset_path,
                None,
              )),
              ast::Argument::StaticMemberExpression(
                self.snippet.builder.alloc_static_member_expression(
                  SPAN,
                  self.snippet.builder.expression_meta_property(
                    SPAN,
                    self.snippet.builder.identifier_name(SPAN, "import"),
                    self.snippet.builder.identifier_name(SPAN, "meta"),
                  ),
                  self.snippet.builder.identifier_name(SPAN, "url"),
                  false,
                ),
              ),
            ]),
            NONE,
          ),
          self.snippet.builder.identifier_name(SPAN, "href"),
          false,
        ),
      );
      return Some(new_expr);
    }
    None
  }

  pub fn handle_new_url_with_string_literal_and_import_meta_url(
    &self,
    expr: &mut ast::NewExpression<'ast>,
  ) -> Option<()> {
    let &rec_idx = self.ctx.module.new_url_references.get(&expr.span())?;
    let rec = &self.ctx.module.import_records[rec_idx];
    let is_callee_global_url = matches!(expr.callee.as_identifier(), Some(ident) if ident.name == "URL" && self.is_global_identifier_reference(ident));

    if !is_callee_global_url {
      return None;
    }

    let is_second_arg_import_meta_url = expr
      .arguments
      .get(1)
      .is_some_and(|arg| arg.as_expression().is_some_and(ExpressionExt::is_import_meta_url));

    if !is_second_arg_import_meta_url {
      return None;
    }

    let first_arg_string_literal = expr.arguments.first_mut().and_then(|arg| match arg {
      ast::Argument::StringLiteral(string_literal) => Some(string_literal),
      _ => None,
    })?;

    let importee = &self.ctx.modules[rec.resolved_module].as_normal()?;
    let chunk_idx = &self.ctx.chunk_graph.module_to_chunk[importee.idx]?;
    let chunk = &self.ctx.chunk_graph.chunk_table[*chunk_idx];
    let asset_filename = &chunk.asset_absolute_preliminary_filenames[&importee.idx];
    let import_path = self.ctx.chunk_graph.chunk_table[self.ctx.chunk_id]
      .relative_path_for(asset_filename.as_path());

    first_arg_string_literal.value = self.snippet.atom(&import_path);
    None
  }

  /// try rewrite `foo_exports.bar` or `foo_exports['bar']`  to `bar` directly
  /// try rewrite `import.meta`
  fn try_rewrite_member_expr(
    &self,
    member_expr: &ast::MemberExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    match member_expr {
      MemberExpression::ComputedMemberExpression(inner_expr) => {
        if let Some((object_ref, props)) =
          self.ctx.linking_info.resolved_member_expr_refs.get(&inner_expr.span)
        {
          match object_ref {
            Some(object_ref) => {
              let object_ref_expr = self.finalized_expr_for_symbol_ref(*object_ref, false, None);

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
        match self.ctx.linking_info.resolved_member_expr_refs.get(&inner_expr.span) {
          Some((object_ref, props)) => {
            match object_ref {
              Some(object_ref) => {
                let object_ref_expr = self.finalized_expr_for_symbol_ref(*object_ref, false, None);

                let replaced_expr =
                  self.snippet.member_expr_or_ident_ref(object_ref_expr, props, inner_expr.span);
                return Some(replaced_expr);
              }
              None => {
                return Some(
                  self.snippet.member_expr_with_void_zero_object(props, inner_expr.span),
                );
              }
            }
            // these two branch are exclusive since `import.meta` is a global member_expr
          }
          _ => {
            if let Some(new_expr) = self.try_rewrite_import_meta_prop_expr(inner_expr) {
              return Some(new_expr);
            }
          }
        }
        None
      }
      MemberExpression::PrivateFieldExpression(_) => None,
    }
  }

  fn get_conflicted_info(
    &self,
    id: &BindingIdentifier<'ast>,
  ) -> Option<(&str, &rolldown_rstr::Rstr)> {
    let symbol_id = id.symbol_id.get()?;
    let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();
    let original_name = symbol_ref.name(self.ctx.symbol_db);
    let canonical_name = self.canonical_name_for(symbol_ref);
    (original_name != canonical_name.as_str()).then_some((original_name, canonical_name))
  }

  /// rewrite toplevel `class ClassName {}` to `var ClassName = class {}`
  fn get_transformed_class_decl(
    &self,
    class: &mut allocator::Box<'ast, ast::Class<'ast>>,
  ) -> Option<ast::Declaration<'ast>> {
    let scope_id = class.scope_id.get()?;

    if self.scope.scoping().scope_parent_id(scope_id) != Some(self.scope.scoping().root_scope_id())
    {
      return None;
    };

    let id = class.id.take()?;

    if let Some(symbol_id) = id.symbol_id.get() {
      if self.ctx.module.self_referenced_class_decl_symbol_ids.contains(&symbol_id) {
        // class T { static a = new T(); }
        // needs to rewrite to `var T = class T { static a = new T(); }`
        let mut id = id.clone();
        let new_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
        id.name = self.snippet.atom(new_name);
        class.id = Some(id);
      }
    }
    Some(self.snippet.builder.declaration_variable(
      SPAN,
      VariableDeclarationKind::Var,
      self.snippet.builder.vec1(self.snippet.builder.variable_declarator(
        SPAN,
        VariableDeclarationKind::Var,
        self.snippet.builder.binding_pattern(
          ast::BindingPatternKind::BindingIdentifier(self.snippet.builder.alloc(id)),
          NONE,
          false,
        ),
        Some(Expression::ClassExpression(class.take_in(self.alloc))),
        false,
      )),
      false,
    ))
  }

  #[allow(clippy::too_many_lines, clippy::collapsible_else_if)]
  fn try_rewrite_global_require_call(
    &self,
    call_expr: &mut ast::CallExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    if call_expr.is_global_require_call(self.scope) && !call_expr.span.is_unspanned() {
      //  `require` calls that can't be recognized by rolldown are ignored in scanning, so they were not stored in `NormalModule#imports`.
      //  we just keep these `require` calls as it is
      if let Some(rec_id) = self.ctx.module.imports.get(&call_expr.span).copied() {
        let rec = &self.ctx.module.import_records[rec_id];
        // use `__require` instead of `require`
        if rec.meta.contains(ImportRecordMeta::CALL_RUNTIME_REQUIRE) {
          *call_expr.callee.get_inner_expression_mut() =
            self.finalized_expr_for_runtime_symbol("__require");
        }
        let rewrite_ast = match &self.ctx.modules[rec.resolved_module] {
          Module::Normal(importee) => {
            match importee.module_type {
              ModuleType::Json => {
                // Nodejs treats json files as an esm module with a default export and rolldown follows this behavior.
                // And to make sure the runtime behavior is correct, we need to rewrite `require('xxx.json')` to `require('xxx.json').default` to align with the runtime behavior of nodejs.

                // Rewrite `require(...)` to `require_xxx(...)` or `(init_xxx(), __toCommonJS(xxx_exports).default)`
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];
                let wrap_ref_expr = self.finalized_expr_for_symbol_ref(
                  importee_linking_info.wrapper_ref.unwrap(),
                  false,
                  None,
                );
                if matches!(importee.exports_kind, ExportsKind::CommonJs) {
                  Some(ast::Expression::CallExpression(
                    self.snippet.alloc_simple_call_expr(wrap_ref_expr),
                  ))
                } else {
                  let ns_name =
                    self.finalized_expr_for_symbol_ref(importee.namespace_object_ref, false, None);
                  let to_commonjs_ref_name = self.finalized_expr_for_runtime_symbol("__toCommonJS");
                  Some(
                    self.snippet.seq2_in_paren_expr(
                      ast::Expression::CallExpression(
                        self.snippet.alloc_simple_call_expr(wrap_ref_expr),
                      ),
                      ast::Expression::StaticMemberExpression(
                        ast::StaticMemberExpression {
                          object: self.snippet.call_expr_with_arg_expr(
                            to_commonjs_ref_name,
                            ns_name,
                            false,
                          ),
                          property: self.snippet.id_name("default", SPAN),
                          ..TakeIn::dummy(self.alloc)
                        }
                        .into_in(self.alloc),
                      ),
                    ),
                  )
                }
              }
              _ => {
                // Rewrite `require(...)` to `require_xxx(...)` or `(init_xxx(), __toCommonJS(xxx_exports))`
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];

                // `init_xxx`
                let wrap_ref_expr = self.finalized_expr_for_symbol_ref(
                  importee_linking_info.wrapper_ref.unwrap(),
                  false,
                  None,
                );

                // `init_xxx()`
                let wrap_ref_call_expr =
                  ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
                    SPAN,
                    wrap_ref_expr,
                    NONE,
                    self.snippet.builder.vec(),
                    false,
                  ));

                if matches!(importee.exports_kind, ExportsKind::CommonJs)
                  || rec.meta.contains(ImportRecordMeta::IS_REQUIRE_UNUSED)
                {
                  // `init_xxx()`
                  Some(wrap_ref_call_expr)
                } else {
                  // `xxx_exports`
                  let namespace_object_ref_expr =
                    self.finalized_expr_for_symbol_ref(importee.namespace_object_ref, false, None);
                  // `__toCommonJS`
                  let to_commonjs_expr = self.finalized_expr_for_runtime_symbol("__toCommonJS");
                  // `__toCommonJS(xxx_exports)`
                  let to_commonjs_call_expr =
                    ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
                      SPAN,
                      to_commonjs_expr,
                      NONE,
                      self.snippet.builder.vec1(ast::Argument::from(namespace_object_ref_expr)),
                      false,
                    ));

                  // `(init_xxx(), __toCommonJS(xxx_exports))`
                  Some(self.snippet.seq2_in_paren_expr(wrap_ref_call_expr, to_commonjs_call_expr))
                }
              }
            }
          }
          Module::External(importee) => {
            let request_path =
              call_expr.arguments.get_mut(0).expect("require should have an argument");
            let importer_chunk = &self.ctx.chunk_graph.chunk_table[self.ctx.chunk_id];
            // Rewrite `require('xxx')` to `require('fs')`, if there is an alias that maps 'xxx' to 'fs'
            *request_path = ast::Argument::StringLiteral(self.snippet.alloc_string_literal(
              &importee.get_import_path(importer_chunk),
              request_path.span(),
            ));
            None
          }
        };
        return rewrite_ast;
      }
    }
    None
  }

  fn try_rewrite_inline_dynamic_import_expr(
    &self,
    import_expr: &ImportExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    let rec_id = self.ctx.module.imports.get(&import_expr.span)?;
    let rec = &self.ctx.module.import_records[*rec_id];
    let importee_id = rec.resolved_module;

    if rec.meta.contains(ImportRecordMeta::DEAD_DYNAMIC_IMPORT) {
      return Some(
        self.snippet.promise_resolve_then_call_expr(
          SPAN,
          self
            .snippet
            .builder
            .vec1(self.snippet.return_stmt(self.snippet.object_freeze_dynamic_import_polyfill())),
        ),
      );
    }
    if self.ctx.options.inline_dynamic_imports {
      match &self.ctx.modules[importee_id] {
        Module::Normal(importee) => {
          let importee_linking_info = &self.ctx.linking_infos[importee_id];
          let new_expr = match importee_linking_info.wrap_kind {
            WrapKind::Esm => {
              // Rewrite `import('./foo.mjs')` to `(init_foo(), foo_exports)`
              let importee_linking_info = &self.ctx.linking_infos[importee_id];

              // `init_foo`
              let importee_wrapper_ref_name =
                self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());

              // `foo_exports`
              let importee_namespace_name = self.canonical_name_for(importee.namespace_object_ref);

              // `(init_foo(), foo_exports)`
              Some(self.snippet.promise_resolve_then_call_expr(
                import_expr.span,
                self.snippet.builder.vec1(self.snippet.return_stmt(
                  self.snippet.seq2_in_paren_expr(
                    self.snippet.call_expr_expr(importee_wrapper_ref_name),
                    self.snippet.id_ref_expr(importee_namespace_name, SPAN),
                  ),
                )),
              ))
            }
            WrapKind::Cjs => {
              //  `__toESM(require_foo())`
              let to_esm_fn_name = self.canonical_name_for_runtime("__toESM");
              let importee_wrapper_ref_name =
                self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());

              Some(self.snippet.promise_resolve_then_call_expr(
                import_expr.span,
                self.snippet.builder.vec1(self.snippet.return_stmt(self.snippet.wrap_with_to_esm(
                  self.snippet.builder.expression_identifier(SPAN, to_esm_fn_name.as_str()),
                  self.snippet.call_expr_expr(importee_wrapper_ref_name),
                  self.ctx.module.should_consider_node_esm_spec(),
                ))),
              ))
            }
            WrapKind::None => {
              // The nature of `import()` is to load the module dynamically/lazily, so imported modules would
              // must be wrapped, so we could make sure the module is executed lazily.
              if cfg!(debug_assertions) {
                unreachable!()
              }
              None
            }
          };
          return new_expr;
        }
        Module::External(_) => {
          // iife format doesn't support external module
        }
      }
    }
    if matches!(self.ctx.options.format, OutputFormat::Cjs) {
      // Convert `import('./foo.mjs')` to `Promise.resolve().then(function() { return require('foo.mjs') })`
      let rec_id = self.ctx.module.imports.get(&import_expr.span)?;
      let rec = &self.ctx.module.import_records[*rec_id];
      let importee_id = rec.resolved_module;
      match &self.ctx.modules[importee_id] {
        Module::Normal(_importee) => {
          let importer_chunk = &self.ctx.chunk_graph.chunk_table[self.ctx.chunk_id];
          let importee_chunk_id = self.ctx.chunk_graph.entry_module_to_entry_chunk[&importee_id];
          let importee_chunk = &self.ctx.chunk_graph.chunk_table[importee_chunk_id];
          let import_path = importer_chunk.import_path_for(importee_chunk);
          let new_expr = self.snippet.promise_resolve_then_call_expr(
            import_expr.span,
            self.snippet.builder.vec1(ast::Statement::ReturnStatement(
              self.snippet.builder.alloc_return_statement(
                SPAN,
                Some(ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
                  SPAN,
                  self.snippet.builder.expression_identifier(SPAN, "require"),
                  NONE,
                  self.snippet.builder.vec1(ast::Argument::StringLiteral(
                    self.snippet.alloc_string_literal(&import_path, import_expr.span),
                  )),
                  false,
                ))),
              ),
            )),
          );
          return Some(new_expr);
        }
        Module::External(_) => {
          // For `import('external')`, we just keep it as it is to preserve user's intention
        }
      }
    }
    None
  }

  #[allow(clippy::too_many_lines)]
  fn remove_unused_top_level_stmt(&mut self, program: &mut ast::Program<'ast>) {
    let old_body = self.alloc.take(&mut program.body);
    // the first statement info is the namespace variable declaration
    // skip first statement info to make sure `program.body` has same index as `stmt_infos`
    old_body.into_iter().enumerate().zip(self.ctx.module.stmt_infos.iter().skip(1)).for_each(
      |((_top_stmt_idx, mut top_stmt), stmt_info)| {
        debug_assert!(matches!(stmt_info.stmt_idx, Some(_top_stmt_idx)));
        if !stmt_info.is_included {
          return;
        }

        if let Some(import_decl) = top_stmt.as_import_declaration() {
          let rec_id = self.ctx.module.imports[&import_decl.span];
          if self.transform_or_remove_import_export_stmt(&mut top_stmt, rec_id) {
            return;
          }
        } else if let Some(export_all_decl) = top_stmt.as_export_all_declaration() {
          let rec_id = self.ctx.module.imports[&export_all_decl.span];
          // "export * as ns from 'path'"
          if let Some(_alias) = &export_all_decl.exported {
            if self.transform_or_remove_import_export_stmt(&mut top_stmt, rec_id) {
              return;
            }
          } else {
            // "export * from 'path'"
            let rec = &self.ctx.module.import_records[rec_id];
            match &self.ctx.modules[rec.resolved_module] {
              Module::Normal(importee) => {
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];
                if matches!(importee_linking_info.wrap_kind, WrapKind::Esm) {
                  let wrapper_ref_name =
                    self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
                  program.body.push(self.snippet.call_expr_stmt(wrapper_ref_name));
                }

                match importee.exports_kind {
                  ExportsKind::Esm => {
                    if importee_linking_info.has_dynamic_exports {
                      let re_export_fn_ref = self.finalized_expr_for_runtime_symbol("__reExport");
                      // exports
                      let importer_namespace_ref = self.finalized_expr_for_symbol_ref(
                        self.ctx.module.namespace_object_ref,
                        false,
                        None,
                      );
                      // otherExports
                      let importee_namespace_ref = self.finalized_expr_for_symbol_ref(
                        importee.namespace_object_ref,
                        false,
                        None,
                      );
                      // __reExport(exports, otherExports)
                      let expression = self.snippet.call_expr_with_2arg_expr(
                        re_export_fn_ref,
                        importer_namespace_ref,
                        importee_namespace_ref,
                      );
                      let stmt = ast::Statement::ExpressionStatement(
                        ast::ExpressionStatement { span: expression.span(), expression }
                          .into_in(self.alloc),
                      );
                      program.body.push(stmt);
                    }
                  }
                  ExportsKind::CommonJs => {
                    let re_export_fn_name = self.finalized_expr_for_runtime_symbol("__reExport");

                    // importer_exports
                    let importer_namespace_ref = self.finalized_expr_for_symbol_ref(
                      self.ctx.module.namespace_object_ref,
                      false,
                      None,
                    );

                    // __toESM
                    let to_esm_fn_ref = self.finalized_expr_for_runtime_symbol("__toESM");

                    // require_foo
                    let importee_wrapper_ref_expr = self.finalized_expr_for_symbol_ref(
                      importee_linking_info.wrapper_ref.unwrap(),
                      false,
                      None,
                    );

                    // __reExport(importer_exports, __toESM(require_foo()))
                    program.body.push(ast::Statement::ExpressionStatement(
                      ast::ExpressionStatement {
                        span: SPAN,
                        expression: self.snippet.call_expr_with_2arg_expr_expr(
                          re_export_fn_name,
                          importer_namespace_ref,
                          self.snippet.wrap_with_to_esm(
                            to_esm_fn_ref,
                            ast::Expression::CallExpression(
                              self.snippet.builder.alloc_call_expression(
                                SPAN,
                                importee_wrapper_ref_expr,
                                NONE,
                                self.snippet.builder.vec(),
                                false,
                              ),
                            ),
                            self.ctx.module.should_consider_node_esm_spec(),
                          ),
                        ),
                      }
                      .into_in(self.alloc),
                    ));
                  }
                  ExportsKind::None => {}
                }
              }
              Module::External(_importee) => {
                match self.ctx.options.format {
                  rolldown_common::OutputFormat::Esm
                  | rolldown_common::OutputFormat::Iife
                  | rolldown_common::OutputFormat::Umd
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
            if self.transform_or_remove_import_export_stmt(&mut top_stmt, rec_id) {
              return;
            }
          }
        }

        program.body.push(top_stmt);
      },
    );
  }

  fn process_fn(
    &mut self,
    symbol_binding_id: Option<&BindingIdentifier<'ast>>,
    name_binding_id: Option<&BindingIdentifier<'ast>>,
  ) -> Option<()> {
    if !self.ctx.options.keep_names {
      return None;
    }
    let (original_name, _) = self.get_conflicted_info(name_binding_id.as_ref()?)?;
    let (_, canonical_name) = self.get_conflicted_info(symbol_binding_id.as_ref()?)?;
    let original_name: Rstr = original_name.into();
    let new_name = canonical_name.clone();
    let insert_position = self.ctx.cur_stmt_index + 1;
    self.ctx.keep_name_statement_to_insert.push((insert_position, original_name, new_name));
    None
  }

  fn keep_name_helper_for_class(
    &self,
    id: Option<&BindingIdentifier<'ast>>,
  ) -> Option<ClassElement<'ast>> {
    if !self.ctx.options.keep_names {
      return None;
    }
    let (original_name, _) = self.get_conflicted_info(id.as_ref()?)?;
    let original_name: Rstr = original_name.into();
    Some(self.snippet.static_block_keep_name_helper(&original_name))
  }

  fn generate_esm_namespace_in_cjs(&self) -> Vec<ast::Statement<'ast>> {
    let mut var_init_stmts = vec![];

    if let Some(esm_ns) = &self.ctx.module.esm_namespace_in_cjs {
      if self.ctx.module.stmt_infos[esm_ns.stmt_info_idx].is_included {
        // `__toESM`
        let to_esm_fn_name = self.finalized_expr_for_symbol_ref(
          self.canonical_ref_for_runtime("__toESM"),
          false,
          None,
        );

        // `require_foo`
        let importee_wrapper_ref_name = self.finalized_expr_for_symbol_ref(
          self.ctx.linking_info.wrapper_ref.unpack(),
          false,
          None,
        );

        // var import_foo = __toESM(require_foo())
        let declarations = self.snippet.builder.vec1(self.snippet.builder.variable_declarator(
          SPAN,
          ast::VariableDeclarationKind::Var,
          self.snippet.builder.binding_pattern(
            self.snippet.builder.binding_pattern_kind_binding_identifier(
              SPAN,
              self.canonical_name_for(esm_ns.namespace_ref).as_str(),
            ),
            NONE,
            false,
          ),
          // __toESM(require_foo())
          Some(self.snippet.wrap_with_to_esm(
            to_esm_fn_name,
            self.snippet.builder.expression_call(
              SPAN,
              importee_wrapper_ref_name,
              NONE,
              self.snippet.builder.vec(),
              false,
            ),
            false,
          )),
          false,
        ));

        let var_init =
          ast::Statement::VariableDeclaration(self.snippet.builder.alloc_variable_declaration(
            SPAN,
            ast::VariableDeclarationKind::Var,
            declarations,
            false,
          ));

        var_init_stmts.push(var_init);
      }
    };
    if let Some(esm_ns) = &self.ctx.module.esm_namespace_in_cjs_node_mode {
      if self.ctx.module.stmt_infos[esm_ns.stmt_info_idx].is_included {
        // `__toESM`
        let to_esm_fn_name = self.finalized_expr_for_symbol_ref(
          self.canonical_ref_for_runtime("__toESM"),
          false,
          None,
        );

        // `require_foo`
        let importee_wrapper_ref_name = self.finalized_expr_for_symbol_ref(
          self.ctx.linking_info.wrapper_ref.unpack(),
          false,
          None,
        );

        // var import_foo = __toESM(require_foo())
        let declarations = self.snippet.builder.vec1(self.snippet.builder.variable_declarator(
          SPAN,
          ast::VariableDeclarationKind::Var,
          self.snippet.builder.binding_pattern(
            self.snippet.builder.binding_pattern_kind_binding_identifier(
              SPAN,
              self.canonical_name_for(esm_ns.namespace_ref).as_str(),
            ),
            NONE,
            false,
          ),
          // __toESM(require_foo())
          Some(self.snippet.wrap_with_to_esm(
            to_esm_fn_name,
            self.snippet.builder.expression_call(
              SPAN,
              importee_wrapper_ref_name,
              NONE,
              self.snippet.builder.vec(),
              false,
            ),
            true,
          )),
          false,
        ));

        let var_init =
          ast::Statement::VariableDeclaration(self.snippet.builder.alloc_variable_declaration(
            SPAN,
            ast::VariableDeclarationKind::Var,
            declarations,
            false,
          ));

        var_init_stmts.push(var_init);
      }
    };
    var_init_stmts
  }
}
