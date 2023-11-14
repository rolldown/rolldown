use std::sync::Arc;

use rolldown_common::{ImportKind, ModuleId};
use rolldown_error::BuildError;
use rolldown_fs::FileSystem;
use rolldown_utils::block_on_spawn_all;

use crate::{
  bundler::{
    module::ModuleVec,
    module_loader::ModuleLoader,
    plugin_driver::SharedPluginDriver,
    runtime::Runtime,
    utils::{
      resolve_id::{resolve_id, ResolvedRequestInfo},
      symbols::Symbols,
    },
  },
  error::{BatchedErrors, BatchedResult},
  HookResolveIdArgsOptions, InputOptions, SharedResolver,
};

pub struct ScanStage<'me, Fs: FileSystem + Default> {
  input_options: &'me InputOptions,
  plugin_driver: SharedPluginDriver,
  fs: Fs,
  resolver: SharedResolver<Fs>,
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub modules: ModuleVec,
  pub entries: Vec<(Option<String>, ModuleId)>,
  pub symbols: Symbols,
  pub runtime: Runtime,
}

impl<'me, Fs: FileSystem + Default + 'static> ScanStage<'me, Fs> {
  pub fn new(
    input_options: &'me InputOptions,
    plugin_driver: SharedPluginDriver,
    fs: Fs,
    resolver: SharedResolver<Fs>,
  ) -> Self {
    Self { input_options, plugin_driver, fs, resolver }
  }

  fn resolve_entries(&self) -> BatchedResult<Vec<(Option<String>, ResolvedRequestInfo)>> {
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
          Ok(r) => {
            let Some(info) = r else {
              return Err(BuildError::unresolved_entry(specifier));
            };

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

  pub async fn scan(&self) -> BatchedResult<ScanStageOutput> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    let mut module_loader = ModuleLoader::new(
      self.input_options,
      Arc::clone(&self.plugin_driver),
      self.fs.share(),
      Arc::clone(&self.resolver),
    );

    let resolved_entries = self.resolve_entries()?;

    let mut runtime = Runtime::new(module_loader.try_spawn_runtime_module_task());

    let (modules, symbols, entries) =
      module_loader.fetch_all_modules(&resolved_entries, &mut runtime).await?;

    Ok(ScanStageOutput { modules, entries, symbols, runtime })
  }
}
