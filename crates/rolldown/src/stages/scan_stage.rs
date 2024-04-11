use std::sync::Arc;

use futures::future::join_all;
use index_vec::IndexVec;
use rolldown_common::{EntryPoint, ImportKind, IntoBatchedResult, NormalModuleId};
use rolldown_error::BuildError;
use rolldown_fs::OsFileSystem;
use rolldown_oxc_utils::OxcAst;
use rolldown_plugin::{HookResolveIdExtraOptions, SharedPluginDriver};

use crate::{
  module_loader::{module_loader::ModuleLoaderOutput, ModuleLoader},
  runtime::RuntimeModuleBrief,
  types::{
    module_table::ModuleTable, resolved_request_info::ResolvedRequestInfo, symbols::Symbols,
  },
  utils::resolve_id::resolve_id,
  SharedOptions, SharedResolver,
};
use index_vec::IndexVec;
use rolldown_common::{EntryPoint, ImportKind, NormalModuleId};
use rolldown_error::{collect_results, BuildError};
use rolldown_fs::OsFileSystem;
use rolldown_oxc_utils::OxcAst;
use rolldown_plugin::{HookResolveIdExtraOptions, SharedPluginDriver};
use rolldown_utils::block_on_spawn_all;

pub struct ScanStage {
  input_options: SharedOptions,
  plugin_driver: SharedPluginDriver,
  fs: OsFileSystem,
  resolver: SharedResolver,
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
    Self { input_options, plugin_driver, fs, resolver }
  }

  #[tracing::instrument(skip_all)]
  pub async fn scan(&self) -> ScanStageOutput {
    tracing::info!("Start scan stage");
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    let mut all_errors = vec![];
    let mut module_loader = ModuleLoader::new(
      Arc::clone(&self.input_options),
      Arc::clone(&self.plugin_driver),
      self.fs.clone(),
      Arc::clone(&self.resolver),
    );

    module_loader.try_spawn_runtime_module_task();

    let user_entries = self.resolve_user_defined_entries().await?;

    let ModuleLoaderOutput {
      module_table,
      entry_points,
      symbols,
      runtime,
      warnings,
      errors,
      ast_table,
    } = module_loader.fetch_all_modules(user_entries).await;
    all_errors.extend(errors);

    tracing::debug!("Scan stage finished {module_table:#?}");

    ScanStageOutput {
      module_table,
      entry_points,
      symbols,
      runtime,
      warnings,
      ast_table,
      errors: all_errors,
    }
  }

  /// Resolve `InputOptions.input`
  #[tracing::instrument(skip_all)]
  async fn resolve_user_defined_entries(
    &self,
  ) -> BatchedResult<Vec<(Option<String>, ResolvedRequestInfo)>> {
    let resolver = &self.resolver;
    let plugin_driver = &self.plugin_driver;

    let resolved_ids = join_all(self.input_options.input.iter().map(|input_item| async move {
      let specifier = &input_item.import;
      match resolve_id(
        resolver,
        plugin_driver,
        specifier,
        None,
        HookResolveIdExtraOptions { is_entry: true, kind: ImportKind::Import },
        false,
      )
      .await
      {
        Ok(info) => {
          if info.is_external {
            return Err(BuildError::entry_cannot_be_external(&*info.path.path));
          }
          Ok((input_item.name.clone(), info))
        }
        Err(e) => Err(e),
      }
    }))
    .await;

    collect_results(resolved_ids)
  }
}
