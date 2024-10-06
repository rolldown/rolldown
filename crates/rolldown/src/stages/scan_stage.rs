use std::sync::Arc;

use anyhow::Result;
use arcstr::ArcStr;
use futures::future::join_all;
use rolldown_common::{EntryPoint, ImportKind, ModuleTable, ResolvedId};
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_resolver::ResolveError;

use crate::{
  module_loader::{module_loader::ModuleLoaderOutput, ModuleLoader},
  runtime::RuntimeModuleBrief,
  type_alias::IndexEcmaAst,
  types::symbol_ref_db::SymbolRefDb,
  utils::resolve_id::resolve_id,
  SharedOptions, SharedResolver,
};

pub struct ScanStage {
  options: SharedOptions,
  plugin_driver: SharedPluginDriver,
  fs: OsFileSystem,
  resolver: SharedResolver,
}

#[derive(Debug)]
pub struct ScanStageOutput {
  pub module_table: ModuleTable,
  pub index_ecma_ast: IndexEcmaAst,
  pub entry_points: Vec<EntryPoint>,
  pub symbol_ref_db: SymbolRefDb,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
}

impl ScanStage {
  pub fn new(
    options: SharedOptions,
    plugin_driver: SharedPluginDriver,
    fs: OsFileSystem,
    resolver: SharedResolver,
  ) -> Self {
    Self { options, plugin_driver, fs, resolver }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn scan(&mut self) -> anyhow::Result<DiagnosableResult<ScanStageOutput>> {
    if self.options.input.is_empty() {
      return Err(anyhow::format_err!("You must supply options.input to rolldown"));
    }

    let module_loader = ModuleLoader::new(
      Arc::clone(&self.options),
      Arc::clone(&self.plugin_driver),
      self.fs,
      Arc::clone(&self.resolver),
    )?;

    let user_entries = match self.resolve_user_defined_entries().await? {
      Ok(entries) => entries,
      Err(errors) => {
        return Ok(Err(errors));
      }
    };

    let ModuleLoaderOutput {
      module_table,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
    } = match module_loader.fetch_all_modules(user_entries).await? {
      Ok(output) => output,
      Err(errors) => {
        return Ok(Err(errors));
      }
    };

    Ok(Ok(ScanStageOutput {
      module_table,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
      errors: vec![],
    }))
  }

  /// Resolve `InputOptions.input`

  #[tracing::instrument(level = "debug", skip_all)]
  #[allow(clippy::type_complexity)]
  async fn resolve_user_defined_entries(
    &mut self,
  ) -> Result<DiagnosableResult<Vec<(Option<ArcStr>, ResolvedId)>>> {
    let resolver = &self.resolver;
    let plugin_driver = &self.plugin_driver;

    let resolved_ids = join_all(self.options.input.iter().map(|input_item| async move {
      struct Args<'a> {
        specifier: &'a str,
      }
      let args = Args { specifier: &input_item.import };
      let resolved = resolve_id(
        resolver,
        plugin_driver,
        args.specifier,
        None,
        true,
        ImportKind::Import,
        None,
        Arc::default(),
        true,
      )
      .await;

      resolved
        .map(|info| (args, info.map(|info| ((input_item.name.clone().map(ArcStr::from)), info))))
    }))
    .await;

    let mut ret = Vec::with_capacity(self.options.input.len());

    let mut errors = vec![];

    for resolve_id in resolved_ids {
      let (args, resolve_id) = resolve_id?;

      match resolve_id {
        Ok(item) => {
          if item.1.is_external {
            errors.push(BuildDiagnostic::entry_cannot_be_external(item.1.id.to_string()));
            continue;
          }
          ret.push(item);
        }
        Err(e) => match e {
          ResolveError::NotFound(..) => {
            errors.push(BuildDiagnostic::unresolved_entry(args.specifier, None));
          }
          ResolveError::PackagePathNotExported(..) => {
            errors.push(BuildDiagnostic::unresolved_entry(args.specifier, Some(e)));
          }
          _ => {
            return Err(e.into());
          }
        },
      }
    }

    if !errors.is_empty() {
      return Ok(Err(errors));
    }

    Ok(Ok(ret))
  }
}
