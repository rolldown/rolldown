use arcstr::ArcStr;
use oxc::span::Span;
use rolldown_rstr::Rstr;
use rolldown_utils::{ecma_script::legitimize_identifier_name, path_ext::PathExt};
use std::sync::Arc;
use sugar_path::SugarPath;

use anyhow::Result;
use rolldown_common::{
  ModuleId, ModuleIdx, ModuleType, ModuleView, NormalModule, ResolvedId, StrOrBytes,
};
use rolldown_error::{BuildDiagnostic, UnloadableDependencyContext};

use super::{task_context::TaskContext, Msg};
use crate::{
  css::create_css_view,
  ecmascript::ecma_module_view_factory::EcmaModuleViewFactory,
  module_loader::NormalModuleTaskResult,
  types::module_factory::{
    CreateModuleContext, CreateModuleViewArgs, CreateModuleViewReturn, ModuleViewFactory,
  },
  utils::{load_source::load_source, transform_source::transform_source},
};

pub struct ModuleTaskOwner {
  source: ArcStr,
  importer_id: Rstr,
  importee_span: Span,
}

impl ModuleTaskOwner {
  pub fn new(source: ArcStr, importer_id: Rstr, importee_span: Span) -> Self {
    ModuleTaskOwner { source, importer_id, importee_span }
  }
}

pub struct ModuleTask {
  ctx: Arc<TaskContext>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  owner: Option<ModuleTaskOwner>,
  errors: Vec<BuildDiagnostic>,
  is_user_defined_entry: bool,
}

impl ModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    owner: Option<ModuleTaskOwner>,
  ) -> Self {
    let is_user_defined_entry = owner.is_none();
    Self { ctx, module_idx: idx, resolved_id, owner, errors: vec![], is_user_defined_entry }
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

  #[expect(clippy::too_many_lines)]
  async fn run_inner(&mut self) -> Result<()> {
    let mut hook_side_effects = self.resolved_id.side_effects.take();
    let mut sourcemap_chain = vec![];
    let mut warnings = vec![];

    // Run plugin load to get content first, if it is None using read fs as fallback.
    let (source, mut module_type) = match load_source(
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
        self.errors.push(BuildDiagnostic::unloadable_dependency(
          self.resolved_id.debug_id(self.ctx.options.cwd.as_path()).into(),
          self.owner.as_ref().map(|owner| UnloadableDependencyContext {
            importer_id: owner.importer_id.as_str().into(),
            importee_span: owner.importee_span,
            source: owner.source.clone(),
          }),
          err.to_string().into(),
        ));
        return Ok(());
      }
    };

    let mut source = match source {
      StrOrBytes::Str(source) => {
        // Run plugin transform.
        let source = transform_source(
          &self.ctx.plugin_driver,
          &self.resolved_id,
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
        self.resolved_id.id
      ));
    };

    let repr_name = self.resolved_id.id.as_path().representative_file_name();
    let repr_name = legitimize_identifier_name(&repr_name);

    let id = ModuleId::new(ArcStr::clone(&self.resolved_id.id));
    let stable_id = id.stabilize(&self.ctx.options.cwd);

    let css_view = if matches!(module_type, ModuleType::Css) {
      let css_source: ArcStr = source.try_into_string()?.into();
      // FIXME: This makes creating `EcmaView` rely on creating `CssView` first, while they should be done in parallel.
      source = StrOrBytes::Str(String::new());
      Some(create_css_view(stable_id.clone(), &css_source)?)
    } else {
      None
    };

    let ret = EcmaModuleViewFactory::create_module_view(
      &mut CreateModuleContext {
        module_index: self.module_idx,
        plugin_driver: &self.ctx.plugin_driver,
        resolved_id: &self.resolved_id,
        options: &self.ctx.options,
        warnings: &mut warnings,
        module_type: module_type.clone(),
        resolver: &self.ctx.resolver,
        replace_global_define_config: self.ctx.meta.replace_global_define_config.clone(),
      },
      CreateModuleViewArgs { source, sourcemap_chain, hook_side_effects },
    )
    .await?;

    let CreateModuleViewReturn { view, resolved_deps, ecma_related, raw_import_records } = match ret
    {
      Ok(ret) => ret,
      Err(errs) => {
        self.errors.extend(errs);
        return Ok(());
      }
    };

    let ecma_view = match view {
      ModuleView::Ecma(view) => view,
      ModuleView::Css(_) => unreachable!(),
    };

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
    };

    self.ctx.plugin_driver.module_parsed(Arc::new(module.to_module_info())).await?;

    if let Err(_err) = self
      .ctx
      .tx
      .send(Msg::NormalModuleDone(NormalModuleTaskResult {
        resolved_deps,
        module_idx: self.module_idx,
        warnings,
        ecma_related,
        module: module.into(),
        raw_import_records,
      }))
      .await
    {
      // The main thread is dead, nothing we can do to handle these send failures.
    }

    Ok(())
  }
}
