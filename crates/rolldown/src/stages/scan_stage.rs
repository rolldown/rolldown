use std::sync::Arc;

use arcstr::ArcStr;
use futures::future::join_all;
use rolldown_common::{
  dynamic_import_usage::DynamicImportExportsUsage, EntryPoint, ImportKind, ModuleIdx, ModuleTable,
  ResolvedId, RuntimeModuleBrief, SymbolRefDb,
};
use rolldown_error::{BuildDiagnostic, BuildResult, ResultExt};
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_resolver::ResolveError;
use rustc_hash::FxHashMap;

use crate::{
  module_loader::{module_loader::ModuleLoaderOutput, ModuleLoader},
  type_alias::IndexEcmaAst,
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
  pub dynamic_import_exports_usage_map: FxHashMap<ModuleIdx, DynamicImportExportsUsage>,
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
  pub async fn scan(&mut self) -> BuildResult<ScanStageOutput> {
    if self.options.input.is_empty() {
      Err(anyhow::anyhow!("You must supply options.input to rolldown"))?;
    }

    self.plugin_driver.build_start(&self.options).await?;

    let module_loader = ModuleLoader::new(
      self.fs,
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
    )?;

    let user_entries = self.resolve_user_defined_entries().await?;

    let ModuleLoaderOutput {
      module_table,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
      dynamic_import_exports_usage_map,
    } = module_loader.fetch_all_modules(user_entries).await?;

    Ok(ScanStageOutput {
      module_table,
      entry_points,
      symbol_ref_db,
      runtime,
      warnings,
      index_ecma_ast,
      dynamic_import_exports_usage_map,
    })
  }

  /// Resolve `InputOptions.input`
  #[tracing::instrument(level = "debug", skip_all)]
  async fn resolve_user_defined_entries(
    &mut self,
  ) -> BuildResult<Vec<(Option<ArcStr>, ResolvedId)>> {
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
          ResolveError::NotFound(_) => {
            errors.push(BuildDiagnostic::unresolved_entry(args.specifier, None));
          }
          ResolveError::PackagePathNotExported(..) => {
            errors.push(BuildDiagnostic::unresolved_entry(args.specifier, Some(e)));
          }
          _ => return Err(e).map_err_to_unhandleable()?,
        },
      }
    }

    if !errors.is_empty() {
      Err(errors)?;
    }

    Ok(ret)
  }
}
