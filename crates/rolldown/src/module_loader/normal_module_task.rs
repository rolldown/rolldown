use std::{path::Path, sync::Arc};

use futures::future::join_all;
use index_vec::IndexVec;
use oxc::span::SourceType;
use rolldown_common::{
  AstScope, FilePath, ImportRecordId, ModuleType, NormalModuleId, RawImportRecord, ResolvedPath,
  ResourceId, SymbolRef,
};
use rolldown_oxc_utils::{OxcCompiler, OxcProgram};
use rolldown_plugin::{HookResolveIdExtraOptions, SharedPluginDriver};
use sugar_path::AsPath;

use super::{module_task_context::ModuleTaskCommonData, Msg};
use crate::{
  ast_scanner::{AstScanner, ScanResult},
  error::{BatchedErrors, BatchedResult},
  module_loader::NormalModuleTaskResult,
  options::normalized_input_options::SharedNormalizedInputOptions,
  types::{
    ast_symbols::AstSymbols, normal_module_builder::NormalModuleBuilder,
    resolved_request_info::ResolvedRequestInfo,
  },
  utils::{load_source::load_source, resolve_id::resolve_id, transform_source::transform_source},
  SharedResolver,
};
pub struct NormalModuleTask<'task> {
  ctx: &'task ModuleTaskCommonData,
  module_id: NormalModuleId,
  resolved_path: ResolvedPath,
  module_type: ModuleType,
  errors: BatchedErrors,
}

impl<'task> NormalModuleTask<'task> {
  pub fn new(
    ctx: &'task ModuleTaskCommonData,
    id: NormalModuleId,
    path: ResolvedPath,
    module_type: ModuleType,
  ) -> Self {
    Self { ctx, module_id: id, resolved_path: path, module_type, errors: BatchedErrors::default() }
  }

  pub async fn run(mut self) {
    match self.run_inner().await {
      Ok(()) => {
        if !self.errors.is_empty() {
          self.ctx.tx.send(Msg::BuildErrors(self.errors)).expect("Send should not fail");
        }
      }
      Err(err) => {
        self.ctx.tx.send(Msg::Panics(err)).expect("Send should not fail");
      }
    }
  }

  async fn run_inner(&mut self) -> anyhow::Result<()> {
    tracing::trace!("process {:?}", self.resolved_path);

    let mut sourcemap_chain = vec![];
    let mut warnings = vec![];

    // Run plugin load to get content first, if it is None using read fs as fallback.
    let source = match load_source(
      &self.ctx.plugin_driver,
      &self.resolved_path,
      &self.ctx.fs,
      &mut sourcemap_chain,
    )
    .await
    {
      Ok(ret) => ret,
      Err(errs) => {
        self.errors.extend(errs);
        return Ok(());
      }
    };

    // Run plugin transform.
    let source: Arc<str> = match transform_source(
      &self.ctx.plugin_driver,
      &self.resolved_path,
      source,
      &mut sourcemap_chain,
    )
    .await
    {
      Ok(ret) => ret.into(),
      Err(errs) => {
        self.errors.extend(errs);
        return Ok(());
      }
    };

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
      path: Some(ResourceId::new(Arc::<str>::clone(&self.resolved_path.path).into())),
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
    input_options: &SharedNormalizedInputOptions,
    resolver: &SharedResolver,
    plugin_driver: &SharedPluginDriver,
    importer: &str,
    specifier: &str,
    options: HookResolveIdExtraOptions,
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
  ) -> anyhow::Result<IndexVec<ImportRecordId, ResolvedRequestInfo>> {
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
          HookResolveIdExtraOptions { is_entry: false, kind },
        )
        .await
        .map(|id| (idx, id))
      })
    });

    let resolved_ids = join_all(jobs).await;

    let mut errors = BatchedErrors::default();
    let mut ret = IndexVec::with_capacity(dependencies.len());
    resolved_ids.into_iter().try_for_each(|handle| -> anyhow::Result<()> {
      let handle = handle?;
      match handle {
        Ok((_idx, item)) => {
          ret.push(item);
        }
        Err(e) => {
          errors.extend(e);
        }
      }
      Ok(())
    })?;

    if errors.is_empty() {
      Ok(ret)
    } else {
      // TODO: The better way here is to filter out the failed-resolved dependencies and return
      // `Ok(rwt)` with recoverable errors `self.errors` instead of returning `Err` and causing panicking.
      let resolved_err = anyhow::format_err!(
        "Resolver errors in {:?} => dependencies: {dependencies:#?}, errors: {errors:#?}",
        self.resolved_path
      );
      self.errors.extend(errors);
      Err(resolved_err)
    }
  }
}
