use arcstr::ArcStr;
use oxc::{
  index::IndexVec,
  semantic::{ScopeTree, SymbolTable},
};
use rolldown_common::{
  side_effects::{DeterminedSideEffects, HookSideEffects},
  AstScopes, EcmaView, ImportRecordIdx, ModuleDefFormat, ModuleId, ModuleIdx, ModuleType,
  RawImportRecord, ResolvedId, SymbolRef, SymbolRefDbForModule, TreeshakeOptions,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::{DiagnosableResult, UnhandleableResult};
use rolldown_utils::{ecma_script::legitimize_identifier_name, path_ext::PathExt};
use sugar_path::SugarPath;

use crate::{
  ast_scanner::{AstScanner, ScanResult},
  types::module_factory::{CreateModuleContext, CreateModuleViewArgs},
  utils::{
    make_ast_symbol_and_scope::make_ast_scopes_and_symbols,
    parse_to_ecma_ast::{parse_to_ecma_ast, ParseToEcmaAstResult},
  },
};

fn scan_ast(
  module_idx: ModuleIdx,
  id: &ArcStr,
  ast: &mut EcmaAst,
  symbols: SymbolTable,
  scopes: ScopeTree,
  module_def_format: ModuleDefFormat,
) -> UnhandleableResult<(AstScopes, ScanResult, SymbolRef)> {
  let (symbol_table, ast_scopes) = make_ast_scopes_and_symbols(symbols, scopes);
  let module_id = ModuleId::new(ArcStr::clone(id));
  let repr_name = module_id.as_path().representative_file_name();
  let repr_name = legitimize_identifier_name(&repr_name);

  let scanner = AstScanner::new(
    module_idx,
    &ast_scopes,
    symbol_table,
    &repr_name,
    module_def_format,
    ast.source(),
    &module_id,
    &ast.trivias,
  );
  let namespace_object_ref = scanner.namespace_object_ref;
  let scan_result = scanner.scan(ast.program())?;

  Ok((ast_scopes, scan_result, namespace_object_ref))
}
pub struct CreateEcmaViewReturn {
  pub view: EcmaView,
  pub resolved_deps: IndexVec<ImportRecordIdx, ResolvedId>,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  pub ast: EcmaAst,
  pub symbols: SymbolRefDbForModule,
}

#[allow(clippy::too_many_lines)]
pub async fn create_ecma_view<'any>(
  ctx: &mut CreateModuleContext<'any>,
  args: CreateModuleViewArgs,
) -> UnhandleableResult<DiagnosableResult<CreateEcmaViewReturn>> {
  let id = ModuleId::new(ArcStr::clone(&ctx.resolved_id.id));
  let stable_id = id.stabilize(&ctx.options.cwd);

  let parse_result = parse_to_ecma_ast(
    ctx.plugin_driver,
    ctx.resolved_id.id.as_path(),
    &stable_id,
    ctx.options,
    &ctx.module_type,
    args.source.clone(),
    ctx.replace_global_define_config.as_ref(),
  )?;

  let ParseToEcmaAstResult { mut ast, symbol_table, scope_tree, source } = match parse_result {
    Ok(parse_result) => parse_result,
    Err(errs) => {
      return Ok(Err(errs));
    }
  };

  let (scope, scan_result, namespace_object_ref) = scan_ast(
    ctx.module_index,
    &ctx.resolved_id.id,
    &mut ast,
    symbol_table,
    scope_tree,
    ctx.resolved_id.module_def_format,
  )?;

  let resolved_deps = match ctx.resolve_dependencies(&scan_result.import_records, source).await? {
    Ok(deps) => deps,
    Err(errs) => {
      return Ok(Err(errs));
    }
  };

  let ScanResult {
    named_imports,
    named_exports,
    stmt_infos,
    import_records,
    star_exports,
    default_export_ref,
    imports,
    exports_kind,
    warnings: scan_warnings,
    has_eval,
    errors,
    ast_usage,
    symbol_ref_db,
  } = scan_result;
  if !errors.is_empty() {
    return Ok(Err(errors));
  }
  ctx.warnings.extend(scan_warnings);

  let mut imported_ids = vec![];
  let mut dynamically_imported_ids = vec![];

  for (record, info) in import_records.iter().zip(&resolved_deps) {
    if record.kind.is_static() {
      imported_ids.push(ArcStr::clone(&info.id).into());
    } else {
      dynamically_imported_ids.push(ArcStr::clone(&info.id).into());
    }
  }

  // The side effects priority is:
  // 1. Hook side effects
  // 2. Package.json side effects
  // 3. Analyzed side effects
  // We should skip the `check_side_effects_for` if the hook side effects is not `None`.
  let lazy_check_side_effects = || {
    if matches!(ctx.module_type, ModuleType::Css) {
      // CSS modules are considered to have side effects by default
      return DeterminedSideEffects::Analyzed(true);
    }
    ctx
      .resolved_id
      .package_json
      .as_ref()
      .and_then(|p| p.check_side_effects_for(&stable_id).map(DeterminedSideEffects::UserDefined))
      .unwrap_or_else(|| {
        let analyzed_side_effects = stmt_infos.iter().any(|stmt_info| stmt_info.side_effect);
        DeterminedSideEffects::Analyzed(analyzed_side_effects)
      })
  };
  let side_effects = match args.hook_side_effects {
    Some(side_effects) => match side_effects {
      HookSideEffects::True => lazy_check_side_effects(),
      HookSideEffects::False => DeterminedSideEffects::UserDefined(false),
      HookSideEffects::NoTreeshake => DeterminedSideEffects::NoTreeshake,
    },
    // If user don't specify the side effects, we use fallback value from `option.treeshake.moduleSideEffects`;
    None => match ctx.options.treeshake {
      // Actually this convert is not necessary, just for passing type checking
      TreeshakeOptions::Boolean(false) => DeterminedSideEffects::NoTreeshake,
      TreeshakeOptions::Boolean(true) => unreachable!(),
      TreeshakeOptions::Option(ref opt) => {
        if opt.module_side_effects.resolve(&stable_id) {
          lazy_check_side_effects()
        } else {
          DeterminedSideEffects::UserDefined(false)
        }
      }
    },
  };

  // TODO: Should we check if there are `check_side_effects_for` returns false but there are side effects in the module?
  let view = EcmaView {
    source: ast.source().clone(),
    ecma_ast_idx: None,
    named_imports,
    named_exports,
    stmt_infos,
    imports,
    star_exports,
    default_export_ref,
    scope,
    exports_kind,
    namespace_object_ref,
    def_format: ctx.resolved_id.module_def_format,
    sourcemap_chain: args.sourcemap_chain,
    import_records: IndexVec::default(),
    is_included: false,
    importers: vec![],
    dynamic_importers: vec![],
    imported_ids,
    dynamically_imported_ids,
    side_effects,
    has_eval,
    ast_usage,
  };

  Ok(Ok(CreateEcmaViewReturn {
    view,
    resolved_deps,
    raw_import_records: import_records,
    ast,
    symbols: symbol_ref_db,
  }))
}
