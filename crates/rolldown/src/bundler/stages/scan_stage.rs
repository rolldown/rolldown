use std::sync::Arc;

use rolldown_common::{ImportKind, ModuleId};
use rolldown_error::BuildError;
use rolldown_fs::FileSystem;
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
  error::{BatchedErrors, BatchedResult},
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
  pub entries: Vec<(Option<String>, ModuleId)>,
  pub symbols: Symbols,
  pub runtime: RuntimeModuleBrief,
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

    let ModuleLoaderOutput { modules, entries, symbols, runtime } =
      module_loader.fetch_all_modules(&user_entries).await?;

    tracing::debug!("Scan stage finished {modules:#?}");

    Ok(ScanStageOutput { modules, entries, symbols, runtime })
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
              return Err(BuildError::entry_cannot_be_external(info.path.as_str()));
            }
            Ok((input_item.name.clone(), info))
          }
          Err(e) => Err(e),
        }
      }));

    let mut errors = BatchedErrors::default();

    let collected =
      resolved_ids.into_iter().filter_map(|item| errors.take_err_from(item)).collect();

    if errors.is_empty() {
      Ok(collected)
    } else {
      Err(errors)
    }
  }
}
