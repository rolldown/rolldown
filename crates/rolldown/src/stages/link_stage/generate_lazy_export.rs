use indexmap::map::Entry;
use oxc::{
  ast::ast::{self, Expression},
  semantic::{SemanticBuilder, Stats},
  span::SPAN,
};
use rolldown_common::{
  AstScopes, ESTarget, EcmaAstIdx, EcmaModuleAstUsage, ExportsKind, LocalExport, Module, ModuleIdx,
  ModuleType, NormalModule, StmtInfo, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRef,
  SymbolRefDbForModule,
};
use rolldown_ecmascript_utils::{AstSnippet, TakeIn};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::{
  concat_string,
  ecmascript::legitimize_identifier_name,
  indexmap::FxIndexMap,
  rayon::{IntoParallelRefMutIterator, ParallelIterator},
};

use super::LinkStage;

impl LinkStage<'_> {
  pub fn generate_lazy_export(&mut self) {
    let module_idx_to_exports_kind = append_only_vec::AppendOnlyVec::new();
    self.module_table.modules.par_iter_mut().for_each(|module| {
      let Module::Normal(module) = module else {
        return;
      };
      if !module.meta.has_lazy_export() {
        return;
      }
      let default_symbol_ref = module.default_export_ref;
      let is_json = matches!(module.module_type, ModuleType::Json);
      if !is_json || module.exports_kind == ExportsKind::CommonJs {
        update_module_default_export_info(module, default_symbol_ref, 1.into());
      }
      module_idx_to_exports_kind.push((module.ecma_ast_idx(), module.exports_kind, is_json));

      // generate `module.exports = expr`
      if module.exports_kind == ExportsKind::CommonJs {
        // since the wrap arguments are generate on demand, we need to insert the module ref usage here.
        module.stmt_infos.infos[StmtInfoIdx::new(1)].side_effect = true;
        module.ecma_view.ast_usage.insert(EcmaModuleAstUsage::ModuleRef);
      }
    });

    for (ast_idx, exports_kind, is_json_module) in module_idx_to_exports_kind {
      let Some((ecma_ast, module_idx)) = self.ast_table.get_mut(ast_idx) else { unreachable!() };
      let module_idx = *module_idx;
      if matches!(exports_kind, ExportsKind::CommonJs) {
        ecma_ast.program.with_mut(|fields| {
          let snippet = AstSnippet::new(fields.allocator);
          let Some(stmt) = fields.program.body.first_mut() else { unreachable!() };
          let expr = match stmt {
            ast::Statement::ExpressionStatement(stmt) => stmt.expression.take_in(snippet.alloc()),
            _ => {
              unreachable!()
            }
          };
          *stmt = snippet.module_exports_expr_stmt(expr);
        });
        continue;
      }
      // ExportsKind == Esm && ModuleType == Json
      if is_json_module {
        if json_object_expr_to_esm(self, module_idx, ast_idx) {
          continue;
        }
        // if json is not a ObjectExpression, we will fallback to normal esm lazy export transform
        let module = &mut self.module_table.modules[module_idx];
        let module = module.as_normal_mut().unwrap();
        update_module_default_export_info(module, module.default_export_ref, 1.into());
      }

      // shadowing the previous mutable ref, to avoid reference mutable ref twice at the same time.
      let Some((ecma_ast, _)) = self.ast_table.get_mut(ast_idx) else { unreachable!() };
      ecma_ast.program.with_mut(|fields| {
        let snippet = AstSnippet::new(fields.allocator);
        let Some(stmt) = fields.program.body.first_mut() else { unreachable!() };
        let expr = match stmt {
          ast::Statement::ExpressionStatement(stmt) => stmt.expression.take_in(snippet.alloc()),
          _ => {
            unreachable!()
          }
        };
        *stmt = snippet.export_default_expr_stmt(expr);
      });
    }
  }
}

fn update_module_default_export_info(
  module: &mut NormalModule,
  default_symbol_ref: SymbolRef,
  idx: StmtInfoIdx,
) {
  module
    .named_exports
    .insert("default".into(), LocalExport { span: SPAN, referenced: default_symbol_ref });
  // needs to support `preferConst`, so default statement may not be the second stmt info
  module.stmt_infos.declare_symbol_for_stmt(idx, default_symbol_ref);
}

#[allow(clippy::too_many_lines)]
/// return true if the json is a ObjectExpression
fn json_object_expr_to_esm(
  link_staged: &mut LinkStage,
  module_idx: ModuleIdx,
  ast_idx: EcmaAstIdx,
) -> bool {
  let target = link_staged.options.target;
  let module = &mut link_staged.module_table.modules[module_idx];
  let Module::Normal(module) = module else {
    return false;
  };

  let (ecma_ast, _) = &mut link_staged.ast_table[ast_idx];
  // (local, exported, legal_ident)
  let mut declaration_binding_names: Vec<(Rstr, Rstr, bool)> = vec![];
  let transformed = ecma_ast.program.with_mut(|fields| {
    let mut index_map = FxIndexMap::default();
    let snippet = AstSnippet::new(fields.allocator);
    let program = fields.program;
    let Some(stmts) = program.body.first_mut() else { unreachable!() };
    let expr = match stmts {
      ast::Statement::ExpressionStatement(stmt) => &mut stmt.expression,
      _ => {
        unreachable!()
      }
    };
    if !matches!(expr.without_parentheses(), Expression::ObjectExpression(_)) {
      return false;
    }
    let Expression::ObjectExpression(mut obj_expr) =
      snippet.expr_without_parentheses(expr.take_in(snippet.alloc()))
    else {
      unreachable!();
    };
    // clean program body, since we already take it and left a dummy expr
    snippet.builder.move_vec(&mut program.body);

    // convert {"a": "b", "c": "d"} to
    // {"a": b, "c": d}
    // and collect related info
    for property in obj_expr.properties.iter_mut() {
      match property {
        ast::ObjectPropertyKind::ObjectProperty(ref mut property) => {
          let key = property.key.static_name().expect("should be static name");
          if key.is_empty() {
            continue;
          }
          let legitimized_ident = legitimize_identifier_name(&key).to_rstr();
          let is_legal_ident = legitimized_ident.as_str() == key;
          declaration_binding_names.push((
            legitimized_ident.clone(),
            key.to_rstr(),
            is_legal_ident,
          ));

          let value = std::mem::replace(
            &mut property.value,
            snippet.builder.expression_identifier_reference(SPAN, legitimized_ident.as_str()),
          );
          if key == "__proto__" && !matches!(target, ESTarget::Es5) {
            property.computed = true;
          } else if is_legal_ident {
            property.shorthand = is_legal_ident;
            property.key = ast::PropertyKey::StaticIdentifier(
              snippet.builder.alloc_identifier_name(SPAN, legitimized_ident.as_ref()),
            );
          }
          match index_map.entry(legitimized_ident) {
            Entry::Occupied(mut occ) => {
              *occ.get_mut() = value;
            }
            Entry::Vacant(vac) => {
              vac.insert(value);
            }
          }
        }
        ast::ObjectPropertyKind::SpreadProperty(_) => unreachable!(),
      };
    }
    // recreate Json Module
    let stmts = index_map
      .into_iter()
      // declaration
      .map(|(local, v)| snippet.var_decl_stmt(local.as_str(), v))
      // export default json module
      .chain(std::iter::once(
        snippet.export_default_expr_stmt(Expression::ObjectExpression(obj_expr)),
      ))
      // export all declaration
      .chain(std::iter::once(
        snippet
          .statement_module_declaration_export_named_declaration(None, &declaration_binding_names),
      ));
    program.body.extend(stmts);
    true
  });

  if !transformed {
    return false;
  }

  // recreate semantic data
  #[allow(clippy::cast_possible_truncation)]
  let (symbol_table, scope) = ecma_ast.make_symbol_table_and_scope_tree_with_semantic_builder(
    SemanticBuilder::new().with_scope_tree_child_ids(true).with_stats(Stats {
      nodes: declaration_binding_names.len().next_power_of_two() as u32,
      scopes: 1,
      symbols: declaration_binding_names.len() as u32,
      references: declaration_binding_names.len() as u32 * 2u32,
    }),
  );

  // let default_symbol_ref = module.default_export_ref;

  // update semantic data of module
  let root_scope_id = scope.root_scope_id();
  let ast_scope = AstScopes::new(scope);
  let mut symbol_ref_db = SymbolRefDbForModule::new(symbol_table, module_idx, root_scope_id);

  let legitimized_repr_name = legitimize_identifier_name(&module.repr_name);
  let default_export_ref =
    symbol_ref_db.create_facade_root_symbol_ref(&concat_string!(legitimized_repr_name, "_default"));

  let name = concat_string!(legitimized_repr_name, "_exports");
  let namespace_object_ref = symbol_ref_db.create_facade_root_symbol_ref(&name);
  module.namespace_object_ref = namespace_object_ref;
  module.default_export_ref = default_export_ref;

  // update module stmts info
  // clear stmt info, since we need to split `ObjectExpression` into multiple decl, the original stmt info is invalid.
  // preserve the first one, which is `NamespaceRef`
  module.stmt_infos.drain(1.into()..);
  let mut all_declared_symbols = vec![];
  for (i, (local, exported, _)) in declaration_binding_names.iter().enumerate() {
    let symbol_id = ast_scope.get_root_binding(local.as_str()).expect("should have binding");
    let symbol_ref = (module_idx, symbol_id).into();
    all_declared_symbols.push(SymbolOrMemberExprRef::from(symbol_ref));
    let stmt_info = StmtInfo::default().with_stmt_idx(i).with_declared_symbols(vec![symbol_ref]);
    module.stmt_infos.add_stmt_info(stmt_info);
    module
      .named_exports
      .insert(exported.clone(), LocalExport { span: SPAN, referenced: symbol_ref });
  }
  // declare default export statement
  let stmt_info = StmtInfo::default()
    .with_stmt_idx(declaration_binding_names.len())
    .with_declared_symbols(vec![default_export_ref])
    .with_referenced_symbols(all_declared_symbols.clone());

  module.stmt_infos.add_stmt_info(stmt_info);
  module
    .named_exports
    .insert("default".into(), LocalExport { span: SPAN, referenced: default_export_ref });

  // declare namespace object statement
  module.exports_kind = ExportsKind::Esm;
  module.stmt_infos.replace_namespace_stmt_info(
    StmtInfo::default()
      .with_declared_symbols(vec![namespace_object_ref])
      .with_referenced_symbols(all_declared_symbols),
  );
  module.ecma_view.scope = ast_scope;
  link_staged.symbols.store_local_db(module_idx, symbol_ref_db);
  true
}
