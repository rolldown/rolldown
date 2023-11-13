use std::sync::Arc;

use rolldown_common::{ImportKind, ModuleId};
use rolldown_error::BuildError;
use rolldown_fs::FileSystemExt;
use rolldown_utils::block_on_spawn_all;

use crate::{
  bundler::{
    graph::symbols::Symbols,
    module::ModuleVec,
    module_loader::ModuleLoader,
    plugin_driver::SharedPluginDriver,
    runtime::Runtime,
    utils::resolve_id::{resolve_id, ResolvedRequestInfo},
  },
  error::{BatchedErrors, BatchedResult},
  HookBuildEndArgs, HookResolveIdArgsOptions, InputOptions, SharedResolver,
};

pub struct BuildStage<'me, T: FileSystemExt + Default> {
  input_options: &'me InputOptions,
  plugin_driver: SharedPluginDriver,
  resolver: SharedResolver<T>,
}

pub struct BuildInfo {
  pub modules: ModuleVec,
  pub entries: Vec<(Option<String>, ModuleId)>,
  pub symbols: Symbols,
  pub runtime: Runtime,
}

impl<'me, T: FileSystemExt + Default> BuildStage<'me, T> {
  pub fn new(
    input_options: &'me InputOptions,
    plugin_driver: SharedPluginDriver,
    resolver: SharedResolver<T>,
  ) -> Self {
    Self { input_options, plugin_driver, resolver }
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

  pub async fn build<Fs: FileSystemExt + Default + 'static>(
    self,
    fs: Arc<Fs>,
  ) -> BatchedResult<BuildInfo> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    self.plugin_driver.build_start().await?;

    let build_info = self.build_inner(fs).await;

    if let Err(e) = build_info {
      let error = e.get().expect("should have a error");
      self
        .plugin_driver
        .build_end(Some(&HookBuildEndArgs {
          // TODO(hyf0): 1.Need a better way to expose the error, 2.How to handle multiple errors
          error: format!("{:?}\n{:?}", error.code(), error.to_diagnostic().print_to_string()),
        }))
        .await?;
      return Err(e);
    }

    self.plugin_driver.build_end(None).await?;
    build_info
  }
  async fn build_inner<Fs: FileSystemExt + Default + 'static>(
    &self,
    fs: Arc<Fs>,
  ) -> BatchedResult<BuildInfo> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    self.plugin_driver.build_start().await?;

    let resolved_entries = self.resolve_entries()?;

    let (modules, runtime, symbols, entries) =
      ModuleLoader::new(self.input_options, Arc::clone(&self.plugin_driver), fs)
        .fetch_all_modules(&resolved_entries)
        .await?;

    Ok(BuildInfo { modules, entries, symbols, runtime })
  }
}
