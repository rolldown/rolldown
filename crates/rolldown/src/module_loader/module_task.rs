use std::{collections::hash_map::Entry, sync::Arc};

use arcstr::ArcStr;
use oxc::span::Span;
use oxc_index::IndexVec;
use rustc_hash::{FxHashMap, FxHashSet};
use sugar_path::SugarPath as _;

use rolldown_common::{
  BarrelInfo, EcmaRelated, EcmaView, FlatOptions, ImportKind, ImportRecordIdx, ImportRecordMeta,
  ImportedExports, ModuleIdx, ModuleInfo, ModuleLoaderMsg, ModuleType, NormalModule,
  NormalModuleTaskResult, RawImportRecord, ResolvedId, SourceMapGenMsg, SourcemapChainElement,
  Specifier, StrOrBytes,
};
use rolldown_error::{
  BuildDiagnostic, BuildResult, UnloadableDependencyContext, downcast_napi_error_diagnostics,
};
use rolldown_std_utils::PathExt as _;
use rolldown_utils::{ecmascript::legitimize_identifier_name, indexmap::FxIndexSet};

use crate::{
  asset::create_asset_view,
  css::create_css_view,
  ecmascript::ecma_module_view_factory::{CreateEcmaViewReturn, create_ecma_view},
  types::module_factory::{CreateModuleContext, CreateModuleViewArgs},
  utils::{load_source::load_source, transform_source::transform_source},
};

use super::{resolve_utils::resolve_dependencies, task_context::TaskContext};

pub struct ModuleTaskOwnerRef<'a> {
  module: &'a NormalModule,
  importee_span: Span,
}

impl<'a> ModuleTaskOwnerRef<'a> {
  pub fn new(module: &'a NormalModule, importee_span: Span) -> Self {
    Self { module, importee_span }
  }
}

impl From<ModuleTaskOwnerRef<'_>> for ModuleTaskOwner {
  fn from(owner: ModuleTaskOwnerRef) -> Self {
    ModuleTaskOwner {
      source: owner.module.source.clone(),
      importer_id: owner.module.stable_id.as_arc_str().clone(),
      importee_span: owner.importee_span,
    }
  }
}

pub struct ModuleTaskOwner {
  source: ArcStr,
  importer_id: ArcStr,
  importee_span: Span,
}

pub struct ModuleTask {
  ctx: Arc<TaskContext>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  owner: Option<ModuleTaskOwner>,
  is_user_defined_entry: bool,
  /// The module is asserted to be this specific module type.
  asserted_module_type: Option<ModuleType>,
  flat_options: FlatOptions,
  magic_string_tx: Option<std::sync::Arc<std::sync::mpsc::Sender<SourceMapGenMsg>>>,
}

impl ModuleTask {
  #[expect(clippy::too_many_arguments)]
  pub fn new(
    ctx: Arc<TaskContext>,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
    is_user_defined_entry: bool,
    assert_module_type: Option<ModuleType>,
    flat_options: FlatOptions,
    magic_string_tx: Option<std::sync::Arc<std::sync::mpsc::Sender<SourceMapGenMsg>>>,
  ) -> Self {
    Self {
      ctx,
      module_idx: idx,
      resolved_id,
      owner,
      is_user_defined_entry,
      asserted_module_type: assert_module_type,
      flat_options,
      magic_string_tx,
    }
  }

  #[tracing::instrument(name="NormalModuleTask::run", level = "trace", skip_all, fields(module_id = ?self.resolved_id.id))]
  pub async fn run(mut self) {
    if let Err(errs) = self.run_inner().await {
      self.ctx.plugin_driver.mark_context_load_modules_loaded(self.resolved_id.id.clone());
      self
        .ctx
        .tx
        .send(ModuleLoaderMsg::BuildErrors(errs.into_vec().into_boxed_slice()))
        .await
        .expect("ModuleLoader: failed to send build errors - main thread terminated while processing module errors");
    }
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    let id = self.resolved_id.id.clone();

    self.ctx.plugin_driver.set_module_info(
      &id,
      Arc::new(ModuleInfo {
        code: None,
        id: id.clone(),
        is_entry: self.is_user_defined_entry,
        importers: FxIndexSet::default(),
        dynamic_importers: FxIndexSet::default(),
        imported_ids: FxIndexSet::default(),
        dynamically_imported_ids: FxIndexSet::default(),
        exports: vec![],
      }),
    );

    let mut sourcemap_chain = vec![];
    let mut hook_side_effects = self.resolved_id.side_effects.take();
    let (mut source, module_type) = self
      .load_source_without_cache(
        &mut sourcemap_chain,
        &mut hook_side_effects,
        self.magic_string_tx.clone(),
      )
      .await?;

    let stable_id = id.stabilize(&self.ctx.options.cwd);
    let mut raw_import_records = IndexVec::default();

    let (asset_view, css_view) = match module_type {
      ModuleType::Asset => {
        let asset_view = create_asset_view(source);
        source = StrOrBytes::Str(String::new());
        (Some(asset_view), None)
      }
      ModuleType::Css => {
        let css_source: ArcStr = source.try_into_string()?.into();
        // FIXME: This makes creating `EcmaView` rely on creating `CssView` first, while they should be done in parallel.
        let (css_view, css_raw_import_records) = create_css_view(&stable_id, &css_source);
        raw_import_records = css_raw_import_records;
        source = StrOrBytes::Str(String::new());
        (None, Some(css_view))
      }
      _ => (None, None),
    };

    let mut warnings = vec![];

    let ret = create_ecma_view(
      &mut CreateModuleContext {
        stable_id: &stable_id,
        module_idx: self.module_idx,
        plugin_driver: &self.ctx.plugin_driver,
        resolved_id: &self.resolved_id,
        options: &self.ctx.options,
        warnings: &mut warnings,
        module_type: module_type.clone(),
        replace_global_define_config: self.ctx.meta.replace_global_define_config.clone(),
        is_user_defined_entry: self.is_user_defined_entry,
        flat_options: self.flat_options,
      },
      CreateModuleViewArgs { source, sourcemap_chain, hook_side_effects },
    )
    .await?;

    let CreateEcmaViewReturn {
      mut ecma_view,
      ecma_related,
      raw_import_records: ecma_raw_import_records,
    } = ret;

    if css_view.is_none() {
      raw_import_records = ecma_raw_import_records;
    }

    let resolved_deps = resolve_dependencies(
      &self.resolved_id,
      &self.ctx.options,
      &self.ctx.resolver,
      &self.ctx.plugin_driver,
      &raw_import_records,
      ecma_view.source.clone(),
      &mut warnings,
      &module_type,
    )
    .await?;

    if css_view.is_none() {
      for (record, info) in raw_import_records.iter().zip(&resolved_deps) {
        match record.kind {
          ImportKind::Import | ImportKind::Require | ImportKind::NewUrl => {
            ecma_view.imported_ids.insert(info.id.clone());
          }
          ImportKind::DynamicImport => {
            ecma_view.dynamically_imported_ids.insert(info.id.clone());
          }
          ImportKind::HotAccept => {
            ecma_view.hmr_info.deps.insert(info.id.clone());
          }
          // for a none css module, we should not have `at-import` or `url-import`
          ImportKind::AtImport | ImportKind::UrlImport => unreachable!(),
        }
      }
    }

    let repr_name = self.resolved_id.id.as_path().representative_file_name();
    let repr_name = legitimize_identifier_name(&repr_name).into_owned();

    // Build BarrelInfo for barrel module optimization
    let barrel_info = self.build_barrel_info(&ecma_view, &ecma_related, &raw_import_records);

    let module = NormalModule {
      repr_name,
      stable_id,
      id,
      debug_id: self.resolved_id.debug_id(&self.ctx.options.cwd),
      idx: self.module_idx,
      exec_order: u32::MAX,
      is_user_defined_entry: self.is_user_defined_entry,
      module_type: module_type.clone(),
      ecma_view,
      css_view,
      asset_view,
      originative_resolved_id: self.resolved_id.clone(),
    };

    let module_info = Arc::new(module.to_module_info(Some(&raw_import_records)));
    self.ctx.plugin_driver.set_module_info(&module.id, Arc::clone(&module_info));
    self.ctx.plugin_driver.module_parsed(Arc::clone(&module_info), &module).await?;
    self.ctx.plugin_driver.mark_context_load_modules_loaded(module.id.clone());

    let result = ModuleLoaderMsg::NormalModuleDone(Box::new(NormalModuleTaskResult {
      module: module.into(),
      ecma_related,
      resolved_deps,
      raw_import_records,
      warnings,
      barrel_info,
    }));

    self.ctx.tx.send(result).await.expect(
      "ModuleLoader channel closed while sending module completion - main thread terminated unexpectedly"
    );

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  async fn load_source_without_cache(
    &self,
    sourcemap_chain: &mut Vec<SourcemapChainElement>,
    hook_side_effects: &mut Option<rolldown_common::side_effects::HookSideEffects>,
    magic_string_tx: Option<std::sync::Arc<std::sync::mpsc::Sender<SourceMapGenMsg>>>,
  ) -> BuildResult<(StrOrBytes, ModuleType)> {
    let mut is_read_from_disk = true;
    let result = load_source(
      &self.ctx.plugin_driver,
      &self.resolved_id,
      self.ctx.fs.clone(),
      sourcemap_chain,
      hook_side_effects,
      &self.ctx.options,
      self.asserted_module_type.as_ref(),
      &mut is_read_from_disk,
    )
    .await;
    if is_read_from_disk {
      // - Only add watch files for files read from disk.
      // - Add watch files as early as possible for we might be able to recover from build errors.
      self.ctx.plugin_driver.watch_files.insert(self.resolved_id.id.as_arc_str().clone());
    }
    let (source, mut module_type) = result.map_err(|err| {
      downcast_napi_error_diagnostics(err).unwrap_or_else(|e| {
        BuildDiagnostic::unloadable_dependency(
          self.resolved_id.debug_id(self.ctx.options.cwd.as_path()).into(),
          self.owner.as_ref().map(|owner| UnloadableDependencyContext {
            source: owner.source.clone(),
            importer_id: owner.importer_id.clone(),
            importee_span: owner.importee_span,
          }),
          e.to_string().into(),
        )
      })
    })?;
    if let Some(asserted) = &self.asserted_module_type {
      module_type = asserted.clone();
    }
    let source = match source {
      _ if self.resolved_id.id.starts_with("rolldown:") => source,
      StrOrBytes::Str(source) => {
        // Run plugin transform.
        let source = transform_source(
          &self.ctx.plugin_driver,
          &self.resolved_id,
          self.module_idx,
          source,
          sourcemap_chain,
          hook_side_effects,
          &mut module_type,
          magic_string_tx,
        )
        .await?;
        source.into()
      }
      StrOrBytes::Bytes(_) => source,
    };
    if let ModuleType::Custom(_) = module_type {
      // TODO: should provide some diagnostics for user how they should handle the module type.
      // e.g.
      // sass -> recommended npm install `sass` etc
      Err(anyhow::anyhow!(
        "`{:?}` is not specified module type,  rolldown can't handle this asset correctly. Please use the load/transform hook to transform the resource",
        self.resolved_id.id
      ))?;
    }
    Ok((source, module_type))
  }

  /// Build BarrelInfo from EcmaView for lazy barrel optimization.
  fn build_barrel_info(
    &self,
    ecma_view: &EcmaView,
    ecma_related: &EcmaRelated,
    raw_import_records: &IndexVec<ImportRecordIdx, RawImportRecord>,
  ) -> Option<BarrelInfo> {
    if !self.ctx.options.experimental.is_lazy_barrel_enabled() {
      return None;
    }

    // Check if module has side effects - barrel modules must be side-effect free
    if ecma_view.side_effects.has_side_effects() || raw_import_records.is_empty() {
      return None;
    }

    let mut immediate_imported_specifiers = FxHashMap::default();
    for named_import in ecma_view.named_imports.values() {
      match &named_import.imported {
        Specifier::Star => {
          immediate_imported_specifiers.insert(named_import.record_idx, ImportedExports::All);
        }
        Specifier::Literal(name) => {
          match immediate_imported_specifiers.entry(named_import.record_idx) {
            Entry::Occupied(mut occ) => {
              if let ImportedExports::Partial(set) = occ.get_mut() {
                set.insert(name.clone());
              }
            }
            Entry::Vacant(vac) => {
              vac.insert(ImportedExports::Partial(FxHashSet::from_iter([name.clone()])));
            }
          }
        }
      }
    }

    let mut star_export_records = Vec::new();
    let mut export_to_record = FxHashMap::default();

    // Find re-exports from named_imports
    // `export { c as d } from './x'`: export_name="d", imported=Literal("c")
    // `export * as ns from './x'`: export_name="ns", imported=Star
    for (export_name, local_export) in &ecma_view.named_exports {
      if let Some(named_import) = ecma_view.named_imports.get(&local_export.referenced) {
        export_to_record
          .insert(export_name.clone(), (named_import.record_idx, named_import.imported.clone()));
      }
    }

    // Find star exports from import records
    for (rec_idx, record) in raw_import_records.iter_enumerated() {
      if record.meta.contains(ImportRecordMeta::IsExportStar) {
        star_export_records.push(rec_idx);
      }
    }

    // Only return Some if there are any re-exports
    let info = BarrelInfo { export_to_record, star_export_records };
    if info.export_to_record.is_empty() && info.star_export_records.is_empty() {
      None
    } else {
      Some(info)
    }
  }
}
