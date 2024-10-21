use arcstr::ArcStr;
use oxc::index::IndexVec;
use oxc::span::SourceType;
use rolldown_common::{
  side_effects::DeterminedSideEffects, AstScopes, EcmaView, ExportsKind, ModuleDefFormat, ModuleId,
  ModuleIdx, ModuleType, NormalModule, SymbolRef, SymbolRefDbForModule,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_error::{BuildDiagnostic, DiagnosableResult, UnhandleableResult};
use rustc_hash::FxHashSet;

use super::Msg;
use crate::{
  ast_scanner::{AstScanner, ScanResult},
  runtime::{RuntimeModuleBrief, RUNTIME_MODULE_ID},
  utils::tweak_ast_for_scanning::tweak_ast_for_scanning,
};
pub struct RuntimeModuleTask {
  tx: tokio::sync::mpsc::Sender<Msg>,
  module_id: ModuleIdx,
  errors: Vec<BuildDiagnostic>,
}

pub struct RuntimeModuleTaskResult {
  pub runtime: RuntimeModuleBrief,
  pub local_symbol_ref_db: SymbolRefDbForModule,
  pub ast: EcmaAst,
  // pub warnings: Vec<BuildError>,
  pub module: NormalModule,
}

pub struct MakeEcmaAstResult {
  ast: EcmaAst,
  ast_scope: AstScopes,
  scan_result: ScanResult,
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

    let MakeEcmaAstResult { ast, ast_scope, scan_result, namespace_object_ref } = ecma_ast_result;

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
      ast_usage,
      symbol_ref_db,
      self_referenced_class_decl_symbol_ids: _,
    } = scan_result;

    let module = NormalModule {
      idx: self.module_id,
      repr_name: "rolldown_runtime".to_string(),
      stable_id: RUNTIME_MODULE_ID.to_string(),
      id: ModuleId::new(RUNTIME_MODULE_ID),

      debug_id: RUNTIME_MODULE_ID.to_string(),
      exec_order: u32::MAX,
      is_user_defined_entry: false,
      module_type: ModuleType::Js,

      ecma_view: EcmaView {
        ecma_ast_idx: None,
        source,

        import_records: IndexVec::default(),
        is_included: false,
        sourcemap_chain: vec![],
        // The internal runtime module `importers/imported` should be skip.
        importers: vec![],
        dynamic_importers: vec![],
        imported_ids: vec![],
        dynamically_imported_ids: vec![],
        side_effects: DeterminedSideEffects::Analyzed(false),
        has_eval,
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
        ast_usage,
        self_referenced_class_decl_symbol_ids: FxHashSet::default(),
      },
      css_view: None,
    };

    if let Err(_err) = self.tx.try_send(Msg::RuntimeNormalModuleDone(RuntimeModuleTaskResult {
      // warnings: self.warnings,
      local_symbol_ref_db: symbol_ref_db,
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
    let facade_path = ModuleId::new("runtime");
    let scanner = AstScanner::new(
      self.module_id,
      &ast_scope,
      symbol_table,
      "rolldown_runtime",
      ModuleDefFormat::EsmMjs,
      source,
      &facade_path,
      &ast.trivias,
    );
    let namespace_object_ref = scanner.namespace_object_ref;
    let scan_result = scanner.scan(ast.program())?;

    Ok(Ok(MakeEcmaAstResult { ast, ast_scope, scan_result, namespace_object_ref }))
  }
}
