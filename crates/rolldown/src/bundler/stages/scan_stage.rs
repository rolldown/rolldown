use std::sync::Arc;

use index_vec::IndexVec;
use rolldown_common::{EntryPoint, ImportKind, ModuleId};
use rolldown_error::BuildError;
use rolldown_fs::FileSystem;
use rolldown_oxc::OxcProgram;
use rolldown_utils::block_on_spawn_all;

use crate::{
  bundler::{
    module::ModuleVec,
    module_loader::{module_loader::ModuleLoaderOutput, ModuleLoader},
    options::input_options::SharedInputOptions,
    plugin_driver::SharedPluginDriver,
    runtime::RuntimeModuleBrief,
    utils::{
      resolve_id::{resolve_id, ResolvedRequestInfo},
      symbols::Symbols,
    },
  },
  error::{into_batched_result, BatchedResult},
  HookResolveIdArgsOptions, SharedResolver,
};

pub struct ScanStage<Fs: FileSystem + Default> {
  input_options: SharedInputOptions,
  plugin_driver: SharedPluginDriver,
  fs: Fs,
  resolver: SharedResolver<Fs>,
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub modules: ModuleVec,
  pub ast_table: IndexVec<ModuleId, OxcProgram>,
  pub entry_points: Vec<EntryPoint>,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildError>,
}

impl<Fs: FileSystem + Default + 'static> ScanStage<Fs> {
  pub fn new(
    input_options: SharedInputOptions,
    plugin_driver: SharedPluginDriver,
    fs: Fs,
    resolver: SharedResolver<Fs>,
  ) -> Self {
    Self { input_options, plugin_driver, fs, resolver }
  }

  #[tracing::instrument(skip_all)]
  pub async fn scan(&self) -> BatchedResult<ScanStageOutput> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    let mut module_loader = ModuleLoader::new(
      Arc::clone(&self.input_options),
      Arc::clone(&self.plugin_driver),
      self.fs.share(),
      Arc::clone(&self.resolver),
    );

    module_loader.try_spawn_runtime_module_task();

    let user_entries = self.resolve_user_defined_entries()?;

    let ModuleLoaderOutput { modules, entry_points, symbols, runtime, warnings, ast_table } =
      module_loader.fetch_all_modules(user_entries).await?;

    tracing::debug!("Scan stage finished {modules:#?}");

    Ok(ScanStageOutput { modules, entry_points, symbols, runtime, warnings, ast_table })
  }

  /// Resolve `InputOptions.input`
  #[tracing::instrument(skip_all)]
  fn resolve_user_defined_entries(
    &self,
  ) -> BatchedResult<Vec<(Option<String>, ResolvedRequestInfo)>> {
    let resolver = &self.resolver;
    let plugin_driver = &self.plugin_driver;

    let resolved_ids =
      block_on_spawn_all(self.input_options.input.iter().map(|input_item| async move {
        let specifier = &input_item.import;
        match resolve_id(
          resolver,
          plugin_driver,
          specifier,
          None,
          HookResolveIdArgsOptions { is_entry: true, kind: ImportKind::Import },
          false,
        )
        .await
        {
          Ok(info) => {
            if info.is_external {
              return Err(BuildError::entry_cannot_be_external(info.path.path.as_str()));
            }
            Ok((input_item.name.clone(), info))
          }
          Err(e) => Err(e),
        }
      }));

    into_batched_result(resolved_ids)
  }
}
