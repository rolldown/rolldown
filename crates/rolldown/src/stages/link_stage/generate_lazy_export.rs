use indexmap::map::Entry;
use oxc::{
  ast::ast::{self, Expression, ObjectPropertyKind, Statement},
  index,
  semantic::SemanticBuilder,
  span::SPAN,
};
use rolldown_common::{
  AstScopes, EcmaAstIdx, EcmaModuleAstUsage, ExportsKind, LocalExport, Module, ModuleIdx,
  ModuleType, StmtInfo, StmtInfoIdx, SymbolOrMemberExprRef, SymbolRefDbForModule,
};
use rolldown_ecmascript::{EcmaAst, ToSourceString, WithMutFields};
use rolldown_ecmascript_utils::{AstSnippet, TakeIn};
use rolldown_rstr::{Rstr, ToRstr};
use rolldown_utils::{
  concat_string,
  ecmascript::legitimize_identifier_name,
  indexmap::FxIndexMap,
  rayon::{IntoParallelRefMutIterator, ParallelIterator},
};

use super::LinkStage;

impl<'link> LinkStage<'link> {
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
        module
          .named_exports
          .insert("default".into(), LocalExport { span: SPAN, referenced: default_symbol_ref });
        // needs to support `preferConst`, so default statement may not be the second stmt info
        module.stmt_infos.declare_symbol_for_stmt(1.into(), default_symbol_ref);
      }
      module_idx_to_exports_kind.push((module.ecma_ast_idx(), module.exports_kind, is_json));

      // generate `module.exports = expr`
      if module.exports_kind == ExportsKind::CommonJs {
        // since the wrap arguments are generate on demand, we need to insert the module ref usage here.
        module.stmt_infos.infos[StmtInfoIdx::new(1)].side_effect = true;
        module.ecma_view.ast_usage.insert(EcmaModuleAstUsage::ModuleRef);
      }
    });

    dbg!(&module_idx_to_exports_kind);
    for (ast_idx, exports_kind, is_json_module) in module_idx_to_exports_kind.into_iter() {
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
        json_object_expr_to_esm(self, module_idx, ast_idx);
        continue;
      }

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

fn json_object_expr_to_esm(
  link_staged: &mut LinkStage,
  module_idx: ModuleIdx,
  ast_idx: EcmaAstIdx,
) {
  let module = &mut link_staged.module_table.modules[module_idx];
  let Module::Normal(module) = module else {
    return;
  };

  let (ecma_ast, _) = &mut link_staged.ast_table[ast_idx];
  let mut body_len = 0;
  let mut declaration_binding_names = vec![];
  ecma_ast.program.with_mut(|fields| {
    let mut index_map = FxIndexMap::default();
    let snippet = AstSnippet::new(fields.allocator);
    let program = fields.program;
    println!("{}", program.to_source_string());
    let Some(stmts) = program.body.first_mut() else { unreachable!() };
    let expr = match stmts {
      ast::Statement::ExpressionStatement(stmt) => &mut stmt.expression,
      _ => {
        unreachable!()
      }
    };
    if !matches!(expr.without_parentheses(), Expression::ObjectExpression(_)) {
      return;
    }
    let Expression::ObjectExpression(mut obj_expr) =
      snippet.expr_without_parentheses(expr.take_in(snippet.alloc()))
    else {
      unreachable!();
    };
    // clearn program body, since we already take it and left a dummy expr
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
          let legal_ident = legitimize_identifier_name(&key).to_rstr();
          declaration_binding_names.push(legal_ident.clone());
          let value = std::mem::replace(
            &mut property.value,
            snippet.builder.expression_identifier_reference(SPAN, legal_ident.as_str()),
          );
          match index_map.entry(legal_ident) {
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
    // create declarations
    let stmts = index_map
      .into_iter()
      .map(|(k, v)| {
        let decl = snippet.decl_var_decl(k.as_str(), v);
        snippet.statement_module_declaration_export_named_declaration(decl)
      })
      .chain(std::iter::once(
        snippet.export_default_expr_stmt(Expression::ObjectExpression(obj_expr)),
      ))
      .collect::<Vec<_>>();
    program.body.extend(stmts);
    println!("{}", program.to_source_string());
    body_len = program.body.len();
  });

  // TODO: Stats
  // recreate semantic data
  let (mut symbol_table, scope) = ecma_ast.make_symbol_table_and_scope_tree();

  // let default_symbol_ref = module.default_export_ref;

  // update semantic data of module
  let root_scope_id = scope.root_scope_id();
  let ast_scope = AstScopes::new(
    scope,
    std::mem::take(&mut symbol_table.references),
    std::mem::take(&mut symbol_table.resolved_references),
  );
  let mut symbol_ref_db = SymbolRefDbForModule::new(symbol_table, module_idx, root_scope_id);

  let legitimized_repr_name = legitimize_identifier_name(&module.repr_name);
  let default_export_ref = symbol_ref_db
    .create_facade_root_symbol_ref(concat_string!(legitimized_repr_name, "_default").into());

  let name = concat_string!(legitimized_repr_name, "_exports");
  let namespace_object_ref = symbol_ref_db.create_facade_root_symbol_ref(name.into());
  module.namespace_object_ref = namespace_object_ref;
  module.default_export_ref = default_export_ref;
  // dbg!(&default_export_ref);
  // dbg!(&namespace_object_ref);

  // update module stmts info
  // clear stmt info, since we need to split `ObjectExpression` into multiple decl, the original stmt info is invalid.
  // preserve the first one, which is `NamespaceRef`
  module.stmt_infos.drain(1.into()..);
  let mut all_declared_symbols = vec![];
  for (i, name) in declaration_binding_names.iter().enumerate() {
    let symbol_id = ast_scope.get_root_binding(name.as_str()).expect("should have binding");
    let symbol_ref = (module_idx, symbol_id).into();
    all_declared_symbols.push(SymbolOrMemberExprRef::from(symbol_ref));
    let stmt_info = StmtInfo::default().with_stmt_idx(i).with_declared_symbols(vec![symbol_ref]);
    module.stmt_infos.add_stmt_info(stmt_info);
    module.named_exports.insert(name.clone(), LocalExport { span: SPAN, referenced: symbol_ref });
  }
  // dbg!(&module.named_exports);
  // declare default export statement
  // dbg!(&all_declared_symbols);
  let stmt_info = StmtInfo::default()
    .with_stmt_idx(declaration_binding_names.len())
    .with_declared_symbols(vec![default_export_ref])
    .with_referenced_symbols(all_declared_symbols);
  // dbg!(&stmt_info);
  module.stmt_infos.add_stmt_info(stmt_info);

  module
    .named_exports
    .insert("default".into(), LocalExport { span: SPAN, referenced: default_export_ref });

  module.ecma_view.scope = ast_scope;
  link_staged.symbols.store_local_db(module_idx, symbol_ref_db);

  // ecma_ast.program.with_mut(|fields| {
  //   fields.program.body = stmts;
  // });
}
