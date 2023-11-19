use std::{path::Path, sync::Arc};

use futures::future::join_all;
use index_vec::IndexVec;
use oxc::span::SourceType;
use rolldown_common::{
  FilePath, ImportRecordId, ModuleId, ModuleType, RawImportRecord, ResourceId, SymbolRef,
};
use rolldown_fs::FileSystem;
use rolldown_oxc::{OxcCompiler, OxcProgram};
use rolldown_resolver::Resolver;
use sugar_path::AsPath;

use super::{module_task_context::ModuleTaskCommonData, Msg};
use crate::{
  bundler::{
    module::normal_module_builder::NormalModuleBuilder,
    module_loader::NormalModuleTaskResult,
    options::input_options::SharedInputOptions,
    plugin_driver::SharedPluginDriver,
    utils::{
      ast_scope::AstScope,
      ast_symbol::AstSymbol,
      resolve_id::{resolve_id, ResolvedRequestInfo},
    },
    visitors::scanner::{self, ScanResult},
  },
  error::{BatchedErrors, BatchedResult},
  HookLoadArgs, HookResolveIdArgsOptions, HookTransformArgs,
};
pub struct NormalModuleTask<'task, T: FileSystem + Default> {
  ctx: &'task ModuleTaskCommonData<T>,
  module_id: ModuleId,
  path: FilePath,
  module_type: ModuleType,
  is_entry: bool,
}

impl<'task, T: FileSystem + Default + 'static> NormalModuleTask<'task, T> {
  pub fn new(
    ctx: &'task ModuleTaskCommonData<T>,
    id: ModuleId,
    is_entry: bool,
    path: FilePath,
    module_type: ModuleType,
  ) -> Self {
    Self { ctx, module_id: id, is_entry, path, module_type }
  }
  pub async fn run(mut self) {
    if let Err(errs) = self.run_inner().await {
      self.ctx.tx.send(Msg::Errors(errs)).expect("Send should not fail");
    }
  }

  async fn run_inner(&mut self) -> BatchedResult<()> {
    tracing::trace!("process {:?}", self.path);

    let mut warnings = vec![];

    // Run plugin load to get content first, if it is None using read fs as fallback.
    let mut source = if let Some(r) =
      self.ctx.plugin_driver.load(&HookLoadArgs { id: self.path.as_ref() }).await?
    {
      r.code
    } else {
      self.ctx.fs.read_to_string(self.path.as_path())?
    };

    // Run plugin transform.
    if let Some(r) = self
      .ctx
      .plugin_driver
      .transform(&HookTransformArgs { id: self.path.as_ref(), code: &source })
      .await?
    {
      source = r.code;
    }

    let (ast, scope, scan_result, ast_symbol, namespace_symbol) = self.scan(source);

    let res = self.resolve_dependencies(&scan_result.import_records).await?;

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      import_records,
      star_exports,
      export_default_symbol_id,
      imports,
      exports_kind,
      repr_name,
      warnings: scan_warnings,
    } = scan_result;
    warnings.extend(scan_warnings);

    let builder = NormalModuleBuilder {
      id: Some(self.module_id),
      ast: Some(ast),
      repr_name: Some(repr_name),
      path: Some(ResourceId::new(self.path.clone())),
      named_imports: Some(named_imports),
      named_exports: Some(named_exports),
      stmt_infos: Some(stmt_infos),
      imports: Some(imports),
      star_exports: Some(star_exports),
      default_export_symbol: export_default_symbol_id,
      scope: Some(scope),
      exports_kind: Some(exports_kind),
      namespace_symbol: Some(namespace_symbol),
      module_type: self.module_type,
      is_entry: self.is_entry,
      pretty_path: Some(self.path.prettify(&self.ctx.input_options.cwd)),
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
      }))
      .expect("Send should not fail");
    Ok(())
  }

  fn scan(&self, source: String) -> (OxcProgram, AstScope, ScanResult, AstSymbol, SymbolRef) {
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
    let source: Arc<str> = source.into();

    let source_type = determine_oxc_source_type(self.path.as_path(), self.module_type);
    let program = OxcCompiler::parse(Arc::clone(&source), source_type);

    let semantic = program.make_semantic(source_type);
    let (mut symbol_table, scope) = semantic.into_symbol_table_and_scope_tree();
    let ast_scope = AstScope::new(scope, std::mem::take(&mut symbol_table.references));
    let mut symbol_for_module = AstSymbol::from_symbol_table(symbol_table);
    let repr_name = self.path.representative_name();
    let scanner = scanner::Scanner::new(
      self.module_id,
      &ast_scope,
      &mut symbol_for_module,
      repr_name.into_owned(),
      self.module_type,
      &source,
      &self.path,
    );
    let namespace_symbol = scanner.namespace_symbol;
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
      let importer = self.path.clone();
      let kind = item.kind;
      // let on_warn = self.input_options.on_warn.clone();
      tokio::spawn(async move {
        Self::resolve_id(
          &input_options,
          &resolver,
          &plugin_driver,
          &importer,
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
        errors.merge(e);
      }
    });
    debug_assert!(errors.is_empty() && ret.len() == dependencies.len(), "{dependencies:#?}");

    Ok(ret)
  }
}
