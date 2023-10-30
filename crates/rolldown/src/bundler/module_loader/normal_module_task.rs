use std::sync::Arc;

use futures::future::join_all;
use index_vec::IndexVec;
use oxc::{
  ast::VisitMut,
  semantic::{ScopeTree, SymbolTable},
  span::SourceType,
};
use rolldown_common::{ImportRecord, ImportRecordId, ModuleId, ModuleType, ResourceId, SymbolRef};
use rolldown_oxc::{OxcCompiler, OxcProgram};
use rolldown_resolver::Resolver;

use super::Msg;
use crate::{
  bundler::{
    graph::symbols::SymbolMap,
    module::normal_module_builder::NormalModuleBuilder,
    module_loader::NormalModuleTaskResult,
    utils::resolve_id::{self, resolve_id, ResolvedRequestInfo},
    visitors::scanner::{self, ScanResult},
  },
  BuildError, BuildResult, SharedResolver,
};
pub struct NormalModuleTask {
  module_id: ModuleId,
  path: ResourceId,
  module_type: ModuleType,
  tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  errors: Vec<BuildError>,
  warnings: Vec<BuildError>,
  resolver: SharedResolver,
  is_entry: bool,
}

impl NormalModuleTask {
  pub fn new(
    id: ModuleId,
    is_entry: bool,
    resolver: SharedResolver,
    path: ResourceId,
    module_type: ModuleType,
    tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  ) -> Self {
    Self {
      module_id: id,
      is_entry,
      resolver,
      path,
      module_type,
      tx,
      errors: Vec::default(),
      warnings: Vec::default(),
    }
  }

  pub async fn run(mut self) -> anyhow::Result<()> {
    let mut builder = NormalModuleBuilder::default();
    tracing::trace!("process {:?}", self.path);
    // load
    let source = tokio::fs::read_to_string(self.path.as_ref()).await?;
    // TODO: transform

    let (ast, scope, scan_result, symbol, namespace_symbol) = self.make_ast(source);

    let res = self.resolve_dependencies(&scan_result.import_records).await?;

    let symbol_map = SymbolMap::from_symbol_table(symbol);

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      import_records,
      star_exports,
      export_default_symbol_id,
      imports,
      exports_kind,
    } = scan_result;

    builder.id = Some(self.module_id);
    builder.ast = Some(ast);
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
      .tx
      .send(Msg::NormalModuleDone(NormalModuleTaskResult {
        resolved_deps: res,
        module_id: self.module_id,
        errors: self.errors,
        warnings: self.warnings,
        symbol_map,
        builder,
      }))
      .unwrap();
    Ok(())
  }

  fn make_ast(
    &self,
    source: String,
  ) -> (OxcProgram, ScopeTree, ScanResult, SymbolTable, SymbolRef) {
    let source_type = SourceType::from_path(self.path.as_ref()).unwrap();
    let mut program = OxcCompiler::parse(source, source_type);

    let semantic = program.make_semantic(source_type);
    let (mut symbol_table, mut scope) = semantic.into_symbol_table_and_scope_tree();
    let unique_name = self.path.generate_unique_name();
    let mut scanner = scanner::Scanner::new(
      self.module_id,
      &mut scope,
      &mut symbol_table,
      &unique_name,
      self.module_type,
    );
    scanner.visit_program(program.program_mut());
    let scan_result = scanner.result;
    let namespace_symbol = scanner.namespace_symbol;
    (program, scope, scan_result, symbol_table, namespace_symbol)
  }

  #[allow(clippy::option_if_let_else)]
  pub(crate) async fn resolve_id(
    resolver: &Resolver,
    importer: &ResourceId,
    specifier: &str,
  ) -> BuildResult<ResolvedRequestInfo> {
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
      let resolver = Arc::clone(&self.resolver);
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
