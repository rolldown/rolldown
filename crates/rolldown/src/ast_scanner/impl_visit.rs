use std::{borrow::Borrow, collections::hash_map::Entry, sync::Arc};

use oxc::{
  ast::{
    ast::{Argument, Expression, FormalParameter, IdentifierReference, MemberExpression},
    visit::walk,
    Visit,
  },
  codegen::{self, Codegen, CodegenOptions, Gen},
  semantic::SymbolId,
  span::CompactStr,
};
use rolldown_common::{ImportKind, SymbolRef};
use rolldown_error::BuildError;
use rustc_hash::FxHashSet;

use crate::utils::call_expression_ext::{CallExpressionExt, CallExpressionKind};

use super::{side_effect_detector::SideEffectDetector, AstScanner};

impl<'me, 'ast> Visit<'ast> for AstScanner<'me> {
  fn visit_program(&mut self, program: &oxc::ast::ast::Program<'ast>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx);
      self.current_stmt_info.side_effect =
        SideEffectDetector::new(self.scopes, self.source, self.trivias)
          .detect_side_effect_of_stmt(stmt);

      if cfg!(debug_assertions) {
        let mut codegen = Codegen::<false>::new(
          "",
          "",
          CodegenOptions {
            enable_typescript: true,
            enable_source_map: false,
            preserve_annotate_comments: false,
          },
          None,
        );
        stmt.gen(&mut codegen, codegen::Context::default());
        self.current_stmt_info.debug_label = Some(codegen.into_source_text());
      }

      self.visit_statement(stmt);
      self.result.stmt_infos.add_stmt_info(std::mem::take(&mut self.current_stmt_info));
    }
    // dbg!(&self.file_path);
    // dbg!(&self.dynamic_import_usage_collector);
  }

  fn visit_binding_identifier(&mut self, ident: &oxc::ast::ast::BindingIdentifier) {
    let symbol_id = ident.symbol_id.get().unwrap();
    if self.is_top_level(symbol_id) {
      self.add_declared_id(symbol_id);
    }
    self.try_diagnostic_forbid_const_assign(symbol_id);
  }

  fn visit_member_expression(&mut self, expr: &MemberExpression<'ast>) {
    let top_level_member_expr: Option<(SymbolId, Vec<CompactStr>)> = match expr {
      MemberExpression::ComputedMemberExpression(expr) => {
        self.visit_computed_member_expression(expr);
        None
      }
      MemberExpression::StaticMemberExpression(inner_expr) => {
        enum ExtractedSymbol {
          Other,
          TopLevel(SymbolId),
          DynamicImportModuleBinding,
        }
        let mut chain = vec![];
        let mut cur = inner_expr;
        let extracted_symbol = loop {
          chain.push(cur.property.clone());
          match &cur.object {
            Expression::StaticMemberExpression(expr) => {
              cur = expr;
            }
            Expression::Identifier(ident) => {
              let symbol_id = self.resolve_symbol_from_reference(ident);

              let collector = &mut self.dynamic_import_usage_collector;
              let res = if collector.in_import_then_body {
                if let (Some((span, symbol_ref)), Some(symbol_id)) =
                  (collector.dynamic_module_ref, symbol_id)
                {
                  if symbol_id == symbol_ref.symbol {
                    match collector.dynamic_import_usage_map.entry(span) {
                      Entry::Occupied(mut occ) => match occ.get_mut() {
                        super::DynamicImportUse::Partial(set) => {
                          set.insert(chain[chain.len() - 1].name.as_str().into());
                        }
                        super::DynamicImportUse::All => {
                          // Do nothing, partial || All = All
                        }
                      },
                      Entry::Vacant(vac) => {
                        let name = CompactStr::from(chain[chain.len() - 1].name.as_str());
                        vac.insert(super::DynamicImportUse::Partial(FxHashSet::from_iter([name])));
                      }
                    };
                    break ExtractedSymbol::DynamicImportModuleBinding;
                  }
                  break ExtractedSymbol::Other;
                }
                ExtractedSymbol::Other
              } else {
                let resolved_top_level = self.resolve_identifier_reference(symbol_id, ident);
                if let Some(id) = resolved_top_level {
                  ExtractedSymbol::TopLevel(id)
                } else {
                  ExtractedSymbol::Other
                }
              };
              break res;
            }
            _ => break ExtractedSymbol::Other,
          }
        };
        chain.reverse();
        let chain =
          chain.into_iter().map(|ident| CompactStr::from(ident.name.as_str())).collect::<Vec<_>>();
        match extracted_symbol {
          ExtractedSymbol::Other => {
            self.visit_expression(&cur.object);
            None
          }
          ExtractedSymbol::TopLevel(symbol_id) => Some((symbol_id, chain)),
          ExtractedSymbol::DynamicImportModuleBinding => None,
        }
      }
      MemberExpression::PrivateFieldExpression(expr) => {
        self.visit_private_field_expression(expr);
        None
      }
    };
    if let Some((symbol_id, chains)) = top_level_member_expr {
      self.add_member_expr_reference(symbol_id, chains);
    }
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference) {
    let symbol_id = self.resolve_symbol_from_reference(ident);
    let collector = &mut self.dynamic_import_usage_collector;
    if collector.in_import_then_body {
      if let (Some((span, symbol_ref)), Some(symbol_id)) = (collector.dynamic_module_ref, symbol_id)
      {
        if symbol_id == symbol_ref.symbol {
          // The whole namespace object ref has been used
          *self.dynamic_import_usage_collector.dynamic_import_usage_map.entry(span).or_default() =
            super::DynamicImportUse::All;
        }
      }
    }
    if let Some(resolved_symbol_id) = self.resolve_identifier_reference(symbol_id, ident) {
      self.add_referenced_symbol(resolved_symbol_id);
    };
  }

  fn visit_statement(&mut self, stmt: &oxc::ast::ast::Statement<'ast>) {
    if let Some(decl) = stmt.as_module_declaration() {
      self.scan_module_decl(decl);
    }
    walk::walk_statement(self, stmt);
  }

  fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(request) = &expr.source {
      let id = self.add_import_record(&request.value, ImportKind::DynamicImport);
      self.result.imports.insert(expr.span, id);
    }
    walk::walk_import_expression(self, expr);
  }

  fn visit_call_expression(&mut self, expr: &oxc::ast::ast::CallExpression<'ast>) {
    let call_expression_kind = expr.extract_call_expression_kind(self.scopes);
    if call_expression_kind.is_global_require() {
      if let Some(oxc::ast::ast::Argument::StringLiteral(request)) = &expr.arguments.first() {
        let id = self.add_import_record(&request.value, ImportKind::Require);
        self.result.imports.insert(expr.span, id);
      }
    }
    if let CallExpressionKind::ImportThen(import_expression_span) = call_expression_kind {
      match expr.arguments.as_slice() {
        // rest of the arguments are meaingless for a `Promise.then`
        [Argument::FunctionExpression(expr), ..] => match expr.params.items.as_slice() {
          [FormalParameter { pattern, .. }] => {
            self.dynamic_import_usage_collector.in_import_then_body = true;
          }
          _ => {}
        },
        [Argument::ArrowFunctionExpression(expr), ..] => match expr.params.items.as_slice() {
          [FormalParameter { pattern, .. }] => {
            match pattern.kind {
              oxc::ast::ast::BindingPatternKind::BindingIdentifier(ref id) => {
                id.symbol_id.get().inspect(|symbol_id| {
                  self.dynamic_import_usage_collector.dynamic_module_ref = Some((
                    import_expression_span,
                    SymbolRef { owner: self.idx, symbol: *symbol_id },
                  ));
                });
              }
              oxc::ast::ast::BindingPatternKind::ObjectPattern(_) => {
                // TODO: support this
                *self
                  .dynamic_import_usage_collector
                  .dynamic_import_usage_map
                  .entry(import_expression_span)
                  .or_default() = super::DynamicImportUse::All;
              }
              oxc::ast::ast::BindingPatternKind::ArrayPattern(_)
              | oxc::ast::ast::BindingPatternKind::AssignmentPattern(_) => {
                *self
                  .dynamic_import_usage_collector
                  .dynamic_import_usage_map
                  .entry(import_expression_span)
                  .or_default() = super::DynamicImportUse::All;
              }
            }
            self.dynamic_import_usage_collector.in_import_then_body = true;
            self
              .dynamic_import_usage_collector
              .dynamic_import_usage_map
              .entry(import_expression_span)
              .or_insert(super::DynamicImportUse::Partial(FxHashSet::default()));
            walk::walk_function_body(self, &expr.body);
            self.dynamic_import_usage_collector.in_import_then_body = false;
            self.dynamic_import_usage_collector.dynamic_module_ref = None;
          }
          _ => {}
        },
        _ => {}
      }
    }

    walk::walk_call_expression(self, expr);
  }
}

fn is_import_then_call() {}
