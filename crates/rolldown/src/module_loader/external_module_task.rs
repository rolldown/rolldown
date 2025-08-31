use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_common::{
  ExternalModuleTaskResult, ModuleId, ModuleIdx, ModuleInfo, ModuleLoaderMsg, ResolvedExternal,
  ResolvedId,
};
use rolldown_error::BuildResult;
use rolldown_utils::{ecmascript::legitimize_identifier_name, indexmap::FxIndexSet};
use sugar_path::SugarPath;

use crate::ecmascript::ecma_module_view_factory::normalize_side_effects;

use super::task_context::TaskContext;

#[expect(clippy::rc_buffer)]
pub struct ExternalModuleTask {
  ctx: Arc<TaskContext>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  user_defined_entries: Arc<Vec<(Option<ArcStr>, ResolvedId)>>,
  // The module is asserted to be this specific module type.
}

#[expect(clippy::rc_buffer)]
impl ExternalModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    user_defined_entries: Arc<Vec<(Option<ArcStr>, ResolvedId)>>,
  ) -> Self {
    Self { ctx, module_idx: idx, resolved_id, user_defined_entries }
  }

  #[tracing::instrument(name="ExternalModuleTask::run", level = "trace", skip_all, fields(module_id = ?self.resolved_id.id))]
  pub async fn run(self) {
    if let Err(errs) = self.run_inner().await {
      self
        .ctx
        .tx
        .send(ModuleLoaderMsg::BuildErrors(errs.into_vec().into_boxed_slice()))
        .await
        .expect("Send should not fail");
    }
  }

  async fn run_inner(&self) -> BuildResult<()> {
    let resolved_id = &self.resolved_id;
    let external_module_side_effects =
      normalize_side_effects(&self.ctx.options, resolved_id, None, None, resolved_id.side_effects)
        .await?;
    let id = ModuleId::new(&resolved_id.id);
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
      }),
    );

    let need_renormalize_render_path = !matches!(resolved_id.external, ResolvedExternal::Absolute)
      && Path::new(resolved_id.id.as_str()).is_absolute();

    let file_name = if need_renormalize_render_path {
      let entries_common_dir = commondir::CommonDir::try_new(
        self.user_defined_entries.iter().map(|(_, resolved_id)| resolved_id.id.as_str()),
      )
      .expect("should have common dir for entries");
      let relative_path =
        Path::new(resolved_id.id.as_str()).relative(entries_common_dir.common_root());
      relative_path.to_slash_lossy().into()
    } else {
      resolved_id.id.clone()
    };

    let identifier_name = if need_renormalize_render_path {
      Path::new(resolved_id.id.as_str())
        .relative(&self.ctx.options.cwd)
        .normalize()
        .to_slash_lossy()
        .into()
    } else {
      resolved_id.id.clone()
    };
    let legitimized_identifier_name = legitimize_identifier_name(&identifier_name);
    let msg = ModuleLoaderMsg::ExternalModuleDone(Box::new(ExternalModuleTaskResult {
      idx: self.module_idx,
      id: resolved_id.id.clone(),
      name: file_name,
      identifier_name: legitimized_identifier_name.into(),
      side_effects: external_module_side_effects,
      need_renormalize_render_path,
    }));
    // If the main thread is dead, nothing we can do to handle these send failures.
    let _ = self.ctx.tx.send(msg).await;
    Ok(())
  }
}
