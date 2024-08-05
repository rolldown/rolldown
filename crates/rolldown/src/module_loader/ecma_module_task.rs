use std::sync::Arc;

use anyhow::Result;
use rolldown_common::{Module, ModuleIdx, ResolvedId, StrOrBytes};
use rolldown_error::BuildDiagnostic;

use super::{task_context::TaskContext, Msg};
use crate::{
  ecmascript::ecma_module_factory::EcmaModuleFactory,
  module_loader::NormalModuleTaskResult,
  types::module_factory::{
    CreateModuleArgs, CreateModuleContext, CreateModuleReturn, ModuleFactory,
  },
  utils::{load_source::load_source, transform_source::transform_source},
};
pub struct EcmaModuleTask {
  ctx: Arc<TaskContext>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  importer_id: Option<String>,
  errors: Vec<BuildDiagnostic>,
  is_user_defined_entry: bool,
}

impl EcmaModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    importer_id: Option<String>,
  ) -> Self {
    let is_user_defined_entry = importer_id.is_none();
    Self { ctx, module_idx: idx, resolved_id, importer_id, errors: vec![], is_user_defined_entry }
  }

  #[tracing::instrument(name="NormalModuleTask::run", level = "trace", skip_all, fields(module_id = ?self.resolved_id.id))]
  pub async fn run(mut self) {
    match self.run_inner().await {
      Ok(()) => {
        if !self.errors.is_empty() {
          self.ctx.tx.send(Msg::BuildErrors(self.errors)).await.expect("Send should not fail");
        }
      }
      Err(err) => {
        self.ctx.tx.send(Msg::Panics(err)).await.expect("Send should not fail");
      }
    }
  }

  async fn run_inner(&mut self) -> Result<()> {
    let mut hook_side_effects = self.resolved_id.side_effects.take();
    let mut sourcemap_chain = vec![];
    let mut warnings = vec![];

    // Run plugin load to get content first, if it is None using read fs as fallback.
    let (source, module_type) = match load_source(
      &self.ctx.plugin_driver,
      &self.resolved_id,
      &self.ctx.fs,
      &mut sourcemap_chain,
      &mut hook_side_effects,
      &self.ctx.options,
    )
    .await
    {
      Ok(ret) => ret,
      Err(err) => {
        self.errors.push(BuildDiagnostic::unresolved_import(
          self.resolved_id.id.to_string(),
          self.importer_id.clone(),
          err.to_string(),
        ));
        return Ok(());
      }
    };

    let source = match source {
      StrOrBytes::Str(source) => {
        // Run plugin transform.
        let source = transform_source(
          &self.ctx.plugin_driver,
          &self.resolved_id,
          source,
          &mut sourcemap_chain,
          &mut hook_side_effects,
        )
        .await?;
        source.into()
      }
      StrOrBytes::Bytes(_) => source,
    };

    let Some(module_type) = module_type else {
      return Err(anyhow::format_err!(
        "[{:?}] is not specified module type, rolldown can't handle this asset correctly. Please use the load/transform hook to transform the resource",
        self.resolved_id.id
      ));
    };

    let ret = EcmaModuleFactory::create_module(
      &mut CreateModuleContext {
        module_index: self.module_idx,
        plugin_driver: &self.ctx.plugin_driver,
        resolved_id: &self.resolved_id,
        options: &self.ctx.options,
        warnings: &mut warnings,
        module_type: module_type.clone(),
        resolver: &self.ctx.resolver,
        is_user_defined_entry: self.is_user_defined_entry,
      },
      CreateModuleArgs { source, sourcemap_chain, hook_side_effects },
    )
    .await?;

    let CreateModuleReturn { module, resolved_deps, ecma_related, raw_import_records } = match ret {
      Ok(ret) => ret,
      Err(errs) => {
        self.errors.extend(errs);
        return Ok(());
      }
    };

    if let Module::Ecma(module) = &module {
      self.ctx.plugin_driver.module_parsed(Arc::new(module.to_module_info())).await?;
    }

    if let Err(_err) = self
      .ctx
      .tx
      .send(Msg::NormalModuleDone(NormalModuleTaskResult {
        resolved_deps,
        module_idx: self.module_idx,
        warnings,
        ecma_related,
        module,
        raw_import_records,
      }))
      .await
    {
      // The main thread is dead, nothing we can do to handle these send failures.
    }

    Ok(())
  }
}
