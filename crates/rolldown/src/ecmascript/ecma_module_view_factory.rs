use std::sync::Arc;

use oxc::span::Span;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaModuleAstUsage, EcmaRelated, EcmaView, EcmaViewMeta, FlatOptions, ImportRecordIdx, ModuleId,
  ModuleSideEffectsOverrides, ModuleType, RawImportRecord, ResolvedId,
  SharedNormalizedBundlerOptions, SideEffectsWithoutHook, StmtEvalFlags,
  side_effects::{DeterminedSideEffects, HookSideEffects},
};
use rolldown_error::BuildResult;
use rolldown_utils::{ecmascript::legitimize_identifier_name, indexmap::FxIndexSet};

use crate::{
  ast_scanner::{AstScanner, ScanResult},
  types::module_factory::{CreateModuleContext, CreateModuleViewArgs},
  utils::parse_to_ecma_ast::{ParseToEcmaAstResult, parse_to_ecma_ast},
};

pub struct CreateEcmaViewReturn {
  pub ecma_view: EcmaView,
  pub ecma_related: EcmaRelated,
  pub raw_import_records: IndexVec<ImportRecordIdx, RawImportRecord>,
  /// The span of the first top-level `await` keyword, if any. Routed through
  /// the task result into a centralized map on the link stage so it doesn't
  /// need to live on every `EcmaView`.
  pub tla_keyword_span: Option<Span>,
}

pub async fn create_ecma_view(
  ctx: &mut CreateModuleContext<'_>,
  args: CreateModuleViewArgs,
) -> BuildResult<CreateEcmaViewReturn> {
  let CreateModuleViewArgs { source, sourcemap_chain, hook_side_effects, declared_module_size } =
    args;
  let json_source_len =
    matches!(ctx.module_type, ModuleType::Json).then(|| source.as_bytes().len());
  let ParseToEcmaAstResult {
    mut ast,
    scoping,
    facade_symbol_reserve,
    has_lazy_export,
    warnings,
    preserve_jsx,
    enum_member_value_map,
  } = parse_to_ecma_ast(ctx, source).await?;
  ctx.flat_options.set(FlatOptions::JsxPreserve, preserve_jsx);
  ctx.warnings.extend(warnings);

  let module_id = ctx.resolved_id.id.clone();

  let repr_name = module_id.representative_name();
  let repr_name = legitimize_identifier_name(&repr_name);

  let scan_result = ast.program.with_mut(|fields| {
    let program = &*fields.program;
    let scanner = AstScanner::new(
      ctx.module_idx,
      scoping,
      facade_symbol_reserve,
      &repr_name,
      ctx.resolved_id.module_def_format,
      fields.source,
      &module_id,
      &program.comments,
      ctx.options,
      fields.allocator,
      ctx.flat_options,
    );
    scanner.scan(program)
  })?;

  let ScanResult {
    commonjs_exports,
    cjs_literal_export_names,
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
    tla_keyword_span,
    symbol_ref_db: symbols,
    self_referenced_class_decl_symbol_ids,
    hashbang_range,
    ecma_view_meta,
    dynamic_import_rec_exports_usage,
    dynamic_import_called_names,
    new_url_references: new_url_imports,
    namespace_in_checks,
    rolldown_file_url_references,
    this_expr_replace_map,
    cjs_this_ref,
    hmr_info,
    hmr_hot_ref,
    directive_range,
    dummy_record_set,
    constant_export_map,
    deferred_write_constants,
    cjs_reexport_require_node_ids: _,
    cjs_reexport_import_record_ids,
    import_defer_scan,
  } = scan_result;
  // If a export symbol in commonjs defined in multiple time, we just bailout treeshake it.
  for (k, v) in commonjs_exports {
    if v.len() == 1 {
      named_exports.insert(k, v[0]);
    }
  }

  if !errors.is_empty() {
    return Err(errors.into());
  }

  ctx.warnings.extend(scan_warnings);

  let side_effects = normalize_side_effects(
    ctx.options,
    &ctx.plugin_driver.module_side_effects_overrides,
    ctx.resolved_id,
    Some(&stmt_infos),
    hook_side_effects,
  )
  .await?;

  // TODO: Should we check if there are `check_side_effects_for` returns false but there are side effects in the module?
  let ecma_view = EcmaView {
    source: ast.source().clone(),
    pre_render_size: declared_module_size.or(json_source_len).unwrap_or(ast.source().len()),
    named_imports,
    named_exports,
    cjs_literal_export_names,
    imports,
    default_export_ref,
    exports_kind,
    namespace_object_ref,
    def_format: ctx.resolved_id.module_def_format,
    sourcemap_chain: Arc::new(sourcemap_chain),
    import_records: IndexVec::default(),
    importers: FxIndexSet::default(),
    importers_idx: FxIndexSet::default(),
    dynamic_importers: FxIndexSet::default(),
    dynamic_importers_idx: FxIndexSet::default(),
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
    new_url_references: new_url_imports,
    namespace_in_checks,
    rolldown_file_url_references,
    this_expr_replace_map,
    cjs_this_ref,
    hmr_info,
    hmr_hot_ref,
    directive_range,
    dummy_record_set,
    constant_export_map,
    deferred_write_constants,
    enum_member_value_map,
    json_module_none_self_reference_included_symbol: None,
    cjs_reexport_import_record_ids,
    dynamic_import_called_names,
    import_defer_scan,
  };

  let ecma_related =
    EcmaRelated { ast, symbols, dynamic_import_rec_exports_usage, preserve_jsx, stmt_infos };
  Ok(CreateEcmaViewReturn { ecma_view, ecma_related, raw_import_records, tla_keyword_span })
}

/// What `treeshake.moduleSideEffects` says about one module.
#[derive(Debug, Clone, Copy)]
pub enum OptionSideEffects {
  /// `treeshake: false`
  TreeshakeDisabled,
  Decided(bool),
  Undecided,
}

/// A function option runs only when no earlier call for the module left a result in
/// `overrides`, because callers ask only when they need the value. See
/// internal-docs/cache/implementation.md.
pub async fn read_option_side_effects(
  options: &SharedNormalizedBundlerOptions,
  overrides: &ModuleSideEffectsOverrides,
  module_id: &ModuleId,
  is_external: bool,
) -> BuildResult<OptionSideEffects> {
  let Some(opt) = options.treeshake.as_ref() else {
    return Ok(OptionSideEffects::TreeshakeDisabled);
  };
  let raw_id = module_id.raw_id();
  let value = if opt.module_side_effects.is_fn() {
    overrides.option_fn_result_or_call(options, module_id.as_str(), raw_id, is_external).await?
  } else {
    opt.module_side_effects.native_resolve(raw_id, is_external)
  };
  Ok(value.map_or(OptionSideEffects::Undecided, OptionSideEffects::Decided))
}

/// The side effects that tree shaking uses, with the same order as
/// [`rolldown_common::module_side_effects_decision`]:
/// 1. Hook side effects. With `treeshake: false`, a hook `true` counts as no value.
/// 2. `treeshake.moduleSideEffects`, read only when no hook decided
/// 3. For an external module: none for a Node.js built-in, else no tree shaking
/// 4. Package.json side effects
/// 5. Analyzed side effects
pub async fn normalize_side_effects(
  options: &SharedNormalizedBundlerOptions,
  overrides: &ModuleSideEffectsOverrides,
  resolved_id: &ResolvedId,
  stmt_infos: Option<&rolldown_common::StmtInfos>,
  mut hook_side_effects: Option<HookSideEffects>,
) -> BuildResult<DeterminedSideEffects> {
  if options.treeshake.is_none() && hook_side_effects == Some(HookSideEffects::True) {
    hook_side_effects = None;
  }
  let is_external = resolved_id.external.is_external();
  let side_effects = match hook_side_effects {
    Some(HookSideEffects::False) => DeterminedSideEffects::UserDefined(false),
    Some(HookSideEffects::NoTreeshake) => DeterminedSideEffects::NoTreeshake,
    // The hook asserted side effects, so it takes priority over `package.json#sideEffects`.
    Some(HookSideEffects::True) if is_external => external_side_effects(resolved_id),
    Some(HookSideEffects::True) => lazy_check_side_effects(stmt_infos),
    None => match read_option_side_effects(options, overrides, &resolved_id.id, is_external).await?
    {
      OptionSideEffects::TreeshakeDisabled => DeterminedSideEffects::NoTreeshake,
      OptionSideEffects::Decided(value) => DeterminedSideEffects::UserDefined(value),
      OptionSideEffects::Undecided if is_external => external_side_effects(resolved_id),
      OptionSideEffects::Undecided => match resolved_id.package_json_side_effects() {
        Some(value) => DeterminedSideEffects::UserDefined(value),
        None => lazy_check_side_effects(stmt_infos),
      },
    },
  };
  Ok(side_effects)
}

pub fn external_side_effects(resolved_id: &ResolvedId) -> DeterminedSideEffects {
  if resolved_id.is_external_without_side_effects {
    DeterminedSideEffects::UserDefined(false)
  } else {
    DeterminedSideEffects::NoTreeshake
  }
}

/// The value `getModuleInfo().moduleSideEffects` shows when no hook decided:
/// `treeshake.moduleSideEffects`, else, for an external module, `false` for a Node.js built-in
/// and `true` otherwise, else `package.json#sideEffects`, else `true`. `true` means "analyze the
/// statements", so it is not the result of that analysis. It is also `true` with
/// `treeshake: false`. These are the `reported` values of
/// [`rolldown_common::module_side_effects_decision`]. A hook value is shown as
/// `HookSideEffects::declared` gives it. A function option is not called here: see
/// `SideEffectsWithoutHook::OptionFn`.
pub fn declared_side_effects_without_hook(
  options: &SharedNormalizedBundlerOptions,
  resolved_id: &ResolvedId,
) -> SideEffectsWithoutHook {
  let is_external = resolved_id.external.is_external();
  let fallback = || {
    if is_external {
      !resolved_id.is_external_without_side_effects
    } else {
      resolved_id.package_json_side_effects().unwrap_or(true)
    }
  };
  match options.treeshake.as_ref() {
    None => SideEffectsWithoutHook::Known(true),
    Some(opt) if opt.module_side_effects.is_fn() => {
      SideEffectsWithoutHook::OptionFn { fallback: fallback(), external: is_external }
    }
    Some(opt) => SideEffectsWithoutHook::Known(
      opt
        .module_side_effects
        .native_resolve(resolved_id.id.raw_id(), is_external)
        .unwrap_or_else(fallback),
    ),
  }
}

/// Statement-level side effects. `package.json` and externals are decided before this runs.
pub fn lazy_check_side_effects(
  stmt_infos: Option<&rolldown_common::StmtInfos>,
) -> DeterminedSideEffects {
  let stmt_infos = stmt_infos.expect("Normal module should have stmt_infos");
  // when determining cjs module side effects:
  // we don't considered `exports.a` has side effects
  let analyzed_side_effects = stmt_infos
    .iter()
    .any(|stmt_info| stmt_info.eval_flags.contains(StmtEvalFlags::UnknownSideEffect));
  DeterminedSideEffects::Analyzed(analyzed_side_effects)
}
