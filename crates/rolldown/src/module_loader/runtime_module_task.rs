use arcstr::ArcStr;
use oxc::index::IndexVec;
use oxc::span::SourceType;
use rolldown_common::{
  side_effects::DeterminedSideEffects, AstScopes, EcmaModule, ExportsKind, ModuleDefFormat,
  ModuleId, ModuleIdx, ModuleType, SymbolRef,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_error::{BuildDiagnostic, DiagnosableResult, UnhandleableResult};

use super::Msg;
use crate::{
  ast_scanner::{AstScanner, ScanResult},
  runtime::{RuntimeModuleBrief, RUNTIME_MODULE_ID},
  types::ast_symbols::AstSymbols,
  utils::tweak_ast_for_scanning::tweak_ast_for_scanning,
};
pub struct RuntimeModuleTask {
  tx: tokio::sync::mpsc::Sender<Msg>,
  module_id: ModuleIdx,
  errors: Vec<BuildDiagnostic>,
}

pub struct RuntimeModuleTaskResult {
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

impl RuntimeModuleTask {
  pub fn new(id: ModuleIdx, tx: tokio::sync::mpsc::Sender<Msg>) -> Self {
    Self { module_id: id, tx, errors: Vec::new() }
  }

  #[tracing::instrument(name = "RuntimeNormalModuleTaskResult::run", level = "debug", skip_all)]
  pub fn run(mut self) -> anyhow::Result<()> {
    let source: ArcStr = arcstr::literal!(include_str!("../runtime/runtime-without-comments.js"));

    let ecma_ast_result = self.make_ecma_ast(RUNTIME_MODULE_ID, &source)?;

    let ecma_ast_result = match ecma_ast_result {
      Ok(ecma_ast_result) => ecma_ast_result,
      Err(errs) => {
        self.errors.extend(errs);
        return Ok(());
      }
    };

    let MakeEcmaAstResult { ast, ast_scope, scan_result, ast_symbols, namespace_object_ref } =
      ecma_ast_result;

    let runtime = RuntimeModuleBrief::new(self.module_id, &ast_scope);

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      star_exports,
      default_export_ref,
      imports,
      import_records: _,
      exports_kind: _,
      warnings: _,
      has_eval,
      errors: _,
    } = scan_result;

    let module = EcmaModule {
      source,
      idx: self.module_id,
      ecma_ast_idx: None,
      repr_name: "rolldown_runtime".to_string(),
      stable_id: RUNTIME_MODULE_ID.to_string(),
      id: ModuleId::new(RUNTIME_MODULE_ID),
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
      debug_id: RUNTIME_MODULE_ID.to_string(),
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
      has_eval,
      css_view: None,
    };

    if let Err(_err) = self.tx.try_send(Msg::RuntimeNormalModuleDone(RuntimeModuleTaskResult {
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

  fn make_ecma_ast(
    &mut self,
    filename: &str,
    source: &ArcStr,
  ) -> UnhandleableResult<DiagnosableResult<MakeEcmaAstResult>> {
    let source_type = SourceType::default();

    let parse_result = EcmaCompiler::parse(filename, source, source_type);

    let mut ast = match parse_result {
      Ok(ast) => ast,
      Err(errs) => {
        return Ok(Err(errs));
      }
    };
    tweak_ast_for_scanning(&mut ast);

    let (mut symbol_table, scope) = ast.make_symbol_table_and_scope_tree();
    let ast_scope = AstScopes::new(
      scope,
      std::mem::take(&mut symbol_table.references),
      std::mem::take(&mut symbol_table.resolved_references),
    );
    let mut ast_symbols = AstSymbols::from_symbol_table(symbol_table);
    let facade_path = ModuleId::new("runtime");
    let scanner = AstScanner::new(
      self.module_id,
      &ast_scope,
      &mut ast_symbols,
      "rolldown_runtime",
      ModuleDefFormat::EsmMjs,
      source,
      &facade_path,
      &ast.trivias,
    );
    let namespace_object_ref = scanner.namespace_object_ref;
    let scan_result = scanner.scan(ast.program())?;

    Ok(Ok(MakeEcmaAstResult { ast, ast_scope, scan_result, ast_symbols, namespace_object_ref }))
  }
}
