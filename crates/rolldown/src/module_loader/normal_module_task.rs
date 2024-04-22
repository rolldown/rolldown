use std::{path::Path, sync::Arc};

use anyhow::Result;
use futures::future::join_all;
use index_vec::IndexVec;
use oxc::span::SourceType;
use rolldown_common::{
  AstScope, FilePath, ImportRecordId, ModuleInfo, ModuleType, NormalModule, NormalModuleId,
  RawImportRecord, ResolvedPath, ResourceId, SymbolRef,
};
use rolldown_error::{BuildError, BuildResult};
use rolldown_oxc_utils::{OxcAst, OxcCompiler};
use rolldown_plugin::{HookResolveIdExtraOptions, SharedPluginDriver};
use sugar_path::SugarPath;

use super::{task_context::TaskContext, Msg};
use crate::{
  ast_scanner::{AstScanner, ScanResult},
  module_loader::NormalModuleTaskResult,
  types::{ast_symbols::AstSymbols, resolved_request_info::ResolvedRequestInfo},
  utils::{load_source::load_source, resolve_id::resolve_id, transform_source::transform_source},
  SharedOptions, SharedResolver,
};
pub struct NormalModuleTask {
  ctx: Arc<TaskContext>,
  module_id: NormalModuleId,
  resolved_path: ResolvedPath,
  module_type: ModuleType,
  errors: Vec<BuildError>,
}

impl NormalModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    id: NormalModuleId,
    path: ResolvedPath,
    module_type: ModuleType,
  ) -> Self {
    Self { ctx, module_id: id, resolved_path: path, module_type, errors: vec![] }
  }

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

  async fn run_inner(&mut self) -> Result<()> {
    tracing::trace!("process {:?}", self.resolved_path);

    let mut sourcemap_chain = vec![];
    let mut warnings = vec![];

    // Run plugin load to get content first, if it is None using read fs as fallback.
    let source =
      load_source(&self.ctx.plugin_driver, &self.resolved_path, &self.ctx.fs, &mut sourcemap_chain)
        .await?;

    // Run plugin transform.
    let source: Arc<str> =
      transform_source(&self.ctx.plugin_driver, &self.resolved_path, source, &mut sourcemap_chain)
        .await?
        .into();

    let (ast, scope, scan_result, ast_symbol, namespace_symbol) = self.scan(&source);
    tracing::trace!("scan {:?}", self.resolved_path);

    self
      .ctx
      .plugin_driver
      .module_parsed(Arc::new(ModuleInfo {
        code: Some(Arc::clone(&source)),
        id: Arc::clone(&self.resolved_path.path),
      }))
      .await?;

    let res = self.resolve_dependencies(&scan_result.import_records).await?;

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      import_records,
      star_exports,
      default_export_ref,
      imports,
      exports_kind,
      repr_name,
      warnings: scan_warnings,
    } = scan_result;
    warnings.extend(scan_warnings);

    let module = NormalModule {
      source,
      id: self.module_id,
      repr_name,
      resource_id: ResourceId::new(Arc::<str>::clone(&self.resolved_path.path).into()),
      named_imports,
      named_exports,
      stmt_infos,
      imports,
      star_exports,
      default_export_ref,
      scope,
      exports_kind,
      namespace_symbol,
      module_type: self.module_type,
      pretty_path: self.resolved_path.prettify(&self.ctx.input_options.cwd),
      sourcemap_chain,
      is_virtual: self.resolved_path.is_virtual_module_path(),
      exec_order: u32::MAX,
      is_user_defined_entry: false,
      import_records: IndexVec::default(),
      is_included: false,
    };

    self
      .ctx
      .tx
      .send(Msg::NormalModuleDone(NormalModuleTaskResult {
        resolved_deps: res,
        module_id: self.module_id,
        warnings,
        ast_symbol,
        module,
        raw_import_records: import_records,
        ast,
      }))
      .await
      .expect("Send should not fail");
    tracing::trace!("end process {:?}", self.resolved_path);
    Ok(())
  }

  fn scan(&self, source: &Arc<str>) -> (OxcAst, AstScope, ScanResult, AstSymbols, SymbolRef) {
    fn determine_oxc_source_type(path: impl AsRef<Path>, ty: ModuleType) -> SourceType {
      // Determine oxc source type for parsing
      let mut default = SourceType::default().with_module(true);
      // Rolldown considers module as esm by default.
      debug_assert!(default.is_module());
      debug_assert!(default.is_javascript());
      debug_assert!(!default.is_jsx());
      let extension = path.as_ref().extension().and_then(std::ffi::OsStr::to_str);
      default = match ty {
        ModuleType::CJS | ModuleType::CjsPackageJson => default.with_script(true),
        _ => default,
      };
      if let Some(ext) = extension {
        default = match ext {
          "cjs" => default.with_script(true),
          "jsx" => default.with_jsx(true),
          _ => default,
        };
      };
      default
    }

    let source_type =
      determine_oxc_source_type(self.resolved_path.path.as_path(), self.module_type);
    let mut program = OxcCompiler::parse(Arc::clone(source), source_type);

    let (mut symbol_table, scope) = program.make_symbol_table_and_scope_tree();
    let ast_scope = AstScope::new(
      scope,
      std::mem::take(&mut symbol_table.references),
      std::mem::take(&mut symbol_table.resolved_references),
    );
    let mut symbol_for_module = AstSymbols::from_symbol_table(symbol_table);
    let file_path = Arc::<str>::clone(&self.resolved_path.path).into();
    let repr_name = FilePath::representative_name(&file_path);
    let scanner = AstScanner::new(
      self.module_id,
      &ast_scope,
      &mut symbol_for_module,
      repr_name.into_owned(),
      self.module_type,
      source,
      &file_path,
    );
    let namespace_symbol = scanner.namespace_ref;
    program.hoist_import_export_from_stmts();
    let scan_result = scanner.scan(program.program());

    (program, ast_scope, scan_result, symbol_for_module, namespace_symbol)
  }

  #[allow(clippy::option_if_let_else)]
  pub(crate) async fn resolve_id(
    input_options: &SharedOptions,
    resolver: &SharedResolver,
    plugin_driver: &SharedPluginDriver,
    importer: &str,
    specifier: &str,
    options: HookResolveIdExtraOptions,
  ) -> anyhow::Result<BuildResult<ResolvedRequestInfo>> {
    // Check external with unresolved path
    if let Some(external) = input_options.external.as_ref() {
      if external.call(specifier.to_string(), Some(importer.to_string()), false).await? {
        return Ok(Ok(ResolvedRequestInfo {
          path: specifier.to_string().into(),
          module_type: ModuleType::Unknown,
          is_external: true,
        }));
      }
    }

    let info =
      resolve_id(resolver, plugin_driver, specifier, Some(importer), options, false).await?;

    match info {
      Ok(mut info) => {
        if !info.is_external {
          // Check external with resolved path
          if let Some(external) = input_options.external.as_ref() {
            info.is_external =
              external.call(specifier.to_string(), Some(importer.to_string()), true).await?;
          }
        }
        Ok(Ok(info))
      }
      Err(e) => Ok(Err(e)),
    }
  }

  #[tracing::instrument(skip_all)]
  async fn resolve_dependencies(
    &mut self,
    dependencies: &IndexVec<ImportRecordId, RawImportRecord>,
  ) -> Result<IndexVec<ImportRecordId, ResolvedRequestInfo>> {
    let jobs = dependencies.iter_enumerated().map(|(idx, item)| {
      let specifier = item.module_request.clone();
      let input_options = Arc::clone(&self.ctx.input_options);
      // FIXME(hyf0): should not use `Arc<Resolver>` here
      let resolver = Arc::clone(&self.ctx.resolver);
      let plugin_driver = Arc::clone(&self.ctx.plugin_driver);
      let importer = self.resolved_path.clone();
      let kind = item.kind;
      // let on_warn = self.input_options.on_warn.clone();
      async move {
        Self::resolve_id(
          &input_options,
          &resolver,
          &plugin_driver,
          &importer.path,
          &specifier,
          HookResolveIdExtraOptions { is_entry: false, kind },
        )
        .await
        .map(|id| (idx, id))
      }
    });

    let resolved_ids = join_all(jobs).await;

    let mut ret = IndexVec::with_capacity(dependencies.len());
    let mut build_errors = vec![];
    for handle in resolved_ids {
      let (_idx, handle) = handle?;
      match handle {
        Ok(info) => {
          ret.push(info);
        }
        Err(e) => {
          build_errors.push(e);
        }
      }
    }

    if build_errors.is_empty() {
      Ok(ret)
    } else {
      // TODO: The better way here is to filter out the failed-resolved dependencies and return
      // `Ok(ret)` with recoverable errors `self.errors` instead of returning `Err` and causing panicking.
      let resolved_err = anyhow::format_err!(
        "Resolver errors in {:?} => dependencies: {dependencies:#?}, build_errors: {build_errors:#?}",
        self.resolved_path
      );
      Err(resolved_err)
    }
  }
}
