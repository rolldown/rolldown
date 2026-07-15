use indexmap::map::Entry;
use oxc::allocator::GetAllocator;
use oxc::{
  allocator::{Allocator, TakeIn},
  ast::ast::{self, Expression, PropertyKind},
  semantic::{SemanticBuilder, Stats},
  span::SPAN,
};
use oxc_str::CompactStr;
use rolldown_common::{
  EcmaModuleAstUsage, ExportsKind, GetLocalDb, GetLocalDbMut, LocalExport, ModuleIdx, ModuleTable,
  ModuleType, NormalModule, StmtEvalFlags, StmtInfo, StmtInfoIdx, StmtInfoMeta, StmtInfos,
  SymbolOrMemberExprRef, SymbolRef, SymbolRefDb, SymbolRefDbForModule, SymbolRefFlags,
  TaggedSymbolRef,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_ecmascript_utils::AstFactory;
use rolldown_error::BuildDiagnostic;
use rolldown_utils::{
  IndexBitSet, ecmascript::legitimize_json_local_binding_name, indexmap::FxIndexMap,
};
use rustc_hash::FxHashSet;
use smallvec::smallvec;

use crate::type_alias::{IndexEcmaAst, IndexStmtInfos};

use super::{
  lazy_json_export_initializers::{
    LazyJsonExportInitializer, LazyJsonExportInitializers, LazyJsonModuleExportInitializers,
  },
  non_splittable_json_defaults::NonSplittableJsonDefaults,
  passes::{ModuleFormatsDraft, WrapperDeclaration, WrapperDeclarationsDraft},
};

/// Index of the first statement after the namespace statement (index 0).
const FIRST_TOP_LEVEL_STMT_IDX: StmtInfoIdx = StmtInfoIdx::from_raw_unchecked(1);

pub(super) fn normalize_lazy_exports(
  module_table: &mut ModuleTable,
  ast_table: &mut IndexEcmaAst,
  stmt_infos: &mut IndexStmtInfos,
  symbols: &mut SymbolRefDb,
  module_formats: &mut ModuleFormatsDraft,
  wrapper_declarations: &mut WrapperDeclarationsDraft,
  protected_identity_owners: &IndexBitSet<ModuleIdx>,
) -> (LazyJsonExportInitializers, NonSplittableJsonDefaults, Vec<BuildDiagnostic>) {
  let mut lazy_json_export_initializers = LazyJsonExportInitializers::default();
  let mut non_splittable_json_defaults = NonSplittableJsonDefaults::default();
  let mut diagnostics = Vec::new();
  for module_idx in module_table.modules.indices() {
    let Some((has_lazy_export, is_json)) = module_table[module_idx].as_normal().map(|module| {
      (module.meta.has_lazy_export(), std::matches!(module.module_type, ModuleType::Json))
    }) else {
      continue;
    };
    if !has_lazy_export {
      continue;
    }
    let Some(exports_kind) = module_formats.get(module_idx) else { continue };

    if is_json
      && exports_kind != ExportsKind::CommonJs
      && json_object_expr_to_esm(
        module_table,
        ast_table,
        stmt_infos,
        symbols,
        module_formats,
        wrapper_declarations,
        JsonRebuildCandidate { protected_identity_owners, module_idx },
      )
    {
      continue;
    }

    let Some(ecma_ast) = &ast_table[module_idx] else {
      if let Some(module) = module_table[module_idx].as_normal() {
        diagnostics.push(lazy_export_invariant_diagnostic(module, "the module has no AST"));
      }
      continue;
    };
    let (body_index, stmt_info_idx) =
      match find_lazy_export_payload_statement(ecma_ast, &stmt_infos[module_idx]) {
        Ok(payload) => payload,
        Err(reason) => {
          if let Some(module) = module_table[module_idx].as_normal() {
            diagnostics.push(lazy_export_invariant_diagnostic(module, reason));
          }
          continue;
        }
      };
    let Some(ecma_ast) = &mut ast_table[module_idx] else { continue };
    let json_fallback = if is_json {
      prepare_json_object_fallback(ecma_ast, body_index)
    } else {
      JsonObjectFallback::default()
    };
    let wrap = if exports_kind == ExportsKind::CommonJs {
      LazyExportWrap::CjsExport
    } else {
      LazyExportWrap::EsmDefault
    };
    if !replace_lazy_expression_statement(ecma_ast, body_index, wrap) {
      let Some(module) = module_table[module_idx].as_normal() else { continue };
      diagnostics.push(lazy_export_invariant_diagnostic(
        module,
        "the selected payload statement could not be replaced",
      ));
      continue;
    }
    let Some(module) = module_table[module_idx].as_normal_mut() else { continue };
    let default_symbol_ref = module.default_export_ref;
    update_module_default_export_info(
      module,
      &mut stmt_infos[module_idx],
      stmt_info_idx,
      default_symbol_ref,
    );
    if is_json && exports_kind != ExportsKind::CommonJs {
      let initializers = add_json_named_export_bindings(
        module,
        &mut stmt_infos[module_idx],
        symbols,
        json_fallback.named_exports,
      );
      if !json_fallback.default_member_reads_splittable {
        non_splittable_json_defaults.insert(default_symbol_ref);
      }
      if !initializers.is_empty() {
        lazy_json_export_initializers
          .record(module_idx, LazyJsonModuleExportInitializers::new(stmt_info_idx, initializers));
      }
    }

    if exports_kind == ExportsKind::CommonJs {
      if let Some(stmt_info) = stmt_infos[module_idx].infos.get_mut(stmt_info_idx) {
        stmt_info.eval_flags = true.into();
      }
      module.ecma_view.ast_usage.insert(EcmaModuleAstUsage::ModuleRef);
      continue;
    }

    // Ensure exports_kind is set to Esm for all modules that generate ESM export syntax.
    // This is needed for proper CJS export rendering in preserveModules mode.
    module_formats.set(module_idx, ExportsKind::Esm);
  }
  (lazy_json_export_initializers, non_splittable_json_defaults, diagnostics)
}

fn lazy_export_invariant_diagnostic(
  module: &NormalModule,
  reason: impl std::fmt::Display,
) -> BuildDiagnostic {
  BuildDiagnostic::unhandleable_error(anyhow::anyhow!(
    "could not normalize the lazy-export payload for `{}`: {reason}",
    module.id.as_str()
  ))
}

#[derive(Default)]
struct JsonObjectFallback {
  named_exports: FxIndexMap<CompactStr, (CompactStr, bool)>,
  default_member_reads_splittable: bool,
}

fn prepare_json_object_fallback(ecma_ast: &mut EcmaAst, body_index: usize) -> JsonObjectFallback {
  ecma_ast.program.with_mut(|fields| {
    let Some(ast::Statement::ExpressionStatement(stmt)) = fields.program.body.get_mut(body_index)
    else {
      return JsonObjectFallback::default();
    };
    let Expression::ObjectExpression(object) = stmt.expression.without_parentheses_mut() else {
      return JsonObjectFallback::default();
    };
    let mut names = FxIndexMap::default();
    let mut exported_names = FxHashSet::default();
    let mut default_member_reads_splittable = true;
    for property in &mut object.properties {
      let ast::ObjectPropertyKind::ObjectProperty(property) = property else { continue };
      if property.kind != PropertyKind::Init {
        default_member_reads_splittable = false;
      }
      let Some(key) = property.key.static_name().map(|key| CompactStr::new(&key)) else {
        continue;
      };
      if key == "__proto__" {
        property.computed = true;
      }
      if key.is_empty() || !exported_names.insert(key.clone()) {
        continue;
      }
      let local = legitimize_json_local_binding_name(&key, &names);
      let is_legal_ident = local == key;
      names.insert(local, (key, is_legal_ident));
    }
    JsonObjectFallback { named_exports: names, default_member_reads_splittable }
  })
}

fn add_json_named_export_bindings(
  module: &mut NormalModule,
  stmt_infos: &mut StmtInfos,
  symbols: &mut SymbolRefDb,
  named_exports: FxIndexMap<CompactStr, (CompactStr, bool)>,
) -> Box<[LazyJsonExportInitializer]> {
  let mut initializers = Vec::with_capacity(named_exports.len());
  for (local, (exported, _)) in named_exports {
    if module.named_exports.contains_key(&exported) {
      continue;
    }
    let binding_ref = symbols.create_facade_root_symbol_ref(module.idx, &local);
    binding_ref.flags_mut(symbols).insert(SymbolRefFlags::IsNotReassigned);
    module.named_exports.insert(
      exported.clone(),
      LocalExport { span: SPAN, referenced: binding_ref, came_from_commonjs: false },
    );
    let initializer_stmt_info_idx = stmt_infos.add_stmt_info(StmtInfo {
      declared_symbols: smallvec![TaggedSymbolRef::normal(binding_ref)],
      referenced_symbols: vec![module.default_export_ref.into()],
      meta: StmtInfoMeta::LazyJsonExportInitializer,
      #[cfg(debug_assertions)]
      debug_label: Some("lazy JSON export initializer".into()),
      ..Default::default()
    });
    initializers.push(LazyJsonExportInitializer::new(
      initializer_stmt_info_idx,
      binding_ref,
      exported,
    ));
  }
  initializers.into_boxed_slice()
}

fn find_lazy_export_payload_statement(
  ecma_ast: &EcmaAst,
  stmt_infos: &StmtInfos,
) -> Result<(usize, StmtInfoIdx), &'static str> {
  let mut marked_payloads = stmt_infos
    .iter_enumerated()
    .filter(|(_, stmt_info)| stmt_info.meta.contains(StmtInfoMeta::LazyExportPayload));
  let Some((stmt_info_idx, _)) = marked_payloads.next() else {
    return Err("no statement is marked as the lazy-export payload");
  };
  if marked_payloads.next().is_some() {
    return Err("multiple statements are marked as the lazy-export payload");
  }
  let Some(body_index) = stmt_info_idx.index().checked_sub(1) else {
    return Err("the namespace statement is marked as the lazy-export payload");
  };
  let Some(statement) = ecma_ast.program().body.get(body_index) else {
    return Err("the lazy-export payload marker is outside the AST");
  };
  if !std::matches!(statement, ast::Statement::ExpressionStatement(_)) {
    return Err("the marked lazy-export payload is not an expression statement");
  }
  Ok((body_index, stmt_info_idx))
}

#[derive(Clone, Copy)]
enum LazyExportWrap {
  CjsExport,
  EsmDefault,
}

/// Takes the selected expression statement and replaces it with either
/// `module.exports = expr` or `export default expr`.
fn replace_lazy_expression_statement(
  ecma_ast: &mut EcmaAst,
  body_index: usize,
  kind: LazyExportWrap,
) -> bool {
  ecma_ast.program.with_mut(|fields| {
    let ast_factory = AstFactory::new(fields.allocator);
    let Some(stmt) = fields.program.body.get_mut(body_index) else { return false };
    let expr = match stmt {
      ast::Statement::ExpressionStatement(stmt) => {
        stmt.expression.take_in(&ast_factory.allocator())
      }
      _ => return false,
    };
    *stmt = match kind {
      LazyExportWrap::CjsExport => ast_factory.make_module_exports_stmt(expr),
      LazyExportWrap::EsmDefault => ast_factory.make_export_default_stmt(expr),
    };
    true
  })
}

/// Takes `expr` (leaving a dummy in its place) and returns the owned inner
/// expression with any wrapping `(...)` parentheses removed.
fn take_without_parentheses<'ast>(
  expr: &mut Expression<'ast>,
  allocator: &'ast Allocator,
) -> Expression<'ast> {
  let mut inner_expr = expr.take_in(&allocator);
  while let Expression::ParenthesizedExpression(mut paren_expr) = inner_expr {
    inner_expr = paren_expr.expression.take_in(&allocator);
  }
  inner_expr
}

fn update_module_default_export_info(
  module: &mut NormalModule,
  stmt_infos: &mut StmtInfos,
  stmt_info_idx: StmtInfoIdx,
  default_symbol_ref: SymbolRef,
) {
  module.named_exports.insert(
    "default".into(),
    LocalExport { span: SPAN, referenced: default_symbol_ref, came_from_commonjs: false },
  );
  if let Some(stmt_info) = stmt_infos.infos.get_mut(stmt_info_idx) {
    stmt_info.meta.remove(StmtInfoMeta::LazyExportPayload);
  }
  if stmt_infos.infos.get(stmt_info_idx).is_some() {
    stmt_infos.declare_symbol_for_stmt(stmt_info_idx, TaggedSymbolRef::normal(default_symbol_ref));
  }
}

#[derive(Clone, Copy)]
struct JsonRebuildCandidate<'a> {
  protected_identity_owners: &'a IndexBitSet<ModuleIdx>,
  module_idx: ModuleIdx,
}

/// Rebuild a pristine object JSON module into independently tree-shakeable ESM bindings.
///
/// Rebuilding replaces every owner-local AST, symbol, and statement identity. A transform hook can
/// add identity-bearing side tables even when the payload still looks like JSON, so this path is
/// deliberately limited to the exact state produced by the JSON loader. All other shapes keep
/// their original semantic tables and use the ordinary lazy-export fallback.
fn json_object_expr_to_esm(
  module_table: &mut ModuleTable,
  ast_table: &mut IndexEcmaAst,
  stmt_infos: &mut IndexStmtInfos,
  symbols: &mut SymbolRefDb,
  module_formats: &mut ModuleFormatsDraft,
  wrapper_declarations: &mut WrapperDeclarationsDraft,
  candidate: JsonRebuildCandidate<'_>,
) -> bool {
  let JsonRebuildCandidate { protected_identity_owners, module_idx } = candidate;
  if protected_identity_owners.has_bit(module_idx) {
    return false;
  }
  let wrapper_declaration = wrapper_declarations.declaration(module_idx);
  {
    let Some(module) = module_table[module_idx].as_normal() else { return false };
    let Some(ecma_ast) = ast_table[module_idx].as_ref() else { return false };
    if !can_rebuild_json_object(
      module,
      ecma_ast,
      &stmt_infos[module_idx],
      symbols,
      wrapper_declaration,
    ) {
      return false;
    }
  }

  let Some(ecma_ast) = ast_table[module_idx].as_mut() else { return false };
  // (local, (exported, legal_ident))
  let mut declaration_binding_names: FxIndexMap<CompactStr, (CompactStr, bool)> =
    FxIndexMap::default();
  let transformed = ecma_ast.program.with_mut(|fields| {
    let mut index_map = FxIndexMap::default();
    let ast_factory = AstFactory::new(fields.allocator);
    let program = fields.program;
    let Some(stmts) = program.body.first_mut() else { return false };
    let expr = match stmts {
      ast::Statement::ExpressionStatement(stmt) => &mut stmt.expression,
      _ => return false,
    };
    if !std::matches!(expr.without_parentheses(), Expression::ObjectExpression(_)) {
      return false;
    }
    let Expression::ObjectExpression(mut obj_expr) =
      take_without_parentheses(expr, ast_factory.allocator())
    else {
      return false;
    };
    // clean program body, since we already take it and left a dummy expr
    program.body.clear();

    // convert {"a": "b", "c": "d"} to
    // {"a": b, "c": d}
    // and collect related info
    for property in &mut obj_expr.properties {
      match property {
        ast::ObjectPropertyKind::ObjectProperty(property) => {
          let Some(key) = property.key.static_name() else { return false };
          if key.is_empty() {
            continue;
          }
          let legitimized_ident =
            legitimize_json_local_binding_name(&key, &declaration_binding_names);

          let is_legal_ident = legitimized_ident.as_str() == key;

          declaration_binding_names
            .insert(legitimized_ident.clone(), (CompactStr::new(&key), is_legal_ident));

          let value = std::mem::replace(
            &mut property.value,
            ast_factory.make_id_ref_expr(SPAN, legitimized_ident.as_str()),
          );
          // TODO(shulaoda): Waiting for oxc transform to support the ES feature `ShorthandProperties`.
          if key == "__proto__" {
            property.computed = true;
          } else if is_legal_ident {
            property.shorthand = is_legal_ident;
            property.key = ast::PropertyKey::StaticIdentifier(ast::IdentifierName::boxed(
              SPAN,
              oxc::ast::ast::Str::from_str_in(legitimized_ident.as_ref(), &ast_factory),
              &ast_factory,
            ));
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
        ast::ObjectPropertyKind::SpreadProperty(_) => return false,
      }
    }
    // recreate Json Module
    let stmts = index_map
      .into_iter()
      // declaration
      .map(|(local, v)| ast_factory.make_var_decl(local.as_str(), v))
      // export default json module
      .chain(std::iter::once(
        ast_factory.make_export_default_stmt(Expression::ObjectExpression(obj_expr)),
      ))
      // export all declaration
      .chain(std::iter::once(
        ast_factory.make_export_named_stmt(None, declaration_binding_names.iter()),
      ));
    program.body.extend(stmts);
    true
  });

  if !transformed {
    return false;
  }
  let Some(module) = module_table[module_idx].as_normal_mut() else { return false };
  let original_symbol_ref_db = std::mem::take(symbols.local_db_mut(module_idx));
  // recreate semantic data
  let binding_count = declaration_binding_names.len();
  let node_capacity = match binding_count.checked_next_power_of_two() {
    Some(capacity) => capacity,
    None => usize::MAX,
  };
  let scoping = ecma_ast.make_symbol_table_and_scope_tree_with_semantic_builder(
    SemanticBuilder::new().with_stats(Stats {
      nodes: saturating_u32(node_capacity),
      scopes: 1,
      symbols: saturating_u32(binding_count),
      references: saturating_u32(binding_count).saturating_mul(2),
    }),
  );

  // update semantic data of module
  let root_scope_id = scoping.root_scope_id();
  // The generated program contains exactly one root `var` binding for each map entry, in this
  // insertion order. SemanticBuilder assigns dense SymbolIds while visiting those declarations.
  let binding_symbol_ids = scoping.symbol_ids().collect::<Vec<_>>();
  if binding_symbol_ids.len() != binding_count {
    tracing::error!(
      bindings = binding_symbol_ids.len(),
      declarations = binding_count,
      "generated JSON binding layout mismatch"
    );
  }
  let mut symbol_ref_db = SymbolRefDbForModule::new(scoping, module_idx, root_scope_id);

  // update module stmts info
  // Replace the whole table: draining only `infos` would leave the private symbol-to-statement
  // reverse maps pointing at identities that no longer exist.
  let original_stmt_infos = std::mem::replace(&mut stmt_infos[module_idx], StmtInfos::new());
  let mut all_declared_symbols = original_stmt_infos
    .infos
    .into_iter()
    .skip(FIRST_TOP_LEVEL_STMT_IDX.index())
    .flat_map(|info| info.referenced_symbols)
    .collect::<Vec<_>>();
  let module_stmt_infos = &mut stmt_infos[module_idx];
  module.named_exports.clear();
  for ((_, (exported, _)), symbol_id) in declaration_binding_names.iter().zip(binding_symbol_ids) {
    let symbol_ref: SymbolRef = (module_idx, symbol_id).into();
    all_declared_symbols.push(SymbolOrMemberExprRef::from(symbol_ref));
    let mut stmt_info = StmtInfo::default();
    stmt_info.declared_symbols.push(TaggedSymbolRef::normal(symbol_ref));
    module_stmt_infos.add_stmt_info(stmt_info);
    module.named_exports.insert(
      exported.clone(),
      LocalExport { span: SPAN, referenced: symbol_ref, came_from_commonjs: false },
    );
  }
  // Re-create facade symbols in the new scoping. The JSON module was re-parsed above,
  // producing fresh binding IDs, so every old facade ID is invalid. Semantic bindings already
  // occupy the prefix of the symbol table; allocating facades now preserves that layout.
  let namespace_name = original_symbol_ref_db.symbol_name(module.namespace_object_ref.symbol);
  let namespace_object_ref = symbol_ref_db.create_facade_root_symbol_ref(namespace_name);
  let default_name = original_symbol_ref_db.symbol_name(module.default_export_ref.symbol);
  let default_export_ref = symbol_ref_db.create_facade_root_symbol_ref(default_name);
  let hmr_hot_ref = module.hmr_hot_ref.map(|old_ref| {
    let name = original_symbol_ref_db.symbol_name(old_ref.symbol);
    symbol_ref_db.create_facade_root_symbol_ref(name)
  });
  module.namespace_object_ref = namespace_object_ref;
  module.default_export_ref = default_export_ref;
  module.hmr_hot_ref = hmr_hot_ref;
  // declare default export statement
  let mut stmt_info = StmtInfo::default();
  stmt_info.declared_symbols.push(TaggedSymbolRef::normal(default_export_ref));
  stmt_info.referenced_symbols.clone_from(&all_declared_symbols);

  module_stmt_infos.add_stmt_info(stmt_info);
  module.named_exports.insert(
    "default".into(),
    LocalExport { span: SPAN, referenced: default_export_ref, came_from_commonjs: false },
  );

  // declare namespace object statement
  module_formats.set(module_idx, ExportsKind::Esm);
  let mut namespace_stmt_info = StmtInfo::default();
  namespace_stmt_info.declared_symbols.push(TaggedSymbolRef::normal(namespace_object_ref));
  namespace_stmt_info.referenced_symbols = all_declared_symbols;
  module_stmt_infos.replace_namespace_stmt_info(namespace_stmt_info);
  // for a es json module it did not needs to be wrapped anyway.
  wrapper_declarations.clear(module_idx);

  symbols.store_local_db(module_idx, symbol_ref_db);
  true
}

fn saturating_u32(value: usize) -> u32 {
  u32::try_from(value).map_or(u32::MAX, |value| value)
}

fn can_rebuild_json_object(
  module: &NormalModule,
  ecma_ast: &EcmaAst,
  stmt_infos: &StmtInfos,
  symbols: &SymbolRefDb,
  wrapper_declaration: WrapperDeclaration,
) -> bool {
  if !has_pristine_json_object_ast(ecma_ast)
    || !has_pristine_json_identity_tables(module)
    || !has_pristine_json_symbols(module, symbols, wrapper_declaration)
    || !stmt_infos.symbol_ref_to_referenced_stmt_idx().is_empty()
  {
    return false;
  }

  let expected_wrapper = match wrapper_declaration {
    WrapperDeclaration::None => None,
    WrapperDeclaration::Esm { wrapper_ref, wrapper_stmt_info } => {
      Some((wrapper_ref, wrapper_stmt_info))
    }
    WrapperDeclaration::Cjs { .. } => return false,
  };
  let expected_len = if expected_wrapper.is_some() { 3 } else { 2 };
  if stmt_infos.infos.len() != expected_len {
    return false;
  }

  let Some(namespace_stmt) = stmt_infos.infos.get(StmtInfos::NAMESPACE_STMT_IDX) else {
    return false;
  };
  if !stmt_has_no_identity_payload(namespace_stmt) {
    return false;
  }
  let Some(payload_stmt) = stmt_infos.infos.get(FIRST_TOP_LEVEL_STMT_IDX) else {
    return false;
  };
  if !stmt_has_no_identity_payload(payload_stmt) {
    return false;
  }
  if !stmt_infos.declared_stmts_by_symbol(&module.default_export_ref).is_empty()
    || !stmt_infos.declared_stmts_by_symbol(&module.namespace_object_ref).is_empty()
    || module
      .hmr_hot_ref
      .is_some_and(|hot_ref| !stmt_infos.declared_stmts_by_symbol(&hot_ref).is_empty())
  {
    return false;
  }

  let Some((wrapper_ref, wrapper_stmt_info)) = expected_wrapper else { return true };
  if wrapper_ref.owner != module.idx || wrapper_stmt_info != StmtInfoIdx::from_usize(2) {
    return false;
  }
  let Some(wrapper_stmt) = stmt_infos.infos.get(wrapper_stmt_info) else { return false };
  if wrapper_stmt.declared_symbols.len() != 1
    || wrapper_stmt.declared_symbols.first().is_none_or(|declared| declared.inner() != wrapper_ref)
    || wrapper_stmt.referenced_symbols.len() != 1
    || wrapper_stmt.eval_flags != StmtEvalFlags::UnknownSideEffect
    || !wrapper_stmt.import_records.is_empty()
    || !wrapper_stmt.meta.is_empty()
    || !wrapper_stmt.force_tree_shaking
    || stmt_infos.declared_stmts_by_symbol(&wrapper_ref) != [wrapper_stmt_info]
  {
    return false;
  }
  std::matches!(
    wrapper_stmt.referenced_symbols.first(),
    Some(SymbolOrMemberExprRef::Symbol(helper)) if helper.owner != module.idx
  )
}

fn has_pristine_json_symbols(
  module: &NormalModule,
  symbols: &SymbolRefDb,
  wrapper_declaration: WrapperDeclaration,
) -> bool {
  let mut expected = Vec::with_capacity(4);
  expected.push(module.default_export_ref);
  expected.push(module.namespace_object_ref);
  if let Some(hot_ref) = module.hmr_hot_ref {
    expected.push(hot_ref);
  }
  match wrapper_declaration {
    WrapperDeclaration::None => {}
    WrapperDeclaration::Esm { wrapper_ref, .. } => expected.push(wrapper_ref),
    WrapperDeclaration::Cjs { .. } => return false,
  }
  if expected.iter().enumerate().any(|(index, symbol_ref)| {
    symbol_ref.owner != module.idx || expected[..index].contains(symbol_ref)
  }) {
    return false;
  }

  let local_db = symbols.local_db(module.idx);
  if local_db.total_symbol_count() != expected.len()
    || local_db.classic_data.len() != expected.len()
    || local_db.flags.len() != expected.len()
  {
    return false;
  }
  expected.into_iter().all(|symbol_ref| {
    if symbol_ref.symbol.index() >= local_db.total_symbol_count()
      || !local_db.is_facade_symbol(symbol_ref.symbol)
      || local_db
        .flags
        .get(&symbol_ref.symbol)
        .is_none_or(|flags| flags.bits() != SymbolRefFlags::IsFacade.bits())
    {
      return false;
    }
    let classic = local_db.get_classic_data(symbol_ref.symbol);
    classic.namespace_alias.is_none() && classic.link.is_none() && classic.chunk_idx.is_none()
  })
}

fn stmt_has_no_identity_payload(stmt: &StmtInfo) -> bool {
  stmt.declared_symbols.is_empty()
    && stmt.referenced_symbols.is_empty()
    && stmt.eval_flags.is_empty()
    && stmt.import_records.is_empty()
    && (stmt.meta.is_empty() || stmt.meta.bits() == StmtInfoMeta::LazyExportPayload.bits())
    && !stmt.force_tree_shaking
}

fn has_pristine_json_identity_tables(module: &NormalModule) -> bool {
  module.named_imports.is_empty()
    && module.named_exports.is_empty()
    && module.import_records.is_empty()
    && module.imports.is_empty()
    && module.imported_ids.is_empty()
    && module.dynamically_imported_ids.is_empty()
    && module.self_referenced_class_decl_symbol_ids.is_empty()
    && module.constant_export_map.is_empty()
    && module.dummy_record_set.is_empty()
    && module.new_url_references.is_empty()
    && module.this_expr_replace_map.is_empty()
    && module.import_attribute_map.is_empty()
    && module.json_module_none_self_reference_included_symbol.is_none()
    && module.cjs_reexport_import_record_ids.is_empty()
    && module.hmr_info.deps.is_empty()
    && module.hmr_info.module_request_to_import_record_idx.is_empty()
    && module.enum_member_value_map.is_empty()
    && module.mutations.is_empty()
    && module.hashbang_range.is_none()
    && module.directive_range.is_empty()
}

fn has_pristine_json_object_ast(ecma_ast: &EcmaAst) -> bool {
  let body = &ecma_ast.program().body;
  if body.len() != 1 {
    return false;
  }
  let Some(ast::Statement::ExpressionStatement(stmt)) = body.first() else { return false };
  let Expression::ObjectExpression(object) = stmt.expression.without_parentheses() else {
    return false;
  };
  has_pristine_json_object(object)
}

fn has_pristine_json_object(object: &ast::ObjectExpression<'_>) -> bool {
  object.properties.iter().all(|property| {
    let ast::ObjectPropertyKind::ObjectProperty(property) = property else { return false };
    property.kind == PropertyKind::Init
      && !property.method
      && !property.shorthand
      && !property.computed
      && property.key.static_name().is_some()
      && has_pristine_json_expression(&property.value)
  })
}

fn has_pristine_json_expression(expression: &Expression<'_>) -> bool {
  match expression.without_parentheses() {
    Expression::BooleanLiteral(_)
    | Expression::NullLiteral(_)
    | Expression::NumericLiteral(_)
    | Expression::StringLiteral(_) => true,
    Expression::ArrayExpression(array) => array
      .elements
      .iter()
      .all(|element| element.as_expression().is_some_and(has_pristine_json_expression)),
    Expression::ObjectExpression(object) => has_pristine_json_object(object),
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use oxc::{allocator::CloneIn, ast::ast, semantic::Scoping, span::SourceType};
  use rolldown_common::{
    LocalExport, Module, StmtInfo, StmtInfoMeta, SymbolOrMemberExprRef, SymbolRef, SymbolRefDb,
    SymbolRefDbForModule, TaggedSymbolRef, json_value_to_ecma_ast,
  };
  use rolldown_ecmascript::EcmaCompiler;

  use super::super::passes::test_utils::{module_idx, normal_module};
  use super::{
    FIRST_TOP_LEVEL_STMT_IDX, WrapperDeclaration, add_json_named_export_bindings,
    can_rebuild_json_object, find_lazy_export_payload_statement, prepare_json_object_fallback,
  };

  fn pristine_state(
    with_wrapper: bool,
  ) -> (Module, SymbolRefDb, rolldown_common::StmtInfos, WrapperDeclaration) {
    let owner = module_idx(0);
    let scoping = Scoping::default();
    let root_scope_id = scoping.root_scope_id();
    let mut local_db = SymbolRefDbForModule::new(scoping, owner, root_scope_id);
    let default_export_ref = local_db.create_facade_root_symbol_ref("data_default");
    let namespace_object_ref = local_db.create_facade_root_symbol_ref("data_exports");
    let wrapper_ref = with_wrapper.then(|| local_db.create_facade_root_symbol_ref("init_data"));
    let mut symbols = SymbolRefDb::new();
    symbols.store_local_db(owner, local_db);

    let mut module = normal_module(0, false, Vec::new());
    let Module::Normal(normal) = &mut module else { panic!("normal module") };
    normal.module_type = rolldown_common::ModuleType::Json;
    normal.default_export_ref = default_export_ref;
    normal.namespace_object_ref = namespace_object_ref;

    let mut stmt_infos = rolldown_common::StmtInfos::new();
    stmt_infos.add_stmt_info(StmtInfo::default());
    let wrapper = match wrapper_ref {
      Some(wrapper_ref) => {
        let helper = SymbolRef { owner: module_idx(1), symbol: oxc::semantic::SymbolId::new(0) };
        let mut stmt = StmtInfo::default();
        stmt.declared_symbols.push(TaggedSymbolRef::normal(wrapper_ref));
        stmt.referenced_symbols.push(helper.into());
        stmt.eval_flags = true.into();
        stmt.force_tree_shaking = true;
        let wrapper_stmt_info = stmt_infos.add_stmt_info(stmt);
        WrapperDeclaration::Esm { wrapper_ref, wrapper_stmt_info }
      }
      None => WrapperDeclaration::None,
    };
    (module, symbols, stmt_infos, wrapper)
  }

  #[test]
  fn rebuild_gate_accepts_only_pristine_loader_identities() {
    let ast = json_value_to_ecma_ast(&serde_json::json!({ "old": 1 }));
    let (mut module, mut symbols, mut stmt_infos, wrapper) = pristine_state(false);
    let Module::Normal(normal) = &mut module else { panic!("normal module") };
    assert!(can_rebuild_json_object(normal, &ast, &stmt_infos, &symbols, wrapper));

    let default_export_ref = normal.default_export_ref;
    normal.named_exports.insert(
      "injected".into(),
      rolldown_common::LocalExport {
        span: oxc::span::SPAN,
        referenced: default_export_ref,
        came_from_commonjs: false,
      },
    );
    assert!(!can_rebuild_json_object(normal, &ast, &stmt_infos, &symbols, wrapper));
    normal.named_exports.clear();

    stmt_infos.infos[FIRST_TOP_LEVEL_STMT_IDX].eval_flags = true.into();
    assert!(!can_rebuild_json_object(normal, &ast, &stmt_infos, &symbols, wrapper));
    stmt_infos.infos[FIRST_TOP_LEVEL_STMT_IDX].eval_flags = false.into();

    symbols.create_facade_root_symbol_ref(module_idx(0), "unexpected");
    assert!(!can_rebuild_json_object(normal, &ast, &stmt_infos, &symbols, wrapper));
  }

  #[test]
  fn rebuild_gate_accepts_the_exact_esm_wrapper_shape() {
    let ast = json_value_to_ecma_ast(&serde_json::json!({ "old": 1 }));
    let (module, symbols, stmt_infos, wrapper) = pristine_state(true);
    let Module::Normal(normal) = &module else { panic!("normal module") };
    assert!(can_rebuild_json_object(normal, &ast, &stmt_infos, &symbols, wrapper));

    let WrapperDeclaration::Esm { wrapper_ref, wrapper_stmt_info } = wrapper else {
      panic!("ESM wrapper")
    };
    assert!(!can_rebuild_json_object(
      normal,
      &ast,
      &stmt_infos,
      &symbols,
      WrapperDeclaration::Cjs { wrapper_ref, wrapper_stmt_info },
    ));
  }

  #[test]
  fn locates_the_payload_after_hoisted_module_declarations() {
    let ast = EcmaCompiler::parse(
      "data.json",
      "import './side-effect.js'; ({ old: 1 }); export const injected = 2;",
      SourceType::default().with_module(true),
    )
    .expect("valid fixture");
    let mut stmt_infos = rolldown_common::StmtInfos::new();
    for _ in &ast.program().body {
      stmt_infos.add_stmt_info(StmtInfo::default());
    }
    stmt_infos.infos[rolldown_common::StmtInfoIdx::from_usize(2)]
      .meta
      .insert(StmtInfoMeta::LazyExportPayload);
    let (body_index, stmt_info_idx) =
      find_lazy_export_payload_statement(&ast, &stmt_infos).expect("payload expression");
    assert_eq!(body_index, 1);
    assert_eq!(stmt_info_idx, rolldown_common::StmtInfoIdx::from_usize(2));

    let two_expressions = EcmaCompiler::parse(
      "data.json",
      "({ old: 1 }); sideEffect();",
      SourceType::default().with_module(true),
    )
    .expect("valid fixture");
    let mut stmt_infos = rolldown_common::StmtInfos::new();
    for _ in &two_expressions.program().body {
      stmt_infos.add_stmt_info(StmtInfo::default());
    }
    assert_eq!(
      find_lazy_export_payload_statement(&two_expressions, &stmt_infos),
      Err("no statement is marked as the lazy-export payload")
    );

    stmt_infos.infos[rolldown_common::StmtInfoIdx::from_usize(2)]
      .meta
      .insert(StmtInfoMeta::LazyExportPayload);
    let (body_index, stmt_info_idx) =
      find_lazy_export_payload_statement(&two_expressions, &stmt_infos)
        .expect("marked payload expression");
    assert_eq!(body_index, 1);
    assert_eq!(stmt_info_idx, rolldown_common::StmtInfoIdx::from_usize(2));

    stmt_infos.infos[rolldown_common::StmtInfoIdx::from_usize(1)]
      .meta
      .insert(StmtInfoMeta::LazyExportPayload);
    assert_eq!(
      find_lazy_export_payload_statement(&two_expressions, &stmt_infos),
      Err("multiple statements are marked as the lazy-export payload")
    );

    stmt_infos.infos[rolldown_common::StmtInfoIdx::from_usize(1)]
      .meta
      .remove(StmtInfoMeta::LazyExportPayload);
    stmt_infos.infos[rolldown_common::StmtInfoIdx::from_usize(2)]
      .meta
      .remove(StmtInfoMeta::LazyExportPayload);
    stmt_infos.infos[rolldown_common::StmtInfos::NAMESPACE_STMT_IDX]
      .meta
      .insert(StmtInfoMeta::LazyExportPayload);
    assert_eq!(
      find_lazy_export_payload_statement(&two_expressions, &stmt_infos),
      Err("the namespace statement is marked as the lazy-export payload")
    );

    let mut stmt_infos = rolldown_common::StmtInfos::new();
    for _ in &ast.program().body {
      stmt_infos.add_stmt_info(StmtInfo::default());
    }
    stmt_infos.infos[rolldown_common::StmtInfoIdx::from_usize(1)]
      .meta
      .insert(StmtInfoMeta::LazyExportPayload);
    assert_eq!(
      find_lazy_export_payload_statement(&ast, &stmt_infos),
      Err("the marked lazy-export payload is not an expression statement")
    );
  }

  #[test]
  fn fallback_exports_use_ordinary_snapshot_bindings() {
    let (mut module, mut symbols, mut stmt_infos, _) = pristine_state(false);
    let Module::Normal(normal) = &mut module else { panic!("normal module") };
    let default_export_ref = normal.default_export_ref;
    let payload_before = stmt_infos.infos[FIRST_TOP_LEVEL_STMT_IDX].clone();
    let original_stmt_count = stmt_infos.infos.len();

    let mut named_exports = rolldown_utils::indexmap::FxIndexMap::default();
    named_exports.insert("property_name".into(), ("property-name".into(), false));
    let initializers =
      add_json_named_export_bindings(normal, &mut stmt_infos, &mut symbols, named_exports);

    let [initializer] = initializers.as_ref() else { panic!("one initializer") };
    let binding_ref = initializer.binding_ref();
    assert_eq!(initializer.property_name(), "property-name");
    assert!(symbols.get(binding_ref).namespace_alias.is_none());
    assert!(binding_ref.is_not_reassigned(&symbols));
    assert_eq!(normal.named_exports["property-name"].referenced, binding_ref);
    assert_eq!(stmt_infos.infos.len(), original_stmt_count + 1);
    let payload_after = &stmt_infos.infos[FIRST_TOP_LEVEL_STMT_IDX];
    assert_eq!(
      payload_after.declared_symbols.iter().map(TaggedSymbolRef::inner).collect::<Vec<_>>(),
      payload_before.declared_symbols.iter().map(TaggedSymbolRef::inner).collect::<Vec<_>>()
    );
    assert_eq!(payload_after.referenced_symbols, payload_before.referenced_symbols);
    assert_eq!(payload_after.eval_flags, payload_before.eval_flags);
    assert_eq!(payload_after.import_records, payload_before.import_records);
    assert_eq!(payload_after.meta.bits(), payload_before.meta.bits());
    assert_eq!(payload_after.force_tree_shaking, payload_before.force_tree_shaking);

    let initializer_stmt = &stmt_infos.infos[initializer.initializer_stmt_info_idx()];
    assert_eq!(initializer_stmt.meta.bits(), StmtInfoMeta::LazyJsonExportInitializer.bits());
    assert_eq!(initializer_stmt.declared_symbols.len(), 1);
    assert_eq!(initializer_stmt.declared_symbols[0].inner(), binding_ref);
    assert!(matches!(
      initializer_stmt.referenced_symbols.as_slice(),
      [SymbolOrMemberExprRef::Symbol(reference)] if *reference == default_export_ref
    ));
    assert_eq!(
      stmt_infos.declared_stmts_by_symbol(&binding_ref),
      [initializer.initializer_stmt_info_idx()]
    );
  }

  #[test]
  fn fallback_exports_preserve_plugin_defined_collisions() {
    let (mut module, mut symbols, mut stmt_infos, _) = pristine_state(false);
    let Module::Normal(normal) = &mut module else { panic!("normal module") };
    let plugin_ref = normal.default_export_ref;
    normal.named_exports.insert(
      "injected".into(),
      LocalExport { span: oxc::span::SPAN, referenced: plugin_ref, came_from_commonjs: false },
    );
    let original_stmt_count = stmt_infos.infos.len();

    let mut named_exports = rolldown_utils::indexmap::FxIndexMap::default();
    named_exports.insert("injected".into(), ("injected".into(), true));
    let initializers =
      add_json_named_export_bindings(normal, &mut stmt_infos, &mut symbols, named_exports);

    assert!(initializers.is_empty());
    assert_eq!(normal.named_exports["injected"].referenced, plugin_ref);
    assert_eq!(stmt_infos.infos.len(), original_stmt_count);
  }

  #[test]
  fn fallback_normalizes_every_duplicate_proto_property() {
    let mut ast = json_value_to_ecma_ast(&serde_json::json!({ "__proto__": 1 }));
    ast.program.with_mut(|fields| {
      let Some(ast::Statement::ExpressionStatement(statement)) = fields.program.body.first_mut()
      else {
        panic!("JSON payload statement")
      };
      let ast::Expression::ObjectExpression(object) =
        statement.expression.without_parentheses_mut()
      else {
        panic!("JSON object payload")
      };
      let Some(property) = object.properties.first() else { panic!("JSON property") };
      let first_duplicate = property.clone_in(fields.allocator);
      let second_duplicate = property.clone_in(fields.allocator);
      object.properties.push(first_duplicate);
      object.properties.push(second_duplicate);
    });

    let fallback = prepare_json_object_fallback(&mut ast, 0);
    assert_eq!(fallback.named_exports.len(), 1);
    let Some(ast::Statement::ExpressionStatement(statement)) = ast.program().body.first() else {
      panic!("JSON payload statement")
    };
    let ast::Expression::ObjectExpression(object) = statement.expression.without_parentheses()
    else {
      panic!("JSON object payload")
    };
    assert_eq!(object.properties.len(), 3);
    assert!(object.properties.iter().all(|property| {
      matches!(property, ast::ObjectPropertyKind::ObjectProperty(property) if property.computed)
    }));
  }
}
