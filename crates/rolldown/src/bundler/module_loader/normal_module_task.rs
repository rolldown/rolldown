use std::{path::Path, sync::Arc};

use futures::future::join_all;
use index_vec::IndexVec;
use oxc::{ast::Visit, span::SourceType};
use rolldown_common::{ImportRecord, ImportRecordId, ModuleId, ModuleType, ResourceId, SymbolRef};
use rolldown_error::BuildError;
use rolldown_oxc::{OxcCompiler, OxcProgram};
use rolldown_resolver::Resolver;
use sugar_path::AsPath;

use super::{module_task_context::ModuleTaskContext, Msg};
use crate::bundler::{
  module::normal_module_builder::NormalModuleBuilder,
  module_loader::NormalModuleTaskResult,
  utils::{
    ast_scope::AstScope,
    ast_symbol::AstSymbol,
    resolve_id::{resolve_id, ResolvedRequestInfo},
  },
  visitors::scanner::{self, ScanResult},
};
pub struct NormalModuleTask<'task> {
  ctx: &'task ModuleTaskContext<'task>,
  module_id: ModuleId,
  path: ResourceId,
  module_type: ModuleType,
  errors: Vec<BuildError>,
  warnings: Vec<BuildError>,
  is_entry: bool,
}

impl<'task> NormalModuleTask<'task> {
  pub fn new(
    ctx: &'task ModuleTaskContext<'task>,
    id: ModuleId,
    is_entry: bool,
    path: ResourceId,
    module_type: ModuleType,
  ) -> Self {
    Self {
      ctx,
      module_id: id,
      is_entry,
      path,
      module_type,
      errors: Vec::default(),
      warnings: Vec::default(),
    }
  }

  pub async fn run(mut self) -> anyhow::Result<()> {
    let mut builder = NormalModuleBuilder::default();
    tracing::trace!("process {:?}", self.path);
    // load
    let source = self.ctx.fs.read_to_string(self.path.as_path())?;
    // TODO: transform

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
      unique_name,
    } = scan_result;

    builder.id = Some(self.module_id);
    builder.ast = Some(ast);
    builder.unique_name = Some(unique_name);
    builder.path = Some(self.path);
    builder.named_imports = Some(named_imports);
    builder.named_exports = Some(named_exports);
    builder.stmt_infos = Some(stmt_infos);
    builder.import_records = Some(import_records);
    builder.imports = Some(imports);
    builder.star_exports = Some(star_exports);
    builder.default_export_symbol = export_default_symbol_id;
    builder.scope = Some(scope);
    builder.exports_kind = exports_kind;
    builder.namespace_symbol = Some(namespace_symbol);
    builder.module_type = self.module_type;
    builder.is_entry = self.is_entry;
    self
      .ctx
      .tx
      .send(Msg::NormalModuleDone(NormalModuleTaskResult {
        resolved_deps: res,
        module_id: self.module_id,
        errors: self.errors,
        warnings: self.warnings,
        ast_symbol,
        builder,
      }))
      .unwrap();
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

    let source_type = determine_oxc_source_type(self.path.as_path(), self.module_type);
    let program = OxcCompiler::parse(source, source_type);

    let semantic = program.make_semantic(source_type);
    let (mut symbol_table, scope) = semantic.into_symbol_table_and_scope_tree();
    let ast_scope = AstScope::new(scope, std::mem::take(&mut symbol_table.references));
    let mut symbol_for_module = AstSymbol::from_symbol_table(symbol_table);
    let unique_name = self.path.generate_unique_name();
    let mut scanner = scanner::Scanner::new(
      self.module_id,
      &ast_scope,
      &mut symbol_for_module,
      unique_name,
      self.module_type,
    );
    scanner.visit_program(program.program());
    let scan_result = scanner.result;
    let namespace_symbol = scanner.namespace_symbol;
    (program, ast_scope, scan_result, symbol_for_module, namespace_symbol)
  }

  #[allow(clippy::option_if_let_else)]
  pub(crate) async fn resolve_id<F: FileSystemExt + Default>(
    resolver: &Resolver<F>,
    importer: &ResourceId,
    specifier: &str,
  ) -> Result<ResolvedRequestInfo, BuildError> {
    // let is_marked_as_external = is_external(specifier, Some(importer.id()), false).await?;

    // if is_marked_as_external {
    //     return Ok(ModuleId::new(specifier, true));
    // }

    let resolved_id = resolve_id(resolver, specifier, Some(importer), false).await?;

    match resolved_id {
      Some(info) => Ok(info),
      None => {
        Ok(ResolvedRequestInfo {
          path: specifier.to_string().into(),
          module_type: ModuleType::Unknown,
          is_external: true,
        })
        // // TODO: should emit warnings like https://rollupjs.org/guide/en#warning-treating-module-as-external-dependency
        // return Err(rolldown_error::Error::unresolved_import(
        //   specifier.to_string(),
        //   importer.prettify(),
        // ));
      }
    }
  }

  #[allow(clippy::collection_is_never_read)]
  async fn resolve_dependencies(
    &mut self,
    dependencies: &IndexVec<ImportRecordId, ImportRecord>,
  ) -> anyhow::Result<Vec<(ImportRecordId, ResolvedRequestInfo)>> {
    let jobs = dependencies.iter_enumerated().map(|(idx, item)| {
      let specifier = item.module_request.clone();
      // FIXME(hyf0): should not use `Arc<Resolver>` here
      let resolver = Arc::clone(self.ctx.resolver);
      let importer = self.path.clone();
      // let is_external = self.is_external.clone();
      // let on_warn = self.input_options.on_warn.clone();
      tokio::spawn(async move {
        Self::resolve_id(&resolver, &importer, &specifier).await.map(|id| (idx, id))
      })
    });

    let resolved_ids = join_all(jobs).await;

    let mut errors = vec![];

    let ret = resolved_ids
      .into_iter()
      .filter_map(|handle| match handle.unwrap() {
        Ok(item) => Some(item),
        Err(e) => {
          errors.push(e);
          None
        }
      })
      .collect();

    Ok(ret)
  }
}
