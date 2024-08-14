use arcstr::ArcStr;
use oxc::span::Span;
use rolldown_rstr::Rstr;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;

use super::{task_context::TaskContext, Msg};
use crate::{
  ecmascript::ecma_module_factory::EcmaModuleFactory,
  module_loader::NormalModuleTaskResult,
  types::module_factory::{
    CreateModuleArgs, CreateModuleContext, CreateModuleReturn, ModuleFactory,
  },
  utils::{load_source::load_source, transform_source::transform_source},
};
use anyhow::{bail, Result};
use rolldown_common::{
  AssetSource, FileNameRenderOptions, Module, ModuleIdx, ModuleType, OutputAsset, ResolvedId,
  StrOrBytes,
};
use rolldown_error::{BuildDiagnostic, UnloadableDependencyContext};
use rolldown_utils::sanitize_file_name::sanitize_file_name;
use rolldown_utils::xxhash::xxhash_base64_url;

pub struct EcmaModuleTaskOwner {
  source: ArcStr,
  importer_id: Rstr,
  importee_span: Span,
}

impl EcmaModuleTaskOwner {
  pub fn new(source: ArcStr, importer_id: Rstr, importee_span: Span) -> Self {
    EcmaModuleTaskOwner { source, importer_id, importee_span }
  }
}

pub struct EcmaModuleTask {
  ctx: Arc<TaskContext>,
  module_idx: ModuleIdx,
  resolved_id: ResolvedId,
  owner: Option<EcmaModuleTaskOwner>,
  errors: Vec<BuildDiagnostic>,
  is_user_defined_entry: bool,
}

impl EcmaModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    idx: ModuleIdx,
    resolved_id: ResolvedId,
    owner: Option<EcmaModuleTaskOwner>,
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

  #[allow(clippy::too_many_lines)]
  async fn run_inner(&mut self) -> Result<()> {
    let mut hook_side_effects = self.resolved_id.side_effects.take();
    let mut sourcemap_chain = vec![];
    let mut warnings = vec![];
    let mut assets = vec![];

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

    let source = match source {
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
      StrOrBytes::Bytes(_) => {
        let bytes = source.try_into_bytes()?;
        let filename = self.resolved_id.id.to_string();
        let path = Path::new(filename.as_str());
        let Ok(extension) = path.extension().and_then(OsStr::to_str).ok_or("") else {
          bail!("Unknown extension.")
        };
        let name = path.file_stem().and_then(OsStr::to_str).map(|x| sanitize_file_name(x.into()));
        let filename = &self.ctx.options.asset_filenames.render(&FileNameRenderOptions {
          name: name.as_deref(),
          hash: Some(&xxhash_base64_url(&bytes).as_str()[..8]),
          ext: Some(extension),
        });
        if let ModuleType::File = module_type {
          assets.push(OutputAsset {
            filename: filename.as_str().to_string(),
            source: AssetSource::Buffer(bytes.clone()),
            name,
          });
        }
        StrOrBytes::from(bytes)
      }
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

    let ret = EcmaModuleFactory::create_module(
      &mut CreateModuleContext {
        module_index: self.module_idx,
        plugin_driver: &self.ctx.plugin_driver,
        resolved_id: &self.resolved_id,
        options: &self.ctx.options,
        warnings: &mut warnings,
        module_type: module_type.clone(),
        resolver: &self.ctx.resolver,
        assets: &assets,
        is_user_defined_entry: self.is_user_defined_entry,
        replace_global_define_config: self.ctx.meta.replace_global_define_config.clone(),
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
        assets,
      }))
      .await
    {
      // The main thread is dead, nothing we can do to handle these send failures.
    }

    Ok(())
  }
}
