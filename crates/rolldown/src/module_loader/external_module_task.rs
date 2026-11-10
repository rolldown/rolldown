use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_common::{
  ExportsKind, ExternalModuleTaskResult, ModuleIdx, ModuleInfo, ModuleLoaderMsg, ResolvedExternal,
  ResolvedId,
};
use rolldown_error::BuildResult;
use rolldown_utils::{ecmascript::legitimize_identifier_name, indexmap::FxIndexSet};
use sugar_path::SugarPath;

use rolldown_fs::FileSystem;

use crate::ecmascript::ecma_module_view_factory::normalize_side_effects;

use super::task_context::{TaskContext, TaskMessageGuard};

pub struct ExternalModuleTask<Fs: FileSystem> {
  ctx: Arc<TaskContext<Fs>>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  // The module is asserted to be this specific module type.
}

impl<Fs: FileSystem> ExternalModuleTask<Fs> {
  pub fn new(ctx: Arc<TaskContext<Fs>>, idx: ModuleIdx, resolved_id: ResolvedId) -> Self {
    Self { ctx, module_idx: idx, resolved_id }
  }

  #[tracing::instrument(name="ExternalModuleTask::run", level = "trace", skip_all, fields(module_id = %self.resolved_id.id))]
  pub async fn run(self) {
    let guard = TaskMessageGuard::new(&self.ctx, self.resolved_id.id.clone());
    if let Err(errs) = self.run_inner().await {
      // The send fails only when the build was dropped before this task finished.
      // Then nothing reads the result, so ignore the error.
      let _ = self.ctx.tx.send(ModuleLoaderMsg::BuildErrors(errs.into_vec().into_boxed_slice()));
    }
    guard.mark_sent();
  }

  async fn run_inner(&self) -> BuildResult<()> {
    let resolved_id = &self.resolved_id;
    let external_module_side_effects =
      normalize_side_effects(&self.ctx.options, resolved_id, None, resolved_id.side_effects)
        .await?;
    let id = resolved_id.id.clone();
    self.ctx.plugin_driver.set_module_info(
      &id.clone(),
      Arc::new(ModuleInfo {
        code: None,
        id,
        is_entry: false,
        importers: FxIndexSet::default(),
        dynamic_importers: FxIndexSet::default(),
        imported_ids: FxIndexSet::default(),
        dynamically_imported_ids: FxIndexSet::default(),
        exports: vec![],
        input_format: ExportsKind::None,
      }),
    );

    let need_renormalize_render_path =
      !matches!(resolved_id.external, ResolvedExternal::Absolute) && resolved_id.id.is_path();

    let raw_id = resolved_id.id.raw_id();

    let identifier_name: ArcStr = if need_renormalize_render_path {
      let relative_path = Path::new(raw_id).relative(&self.ctx.options.cwd);
      ArcStr::from(relative_path.to_slash())
    } else {
      raw_id.into()
    };
    let legitimized_identifier_name = legitimize_identifier_name(&identifier_name);
    let msg = ModuleLoaderMsg::ExternalModuleDone(Box::new(ExternalModuleTaskResult {
      idx: self.module_idx,
      id: resolved_id.id.clone(),
      name: raw_id.into(),
      identifier_name: legitimized_identifier_name.into(),
      side_effects: external_module_side_effects,
      need_renormalize_render_path,
      attributes: resolved_id.attributes.clone(),
    }));
    // The send fails only when the build was dropped before this task finished.
    // Then nothing reads the result, so ignore the error.
    let _ = self.ctx.tx.send(msg);
    Ok(())
  }
}
