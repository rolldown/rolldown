use oxc_index::IndexVec;
use rolldown_common::{
  EcmaModuleAstUsage, EcmaRelated, EcmaView, EcmaViewMeta, ImportRecordIdx, ModuleId, ModuleType,
  RawImportRecord, ResolvedId, SharedNormalizedBundlerOptions, SideEffectDetail,
  side_effects::{DeterminedSideEffects, HookSideEffects},
};
use rolldown_error::BuildResult;
use rolldown_std_utils::PathExt;
use rolldown_utils::{ecmascript::legitimize_identifier_name, indexmap::FxIndexSet};
use sugar_path::SugarPath;

use crate::{
  ast_scanner::{AstScanner, ScanResult},
  types::module_factory::{CreateModuleContext, CreateModuleViewArgs},
  utils::parse_to_ecma_ast::{ParseToEcmaAstResult, parse_to_ecma_ast},
};

pub struct CreateEcmaViewReturn {
  pub ecma_view: EcmaView,
  pub ecma_related: EcmaRelated,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
}

#[expect(clippy::too_many_lines)]
pub async fn create_ecma_view(
  ctx: &mut CreateModuleContext<'_>,
  args: CreateModuleViewArgs,
) -> BuildResult<CreateEcmaViewReturn> {
  let CreateModuleViewArgs { source, sourcemap_chain, hook_side_effects } = args;
  let ParseToEcmaAstResult { ast, scoping, has_lazy_export, warnings } =
    parse_to_ecma_ast(ctx, source).await?;

  ctx.warnings.extend(warnings);

  let module_id = ModuleId::new(&ctx.resolved_id.id);

  let repr_name = module_id.as_path().representative_file_name();
  let repr_name = legitimize_identifier_name(&repr_name);

  let scanner = AstScanner::new(
    ctx.module_index,
    scoping,
    &repr_name,
    ctx.resolved_id.module_def_format,
    ast.source(),
    &module_id,
    ast.comments(),
    ctx.options,
    ast.allocator(),
  );

  let ScanResult {
    commonjs_exports,
    named_imports,
    mut named_exports,
    stmt_infos,
    import_records: raw_import_records,
    default_export_ref,
    namespace_object_ref,
    imports,
    exports_kind,
    warnings: scan_warnings,
    errors,
    ast_usage,
    symbol_ref_db: symbols,
    self_referenced_class_decl_symbol_ids,
    hashbang_range,
    ecma_view_meta,
    dynamic_import_rec_exports_usage,
    new_url_references: new_url_imports,
    this_expr_replace_map,
    hmr_info,
    hmr_hot_ref,
    directive_range,
    dummy_record_set,
    constant_export_map,
    import_attribute_map,
  } = scanner.scan(ast.program())?;
  named_exports.extend(commonjs_exports);

  if !errors.is_empty() {
    return Err(errors.into());
  }

  ctx.warnings.extend(scan_warnings);

  let side_effects = normalize_side_effects(
    ctx.options,
    ctx.resolved_id,
    Some(&stmt_infos),
    Some(&ctx.module_type),
    hook_side_effects,
  )
  .await?;

  // TODO: Should we check if there are `check_side_effects_for` returns false but there are side effects in the module?
  let ecma_view = EcmaView {
    source: ast.source().clone(),
    named_imports,
    named_exports,
    stmt_infos,
    imports,
    default_export_ref,
    exports_kind,
    namespace_object_ref,
    def_format: ctx.resolved_id.module_def_format,
    sourcemap_chain,
    import_records: IndexVec::default(),
    importers: FxIndexSet::default(),
    importers_idx: FxIndexSet::default(),
    dynamic_importers: FxIndexSet::default(),
    imported_ids: FxIndexSet::default(),
    dynamically_imported_ids: FxIndexSet::default(),
    side_effects,
    meta: {
      let mut meta = ecma_view_meta;
      meta.set(EcmaViewMeta::HasLazyExport, has_lazy_export);
      meta.set(
        EcmaViewMeta::SafelyTreeshakeCommonjs,
        ast_usage.contains(EcmaModuleAstUsage::AllStaticExportPropertyAccess)
          && !ast_usage.contains(EcmaModuleAstUsage::UnknownExportsRead),
      );
      meta
    },
    ast_usage,
    self_referenced_class_decl_symbol_ids,
    hashbang_range,
    mutations: vec![],
    new_url_references: new_url_imports,
    this_expr_replace_map,
    hmr_info,
    hmr_hot_ref,
    directive_range,
    dummy_record_set,
    constant_export_map,
    depended_runtime_helper: Box::default(),
    import_attribute_map,
  };

  let ecma_related = EcmaRelated { ast, symbols, dynamic_import_rec_exports_usage };
  Ok(CreateEcmaViewReturn { ecma_view, ecma_related, raw_import_records })
}

/// The side effects priority is:
/// 1. Hook side effects
/// 2. Package.json side effects
/// 3. Analyzed side effects
///
/// We should skip the `check_side_effects_for` if the hook side effects is not `None`.
pub async fn normalize_side_effects(
  options: &SharedNormalizedBundlerOptions,
  resolved_id: &ResolvedId,
  stmt_infos: Option<&rolldown_common::StmtInfos>,
  module_type: Option<&ModuleType>,
  hook_side_effects: Option<HookSideEffects>,
) -> BuildResult<DeterminedSideEffects> {
  let side_effects = match hook_side_effects {
    Some(side_effects) => match side_effects {
      HookSideEffects::True => lazy_check_side_effects(resolved_id, module_type, stmt_infos),
      HookSideEffects::False => DeterminedSideEffects::UserDefined(false),
      HookSideEffects::NoTreeshake => DeterminedSideEffects::NoTreeshake,
    },
    // If user don't specify the side effects, we use fallback value from `option.treeshake.moduleSideEffects`;
    None => match options.treeshake.as_ref() {
      // Actually this convert is not necessary, just for passing type checking
      None => DeterminedSideEffects::NoTreeshake,
      Some(opt) => {
        if opt.module_side_effects.is_fn() {
          match opt
            .module_side_effects
            .ffi_resolve(&resolved_id.id, resolved_id.external.is_external())
            .await?
          {
            Some(value) => DeterminedSideEffects::UserDefined(value),
            None => lazy_check_side_effects(resolved_id, module_type, stmt_infos),
          }
        } else {
          match opt
            .module_side_effects
            .native_resolve(&resolved_id.id, resolved_id.external.is_external())
          {
            Some(value) => DeterminedSideEffects::UserDefined(value),
            None => lazy_check_side_effects(resolved_id, module_type, stmt_infos),
          }
        }
      }
    },
  };
  Ok(side_effects)
}

pub fn lazy_check_side_effects(
  resolved_id: &ResolvedId,
  module_type: Option<&ModuleType>,
  stmt_infos: Option<&rolldown_common::StmtInfos>,
) -> DeterminedSideEffects {
  if resolved_id.external.is_external() {
    return if resolved_id.is_external_without_side_effects {
      DeterminedSideEffects::UserDefined(false)
    } else {
      DeterminedSideEffects::NoTreeshake
    };
  }
  let module_type = module_type.expect("Normal module should have module_type");
  let stmt_infos = stmt_infos.expect("Normal module should have stmt_infos");
  if matches!(module_type, ModuleType::Css) {
    // CSS modules are considered to have side effects by default
    return DeterminedSideEffects::Analyzed(true);
  }
  resolved_id
    .package_json
    .as_ref()
    .and_then(|p| {
      // the glob expr is based on parent path of package.json, which is package path
      // so we should use the relative path of the module to package path
      let module_path_relative_to_package = resolved_id.id.as_path().relative(p.realpath.parent()?);
      p.check_side_effects_for(&module_path_relative_to_package.to_string_lossy())
        .map(DeterminedSideEffects::UserDefined)
    })
    .unwrap_or_else(|| {
      // when determining cjs module side effects:
      // we don't considered `exports.a` has side effects
      let analyzed_side_effects = stmt_infos
        .iter()
        .any(|stmt_info| stmt_info.side_effect.contains(SideEffectDetail::Unknown));
      DeterminedSideEffects::Analyzed(analyzed_side_effects)
    })
}
