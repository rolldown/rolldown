use arcstr::ArcStr;
use oxc::{index::IndexVec, span::Span};
use rolldown_resolver::ResolveError;
use rolldown_utils::{ecmascript::legitimize_identifier_name, path_ext::PathExt};
use std::sync::Arc;
use sugar_path::SugarPath;

use rolldown_common::{
  create_asset_view, create_css_view, ImportKind, ImportRecordIdx, ModuleDefFormat, ModuleId,
  ModuleIdx, ModuleType, NormalModule, RawImportRecord, ResolvedId, SharedNormalizedBundlerOptions,
  StrOrBytes,
};
use rolldown_error::{
  BuildDiagnostic, BuildResult, DiagnosableArcstr, UnloadableDependencyContext,
};

use crate::SharedPluginDriver;

use super::{load_source::load_source, transform_source::transform_source};

#[expect(clippy::too_many_lines)]
pub async fn fetch_module(
  resolved_id: ResolvedId,
  plugin_driver: SharedPluginDriver,
  fs: rolldown_fs::OsFileSystem,
  options: SharedNormalizedBundlerOptions,
) -> BuildResult<()> {
  let mut hook_side_effects = resolved_id.side_effects.take();
  let mut sourcemap_chain = vec![];
  let mut warnings = vec![];

  // Run plugin load to get content first, if it is None using read fs as fallback.
  let (source, mut module_type) = match load_source(
    &plugin_driver,
    &resolved_id,
    &fs,
    &mut sourcemap_chain,
    &mut hook_side_effects,
    &options,
  )
  .await
  {
    Ok(ret) => ret,
    Err(err) => {
      //   self.errors.push(BuildDiagnostic::unloadable_dependency(
      //     self.resolved_id.debug_id(self.ctx.options.cwd.as_path()).into(),
      //     self.owner.as_ref().map(|owner| UnloadableDependencyContext {
      //       importer_id: owner.importer_id.as_str().into(),
      //       importee_span: owner.importee_span,
      //       source: owner.source.clone(),
      //     }),
      //     err.to_string().into(),
      //   ));
      return Ok(());
    }
  };

  let mut source = match source {
    StrOrBytes::Str(source) => {
      // Run plugin transform.
      let source = transform_source(
        &plugin_driver,
        &resolved_id,
        source,
        &mut sourcemap_chain,
        &mut hook_side_effects,
        &mut module_type,
      )
      .await?;
      source.into()
    }
    StrOrBytes::Bytes(_) => source,
  };

  // TODO: module type should be able to updated by transform hook, for now we don't impl it.
  if let ModuleType::Custom(_) = module_type {
    // TODO: should provide some diagnostics for user how they should handle the module type.
    // e.g.
    // sass -> recommended npm install `sass` etc
    return Err(anyhow::format_err!(
      "`{:?}` is not specified module type,  rolldown can't handle this asset correctly. Please use the load/transform hook to transform the resource",
      resolved_id.id
    ))?;
  };

  let repr_name = resolved_id.id.as_path().representative_file_name().into_owned();
  let repr_name = legitimize_identifier_name(&repr_name);

  let id = ModuleId::new(ArcStr::clone(&resolved_id.id));
  let stable_id = id.stabilize(&options.cwd);

  let mut raw_import_records = IndexVec::default();

  let asset_view = if matches!(module_type, ModuleType::Asset) {
    let asset_source = source.into_bytes();
    source = StrOrBytes::Str(String::new());
    Some(create_asset_view(asset_source.into()))
  } else {
    None
  };

  let css_view = if matches!(module_type, ModuleType::Css) {
    let css_source: ArcStr = source.try_into_string()?.into();
    // FIXME: This makes creating `EcmaView` rely on creating `CssView` first, while they should be done in parallel.
    source = StrOrBytes::Str(String::new());
    let create_ret = create_css_view(&stable_id, &css_source);
    raw_import_records = create_ret.1;
    Some(create_ret.0)
  } else {
    None
  };

  let ret = create_ecma_view(
    &mut CreateModuleContext {
      module_index: self.module_idx,
      plugin_driver: &self.ctx.plugin_driver,
      resolved_id: &self.resolved_id,
      options: &self.ctx.options,
      warnings: &mut warnings,
      module_type: module_type.clone(),
      replace_global_define_config: self.ctx.meta.replace_global_define_config.clone(),
      is_user_defined_entry: self.is_user_defined_entry,
    },
    CreateModuleViewArgs { source, sourcemap_chain, hook_side_effects },
  )
  .await?;

  let CreateEcmaViewReturn {
    view: mut ecma_view,
    ast,
    symbols,
    raw_import_records: ecma_raw_import_records,
  } = ret;

  if !matches!(module_type, ModuleType::Css) {
    raw_import_records = ecma_raw_import_records;
  }
  let resolved_deps = match self
    .resolve_dependencies(
      &raw_import_records,
      ecma_view.source.clone(),
      &mut warnings,
      &module_type,
    )
    .await?
  {
    Ok(deps) => deps,
    Err(errs) => {
      self.errors.extend(errs.into_vec());
      return Ok(());
    }
  };
  if !matches!(module_type, ModuleType::Css) {
    for (record, info) in raw_import_records.iter().zip(&resolved_deps) {
      match record.kind {
        ImportKind::Import | ImportKind::Require => {
          ecma_view.imported_ids.push(ArcStr::clone(&info.id).into());
        }
        ImportKind::DynamicImport => {
          ecma_view.dynamically_imported_ids.push(ArcStr::clone(&info.id).into());
        }
        // for a none css module, we should not have `at-import` or `url-import`
        ImportKind::AtImport | ImportKind::UrlImport => unreachable!(),
      }
    }
  }
  let module = NormalModule {
    repr_name: repr_name.into_owned(),
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
  };

  let module_info = Arc::new(module.to_module_info());
  self.ctx.plugin_driver.set_module_info(&module.id, Arc::clone(&module_info));
  self.ctx.plugin_driver.module_parsed(Arc::clone(&module_info)).await?;
  self.ctx.plugin_driver.mark_context_load_modules_loaded(&module.id).await?;

  //   if let Err(_err) = self
  //     .ctx
  //     .tx
  //     .send(Msg::NormalModuleDone(NormalModuleTaskResult {
  //       resolved_deps,
  //       module_idx: self.module_idx,
  //       warnings,
  //       ecma_related: Some((ast, symbols)),
  //       module: module.into(),
  //       raw_import_records,
  //     }))
  //     .await
  //   {
  //     // The main thread is dead, nothing we can do to handle these send failures.
  //   }

  Ok(())
}
