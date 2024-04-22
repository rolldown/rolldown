use std::sync::Arc;

use index_vec::IndexVec;
use oxc::span::SourceType;
use rolldown_common::{
  AstScope, ExportsKind, FilePath, ModuleType, NormalModule, NormalModuleId, ResourceId, SymbolRef,
};
use rolldown_error::BuildError;
use rolldown_oxc_utils::{OxcAst, OxcCompiler};

use super::Msg;
use crate::{
  ast_scanner::{AstScanner, ScanResult},
  runtime::RuntimeModuleBrief,
  types::ast_symbols::AstSymbols,
};
pub struct RuntimeNormalModuleTask {
  tx: tokio::sync::mpsc::Sender<Msg>,
  module_id: NormalModuleId,
  warnings: Vec<BuildError>,
}

pub struct RuntimeNormalModuleTaskResult {
  pub runtime: RuntimeModuleBrief,
  pub ast_symbol: AstSymbols,
  pub ast: OxcAst,
  pub warnings: Vec<BuildError>,
  pub module: NormalModule,
}

impl RuntimeNormalModuleTask {
  pub fn new(id: NormalModuleId, tx: tokio::sync::mpsc::Sender<Msg>) -> Self {
    Self { module_id: id, tx, warnings: Vec::default() }
  }

  #[tracing::instrument(skip_all)]
  pub fn run(self) {
    tracing::trace!("process <runtime>");

    let source: Arc<str> =
      include_str!("../runtime/runtime-without-comments.js").to_string().into();

    let (ast, scope, scan_result, symbol, namespace_symbol) = self.make_ast(&source);

    let runtime = RuntimeModuleBrief::new(self.module_id, &scope);

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      star_exports,
      default_export_ref,
      imports,
      repr_name,
      import_records: _,
      exports_kind: _,
      warnings: _,
    } = scan_result;

    let module = NormalModule {
      source,
      id: self.module_id,
      repr_name,
      // TODO: Runtime module should not have FilePath as source id
      resource_id: ResourceId::new("runtime".to_string().into()),
      named_imports,
      named_exports,
      stmt_infos,
      imports,
      star_exports,
      default_export_ref,
      scope,
      exports_kind: ExportsKind::Esm,
      namespace_symbol,
      module_type: ModuleType::EsmMjs,
      pretty_path: "<runtime>".to_string(),
      is_virtual: true,
      exec_order: u32::MAX,
      is_user_defined_entry: false,
      import_records: IndexVec::default(),
      is_included: false,
      sourcemap_chain: vec![],
    };

    if let Err(_err) =
      self.tx.try_send(Msg::RuntimeNormalModuleDone(RuntimeNormalModuleTaskResult {
        warnings: self.warnings,
        ast_symbol: symbol,
        module,
        runtime,
        ast,
      }))
    {
      // hyf0: If main thread is dead, we should handle errors of main thread. So we just ignore the error here.
    };
  }

  fn make_ast(&self, source: &Arc<str>) -> (OxcAst, AstScope, ScanResult, AstSymbols, SymbolRef) {
    let source_type = SourceType::default();
    let mut ast = OxcCompiler::parse(Arc::clone(source), source_type);

    let (mut symbol_table, scope) = ast.make_symbol_table_and_scope_tree();
    let ast_scope = AstScope::new(
      scope,
      std::mem::take(&mut symbol_table.references),
      std::mem::take(&mut symbol_table.resolved_references),
    );
    let mut symbol_for_module = AstSymbols::from_symbol_table(symbol_table);
    let facade_path = FilePath::new("runtime");
    let scanner = AstScanner::new(
      self.module_id,
      &ast_scope,
      &mut symbol_for_module,
      "runtime".to_string(),
      ModuleType::EsmMjs,
      source,
      &facade_path,
    );
    let namespace_symbol = scanner.namespace_ref;
    ast.hoist_import_export_from_stmts();
    let scan_result = scanner.scan(ast.program());

    (ast, ast_scope, scan_result, symbol_for_module, namespace_symbol)
  }
}
