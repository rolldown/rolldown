use std::{path::Path, sync::Arc};

use futures::future::join_all;
use index_vec::IndexVec;
use oxc::span::SourceType;
use rolldown_common::{
  AstScope, FilePath, ImportRecordId, ModuleType, NormalModuleId, RawImportRecord, ResolvedPath,
  ResourceId, SymbolRef,
};
use rolldown_fs::FileSystem;
use rolldown_oxc::{OxcCompiler, OxcProgram};
use rolldown_plugin::HookResolveIdArgsOptions;
use rolldown_resolver::Resolver;
use sugar_path::AsPath;

use super::{module_task_context::ModuleTaskCommonData, Msg};
use crate::{
  error::{BatchedErrors, BatchedResult},
  {
    ast_scanner::{AstScanner, ScanResult},
    module_loader::NormalModuleTaskResult,
    options::input_options::SharedInputOptions,
    plugin_driver::SharedPluginDriver,
    types::{
      ast_symbols::AstSymbols, normal_module_builder::NormalModuleBuilder,
      resolved_request_info::ResolvedRequestInfo,
    },
    utils::{load_source::load_source, resolve_id::resolve_id, transform_source::transform_source},
  },
};
pub struct NormalModuleTask<'task, T: FileSystem + Default> {
  ctx: &'task ModuleTaskCommonData<T>,
  module_id: NormalModuleId,
  resolved_path: ResolvedPath,
  module_type: ModuleType,
}

impl<'task, T: FileSystem + Default + 'static> NormalModuleTask<'task, T> {
  pub fn new(
    ctx: &'task ModuleTaskCommonData<T>,
    id: NormalModuleId,
    path: ResolvedPath,
    module_type: ModuleType,
  ) -> Self {
    Self { ctx, module_id: id, resolved_path: path, module_type }
  }
  pub async fn run(mut self) {
    if let Err(errs) = self.run_inner().await {
      self.ctx.tx.send(Msg::Errors(errs)).expect("Send should not fail");
    }
  }

  async fn run_inner(&mut self) -> BatchedResult<()> {
    tracing::trace!("process {:?}", self.resolved_path);

    let mut sourcemap_chain = vec![];
    let mut warnings = vec![];

    // Run plugin load to get content first, if it is None using read fs as fallback.
    let source =
      load_source(&self.ctx.plugin_driver, &self.resolved_path, &self.ctx.fs, &mut sourcemap_chain)
        .await?;

    // Run plugin transform.
    let source: Arc<str> =
      transform_source(&self.ctx.plugin_driver, source, &mut sourcemap_chain).await?.into();

    let (ast, scope, scan_result, ast_symbol, namespace_symbol) = self.scan(&source);
    tracing::trace!("scan {:?}", self.resolved_path);

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

    let builder = NormalModuleBuilder {
      source: Some(source),
      id: Some(self.module_id),
      repr_name: Some(repr_name),
      path: Some(ResourceId::new(self.resolved_path.path.clone())),
      named_imports: Some(named_imports),
      named_exports: Some(named_exports),
      stmt_infos: Some(stmt_infos),
      imports: Some(imports),
      star_exports: Some(star_exports),
      default_export_ref,
      scope: Some(scope),
      exports_kind: Some(exports_kind),
      namespace_symbol: Some(namespace_symbol),
      module_type: self.module_type,
      pretty_path: Some(self.resolved_path.prettify(&self.ctx.input_options.cwd)),
      sourcemap_chain,
      ..Default::default()
    };

    self
      .ctx
      .tx
      .send(Msg::NormalModuleDone(NormalModuleTaskResult {
        resolved_deps: res,
        module_id: self.module_id,
        warnings,
        ast_symbol,
        builder,
        raw_import_records: import_records,
        ast,
      }))
      .expect("Send should not fail");
    tracing::trace!("end process {:?}", self.resolved_path);
    Ok(())
  }

  fn scan(&self, source: &Arc<str>) -> (OxcProgram, AstScope, ScanResult, AstSymbols, SymbolRef) {
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

    let semantic = program.make_semantic(source_type);
    let (mut symbol_table, scope) = semantic.into_symbol_table_and_scope_tree();
    let ast_scope = AstScope::new(scope, std::mem::take(&mut symbol_table.references));
    let mut symbol_for_module = AstSymbols::from_symbol_table(symbol_table);
    let repr_name = self.resolved_path.path.representative_name();
    let scanner = AstScanner::new(
      self.module_id,
      &ast_scope,
      &mut symbol_for_module,
      repr_name.into_owned(),
      self.module_type,
      source,
      &self.resolved_path.path,
    );
    let namespace_symbol = scanner.namespace_ref;
    program.hoist_import_export_from_stmts();
    let scan_result = scanner.scan(program.program());

    (program, ast_scope, scan_result, symbol_for_module, namespace_symbol)
  }

  #[allow(clippy::option_if_let_else)]
  pub(crate) async fn resolve_id<F: FileSystem + Default>(
    input_options: &SharedInputOptions,
    resolver: &Resolver<F>,
    plugin_driver: &SharedPluginDriver,
    importer: &FilePath,
    specifier: &str,
    options: HookResolveIdArgsOptions,
  ) -> BatchedResult<ResolvedRequestInfo> {
    // Check external with unresolved path
    if input_options.external.call(specifier.to_string(), Some(importer.to_string()), false).await?
    {
      return Ok(ResolvedRequestInfo {
        path: specifier.to_string().into(),
        module_type: ModuleType::Unknown,
        is_external: true,
      });
    }

    let mut info =
      resolve_id(resolver, plugin_driver, specifier, Some(importer), options, false).await?;

    if !info.is_external {
      // Check external with resolved path
      info.is_external = input_options
        .external
        .call(specifier.to_string(), Some(importer.to_string()), true)
        .await?;
    }
    Ok(info)
  }

  #[tracing::instrument(skip_all)]
  async fn resolve_dependencies(
    &mut self,
    dependencies: &IndexVec<ImportRecordId, RawImportRecord>,
  ) -> BatchedResult<IndexVec<ImportRecordId, ResolvedRequestInfo>> {
    let jobs = dependencies.iter_enumerated().map(|(idx, item)| {
      let specifier = item.module_request.clone();
      let input_options = Arc::clone(&self.ctx.input_options);
      // FIXME(hyf0): should not use `Arc<Resolver>` here
      let resolver = Arc::clone(&self.ctx.resolver);
      let plugin_driver = Arc::clone(&self.ctx.plugin_driver);
      let importer = self.resolved_path.clone();
      let kind = item.kind;
      // let on_warn = self.input_options.on_warn.clone();
      tokio::spawn(async move {
        Self::resolve_id(
          &input_options,
          &resolver,
          &plugin_driver,
          &importer.path,
          &specifier,
          HookResolveIdArgsOptions { is_entry: false, kind },
        )
        .await
        .map(|id| (idx, id))
      })
    });

    let resolved_ids = join_all(jobs).await;

    let mut errors = BatchedErrors::default();
    let mut ret = IndexVec::with_capacity(dependencies.len());
    resolved_ids.into_iter().for_each(|handle| match handle.expect("Assuming no task panics") {
      Ok((_idx, item)) => {
        ret.push(item);
      }
      Err(e) => {
        errors.extend(e);
      }
    });
    debug_assert!(errors.is_empty() && ret.len() == dependencies.len(), "{dependencies:#?}");

    Ok(ret)
  }
}
