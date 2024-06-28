use std::sync::Arc;

use anyhow::Result;
use futures::future::join_all;
use oxc::index::IndexVec;
use rolldown_common::{EntryPoint, ImportKind, ModuleTable, NormalModuleId, ResolvedRequestInfo};
use rolldown_error::BuildError;
use rolldown_fs::OsFileSystem;
use rolldown_oxc_utils::OxcAst;
use rolldown_plugin::{HookResolveIdExtraOptions, SharedPluginDriver};
use rolldown_resolver::ResolveError;

use crate::{
  module_loader::{module_loader::ModuleLoaderOutput, ModuleLoader},
  runtime::RuntimeModuleBrief,
  types::symbols::Symbols,
  utils::resolve_id::resolve_id,
  SharedOptions, SharedResolver,
};

pub struct ScanStage {
  input_options: SharedOptions,
  plugin_driver: SharedPluginDriver,
  fs: OsFileSystem,
  resolver: SharedResolver,
  pub errors: Vec<BuildError>,
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub module_table: ModuleTable,
  pub ast_table: IndexVec<NormalModuleId, OxcAst>,
  pub entry_points: Vec<EntryPoint>,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildError>,
  pub errors: Vec<BuildError>,
}

impl ScanStage {
  pub fn new(
    input_options: SharedOptions,
    plugin_driver: SharedPluginDriver,
    fs: OsFileSystem,
    resolver: SharedResolver,
  ) -> Self {
    Self { input_options, plugin_driver, fs, resolver, errors: vec![] }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn scan(&mut self) -> anyhow::Result<ScanStageOutput> {
    if self.input_options.input.is_empty() {
      return Err(anyhow::format_err!("You must supply options.input to rolldown"));
    }

    let module_loader = ModuleLoader::new(
      Arc::clone(&self.input_options),
      Arc::clone(&self.plugin_driver),
      self.fs,
      Arc::clone(&self.resolver),
    );

    let user_entries = self.resolve_user_defined_entries().await?;

    let ModuleLoaderOutput {
      module_table,
      entry_points,
      symbols,
      runtime,
      warnings,
      errors,
      ast_table,
    } = module_loader.fetch_all_modules(user_entries).await?;
    self.errors.extend(errors);

    Ok(ScanStageOutput {
      module_table,
      entry_points,
      symbols,
      runtime,
      warnings,
      ast_table,
      errors: std::mem::take(&mut self.errors),
    })
  }

  /// Resolve `InputOptions.input`

  #[tracing::instrument(level = "debug", skip_all)]
  async fn resolve_user_defined_entries(&mut self) -> Result<Vec<(String, ResolvedRequestInfo)>> {
    let resolver = &self.resolver;
    let plugin_driver = &self.plugin_driver;

    let resolved_ids = join_all(self.input_options.input.iter().map(|input_item| async move {
      struct Args<'a> {
        specifier: &'a str,
      }
      let args = Args { specifier: &input_item.import };
      let resolved = resolve_id(
        resolver,
        plugin_driver,
        args.specifier,
        None,
        HookResolveIdExtraOptions { is_entry: true, kind: ImportKind::Import },
      )
      .await;

      resolved.map(|info| (args, info.map(|info| (input_item.name.clone(), info))))
    }))
    .await;

    let mut ret = Vec::with_capacity(self.input_options.input.len());

    for resolve_id in resolved_ids {
      let (args, resolve_id) = resolve_id?;

      match resolve_id {
        Ok(item) => {
          ret.push(item);
        }
        Err(e) => match e {
          ResolveError::NotFound(..) => {
            self.errors.push(BuildError::unresolved_entry(args.specifier, None));
          }
          ResolveError::PackagePathNotExported(..) => {
            self.errors.push(BuildError::unresolved_entry(args.specifier, Some(e)));
          }
          _ => {
            return Err(e.into());
          }
        },
      }
    }
    Ok(ret)
  }
}
