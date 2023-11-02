use oxc::{
  ast::VisitMut,
  semantic::{ScopeTree, SymbolTable},
  span::SourceType,
};
use rolldown_common::{ModuleId, ModuleType, ResourceId, SymbolRef};
use rolldown_error::BuildError;
use rolldown_oxc::{OxcCompiler, OxcProgram};

use super::Msg;
use crate::{
  bundler::{
    graph::symbols::SymbolMap,
    module::normal_module_builder::NormalModuleBuilder,
    module_loader::NormalModuleTaskResult,
    runtime::RUNTIME_PATH,
    visitors::scanner::{self, ScanResult},
  },
  SharedResolver,
};
pub struct RuntimeNormalModuleTask {
  module_id: ModuleId,
  tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  module_type: ModuleType,
  errors: Vec<BuildError>,
  warnings: Vec<BuildError>,
  resolver: SharedResolver,
}

impl RuntimeNormalModuleTask {
  pub fn new(
    id: ModuleId,
    resolver: SharedResolver,
    tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  ) -> Self {
    Self {
      module_id: id,
      module_type: ModuleType::EsmMjs,
      resolver,
      tx,
      errors: Vec::default(),
      warnings: Vec::default(),
    }
  }

  pub fn run(self) {
    let mut builder = NormalModuleBuilder::default();

    let source = include_str!("../runtime/index.js").to_string();

    let (ast, scope, scan_result, symbol, namespace_symbol) = self.make_ast(source);

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
      unique_name,
    } = scan_result;

    builder.id = Some(self.module_id);
    builder.ast = Some(ast);
    builder.unique_name = Some(unique_name);
    builder.path = Some(ResourceId::new(RUNTIME_PATH.to_string().into(), self.resolver.cwd()));
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

    self
      .tx
      .send(Msg::RuntimeNormalModuleDone(NormalModuleTaskResult {
        resolved_deps: Vec::default(),
        module_id: self.module_id,
        errors: self.errors,
        warnings: self.warnings,
        symbol_map,
        builder,
      }))
      .unwrap();
  }

  fn make_ast(
    &self,
    source: String,
  ) -> (OxcProgram, ScopeTree, ScanResult, SymbolTable, SymbolRef) {
    let source_type = SourceType::default();
    let mut program = OxcCompiler::parse(source, source_type);

    let semantic = program.make_semantic(source_type);
    let (mut symbol_table, mut scope) = semantic.into_symbol_table_and_scope_tree();
    let mut scanner = scanner::Scanner::new(
      self.module_id,
      &mut scope,
      &mut symbol_table,
      RUNTIME_PATH.to_string(),
      self.module_type,
    );
    let namespace_symbol = scanner.namespace_symbol;
    scanner.visit_program(program.program_mut());
    let scan_result = scanner.result;

    (program, scope, scan_result, symbol_table, namespace_symbol)
  }
}
