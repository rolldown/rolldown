use std::{path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_common::{
  ExternalModuleTaskResult, ModuleIdx, ModuleInfo, ModuleInfoSideEffects, ModuleLoaderMsg,
  ResolvedExternal, ResolvedId, side_effects::HookSideEffects,
};
use rolldown_error::BuildResult;
use rolldown_utils::ecmascript::legitimize_identifier_name;
use sugar_path::SugarPath;

use rolldown_fs::FileSystem;

use crate::ecmascript::ecma_module_view_factory::{
  declared_side_effects_without_hook, external_side_effects, normalize_side_effects,
};

use super::task_context::{TaskContext, TaskMessageGuard};

pub struct ExternalModuleTask<Fs: FileSystem> {
  ctx: Arc<TaskContext<Fs>>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  create_js_module_options: bool,
}

impl<Fs: FileSystem> ExternalModuleTask<Fs> {
  pub fn new(
    ctx: Arc<TaskContext<Fs>>,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    create_js_module_options: bool,
  ) -> Self {
    Self { ctx, module_idx: idx, resolved_id, create_js_module_options }
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
    if self.create_js_module_options {
      self.ctx.create_js_module_options(resolved_id, None).await?;
    }
    let overrides = &self.ctx.plugin_driver.module_side_effects_overrides;
    let external_module_side_effects = normalize_side_effects(
      &self.ctx.options,
      overrides,
      resolved_id,
      None,
      resolved_id.side_effects,
    )
    .await?;
    let id = resolved_id.id.clone();
    self.ctx.plugin_driver.set_module_info(
      &id.clone(),
      Arc::new(ModuleInfo::placeholder(
        id,
        false,
        Some(ModuleInfoSideEffects {
          decided: resolved_id.side_effects.map(HookSideEffects::declared),
          without_hook: declared_side_effects_without_hook(&self.ctx.options, resolved_id),
        }),
      )),
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
      side_effects_without_option: external_side_effects(resolved_id),
      need_renormalize_render_path,
      attributes: resolved_id.attributes.clone(),
    }));
    // The send fails only when the build was dropped before this task finished.
    // Then nothing reads the result, so ignore the error.
    let _ = self.ctx.tx.send(msg);
    Ok(())
  }
}
