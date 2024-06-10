use std::sync::Arc;

use oxc::span::SourceType;
use oxc_index::IndexVec;
use rolldown_common::{
  side_effects::DeterminedSideEffects, AstScopes, ExportsKind, ModuleDefFormat, NormalModule,
  NormalModuleId, ResourceId, SymbolRef,
};
use rolldown_error::BuildError;
use rolldown_oxc_utils::{OxcAst, OxcCompiler};

use super::Msg;
use crate::{
  ast_scanner::{AstScanner, ScanResult},
  runtime::{RuntimeModuleBrief, ROLLDOWN_RUNTIME_RESOURCE_ID},
  types::ast_symbols::AstSymbols,
  utils::tweak_ast_for_scanning::tweak_ast_for_scanning,
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

  #[tracing::instrument(name = "RuntimeNormalModuleTaskResult::run", level = "debug", skip_all)]
  pub fn run(self) -> anyhow::Result<()> {
    let source: Arc<str> =
      include_str!("../runtime/runtime-without-comments.js").to_string().into();

    let (ast, scope, scan_result, symbol, namespace_object_ref) = self.make_ast(&source)?;

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
      stable_resource_id: ROLLDOWN_RUNTIME_RESOURCE_ID.to_string(),
      resource_id: ResourceId::new(ROLLDOWN_RUNTIME_RESOURCE_ID),
      named_imports,
      named_exports,
      stmt_infos,
      imports,
      star_exports,
      default_export_ref,
      scope,
      exports_kind: ExportsKind::Esm,
      namespace_object_ref,
      def_format: ModuleDefFormat::EsmMjs,
      debug_resource_id: ROLLDOWN_RUNTIME_RESOURCE_ID.to_string(),
      exec_order: u32::MAX,
      is_user_defined_entry: false,
      import_records: IndexVec::default(),
      is_included: false,
      sourcemap_chain: vec![],
      // The internal runtime module `importers/imported` should be skip.
      importers: vec![],
      dynamic_importers: vec![],
      imported_ids: vec![],
      dynamically_imported_ids: vec![],
      side_effects: DeterminedSideEffects::Analyzed(false),
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

    Ok(())
  }

  fn make_ast(
    &self,
    source: &Arc<str>,
  ) -> anyhow::Result<(OxcAst, AstScopes, ScanResult, AstSymbols, SymbolRef)> {
    let source_type = SourceType::default();
    let mut ast = OxcCompiler::parse(Arc::clone(source), source_type)?;
    tweak_ast_for_scanning(&mut ast);

    let (mut symbol_table, scope) = ast.make_symbol_table_and_scope_tree();
    let ast_scope = AstScopes::new(
      scope,
      std::mem::take(&mut symbol_table.references),
      std::mem::take(&mut symbol_table.resolved_references),
    );
    let mut symbol_for_module = AstSymbols::from_symbol_table(symbol_table);
    let facade_path = ResourceId::new("runtime");
    let scanner = AstScanner::new(
      self.module_id,
      &ast_scope,
      &mut symbol_for_module,
      "runtime".to_string(),
      ModuleDefFormat::EsmMjs,
      source,
      &facade_path,
      &ast.trivias,
    );
    let namespace_symbol = scanner.namespace_object_ref;
    let scan_result = scanner.scan(ast.program());

    Ok((ast, ast_scope, scan_result, symbol_for_module, namespace_symbol))
  }
}
