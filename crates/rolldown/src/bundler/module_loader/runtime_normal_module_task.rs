use index_vec::IndexVec;
use oxc::{ast::Visit, span::SourceType};
use rolldown_common::{ExportsKind, ModuleId, ModuleType, ResourceId, SymbolRef};
use rolldown_error::BuildError;
use rolldown_oxc::{OxcCompiler, OxcProgram};

use super::Msg;
use crate::bundler::{
  module::normal_module_builder::NormalModuleBuilder,
  runtime::RuntimeModuleBrief,
  utils::{ast_scope::AstScope, ast_symbol::AstSymbol},
  visitors::scanner::{self, ScanResult},
};
pub struct RuntimeNormalModuleTask {
  tx: tokio::sync::mpsc::UnboundedSender<Msg>,
  module_id: ModuleId,
  warnings: Vec<BuildError>,
}

pub struct RuntimeNormalModuleTaskResult {
  pub runtime: RuntimeModuleBrief,
  pub ast_symbol: AstSymbol,
  pub warnings: Vec<BuildError>,
  pub builder: NormalModuleBuilder,
}

impl RuntimeNormalModuleTask {
  pub fn new(id: ModuleId, tx: tokio::sync::mpsc::UnboundedSender<Msg>) -> Self {
    Self { module_id: id, tx, warnings: Vec::default() }
  }

  #[tracing::instrument(skip_all)]
  pub fn run(self) {
    let mut builder = NormalModuleBuilder::default();

    let source = include_str!("../runtime/index.js").to_string();

    let (ast, scope, scan_result, symbol, namespace_symbol) = self.make_ast(source);

    let runtime = RuntimeModuleBrief::new(self.module_id, &scope);

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      star_exports,
      export_default_symbol_id,
      imports,
      unique_name,
      import_records: _,
      exports_kind: _,
    } = scan_result;

    builder.id = Some(self.module_id);
    builder.ast = Some(ast);
    builder.unique_name = Some(unique_name);
    builder.path = Some(ResourceId::new(
      "TODO: Runtime module should not have FilePath as source id".to_string().into(),
    ));
    builder.named_imports = Some(named_imports);
    builder.named_exports = Some(named_exports);
    builder.stmt_infos = Some(stmt_infos);
    builder.imports = Some(imports);
    builder.star_exports = Some(star_exports);
    builder.default_export_symbol = export_default_symbol_id;
    builder.import_records = Some(IndexVec::default());
    builder.scope = Some(scope);
    builder.exports_kind = Some(ExportsKind::Esm);
    builder.namespace_symbol = Some(namespace_symbol);
    builder.pretty_path = Some("<runtime>".to_string());

    self
      .tx
      .send(Msg::RuntimeNormalModuleDone(RuntimeNormalModuleTaskResult {
        warnings: self.warnings,
        ast_symbol: symbol,
        builder,
        runtime,
      }))
      .unwrap();
  }

  fn make_ast(&self, source: String) -> (OxcProgram, AstScope, ScanResult, AstSymbol, SymbolRef) {
    let source_type = SourceType::default();
    let program = OxcCompiler::parse(source, source_type);

    let semantic = program.make_semantic(source_type);
    let (mut symbol_table, scope) = semantic.into_symbol_table_and_scope_tree();
    let ast_scope = AstScope::new(scope, std::mem::take(&mut symbol_table.references));
    let mut symbol_for_module = AstSymbol::from_symbol_table(symbol_table);
    let mut scanner = scanner::Scanner::new(
      self.module_id,
      &ast_scope,
      &mut symbol_for_module,
      "runtime".to_string(),
      ModuleType::EsmMjs,
    );
    let namespace_symbol = scanner.namespace_symbol;
    scanner.visit_program(program.program());
    let scan_result = scanner.result;

    (program, ast_scope, scan_result, symbol_for_module, namespace_symbol)
  }
}
