use arcstr::ArcStr;
use oxc::index::IndexVec;
use oxc::span::SourceType;
use rolldown_common::{
  side_effects::DeterminedSideEffects, AstScopes, EcmaModule, ExportsKind, ModuleDefFormat,
  ModuleIdx, ModuleType, ResourceId, SymbolRef,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};

use super::Msg;
use crate::{
  ast_scanner::{AstScanner, ScanResult},
  runtime::{RuntimeModuleBrief, ROLLDOWN_RUNTIME_RESOURCE_ID},
  types::ast_symbols::AstSymbols,
  utils::tweak_ast_for_scanning::tweak_ast_for_scanning,
};
pub struct RuntimeEcmaModuleTask {
  tx: tokio::sync::mpsc::Sender<Msg>,
  module_id: ModuleIdx,
  // warnings: Vec<BuildError>,
}

pub struct RuntimeEcmaModuleTaskResult {
  pub runtime: RuntimeModuleBrief,
  pub ast_symbols: AstSymbols,
  pub ast: EcmaAst,
  // pub warnings: Vec<BuildError>,
  pub module: EcmaModule,
}

pub struct MakeEcmaAstResult {
  ast: EcmaAst,
  ast_scope: AstScopes,
  scan_result: ScanResult,
  ast_symbols: AstSymbols,
  namespace_object_ref: SymbolRef,
}

impl RuntimeEcmaModuleTask {
  pub fn new(id: ModuleIdx, tx: tokio::sync::mpsc::Sender<Msg>) -> Self {
    Self { module_id: id, tx }
  }

  #[tracing::instrument(name = "RuntimeNormalModuleTaskResult::run", level = "debug", skip_all)]
  pub fn run(self) -> anyhow::Result<()> {
    let source: ArcStr = arcstr::literal!(include_str!("../runtime/runtime-without-comments.js"));

    let MakeEcmaAstResult { ast, ast_scope, scan_result, ast_symbols, namespace_object_ref } =
      self.make_ecma_ast(&source)?;

    let runtime = RuntimeModuleBrief::new(self.module_id, &ast_scope);

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

    let module = EcmaModule {
      source,
      idx: self.module_id,
      repr_name,
      stable_resource_id: ROLLDOWN_RUNTIME_RESOURCE_ID.to_string(),
      resource_id: ResourceId::new(ROLLDOWN_RUNTIME_RESOURCE_ID),
      named_imports,
      named_exports,
      stmt_infos,
      imports,
      star_exports,
      default_export_ref,
      scope: ast_scope,
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
      module_type: ModuleType::Js,
    };

    if let Err(_err) = self.tx.try_send(Msg::RuntimeNormalModuleDone(RuntimeEcmaModuleTaskResult {
      // warnings: self.warnings,
      ast_symbols,
      module,
      runtime,
      ast,
    })) {
      // hyf0: If main thread is dead, we should handle errors of main thread. So we just ignore the error here.
    };

    Ok(())
  }

  fn make_ecma_ast(&self, source: &ArcStr) -> anyhow::Result<MakeEcmaAstResult> {
    let source_type = SourceType::default();
    let mut ast = EcmaCompiler::parse(source, source_type)?;
    tweak_ast_for_scanning(&mut ast);

    let (mut symbol_table, scope) = ast.make_symbol_table_and_scope_tree();
    let ast_scope = AstScopes::new(
      scope,
      std::mem::take(&mut symbol_table.references),
      std::mem::take(&mut symbol_table.resolved_references),
    );
    let mut ast_symbols = AstSymbols::from_symbol_table(symbol_table);
    let facade_path = ResourceId::new("runtime");
    let scanner = AstScanner::new(
      self.module_id,
      &ast_scope,
      &mut ast_symbols,
      "runtime".to_string(),
      ModuleDefFormat::EsmMjs,
      source,
      &facade_path,
      &ast.trivias,
    );
    let namespace_object_ref = scanner.namespace_object_ref;
    let scan_result = scanner.scan(ast.program());

    Ok(MakeEcmaAstResult { ast, ast_scope, scan_result, ast_symbols, namespace_object_ref })
  }
}
