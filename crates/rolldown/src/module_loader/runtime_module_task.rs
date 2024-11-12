use arcstr::ArcStr;
use oxc::span::SourceType;
use oxc::{ast::VisitMut, index::IndexVec};
use rolldown_common::{
  side_effects::DeterminedSideEffects, AstScopes, EcmaView, EcmaViewMeta, ExportsKind,
  ModuleDefFormat, ModuleId, ModuleIdx, ModuleType, NormalModule, SymbolRef,
};
use rolldown_common::{
  ModuleLoaderMsg, RuntimeModuleBrief, RuntimeModuleTaskResult, RUNTIME_MODULE_ID,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rustc_hash::FxHashSet;

use crate::{
  ast_scanner::{AstScanner, ScanResult},
  utils::tweak_ast_for_scanning::PreProcessor,
};
pub struct RuntimeModuleTask {
  tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
  module_id: ModuleIdx,
  errors: Vec<BuildDiagnostic>,
}

pub struct MakeEcmaAstResult {
  ast: EcmaAst,
  ast_scope: AstScopes,
  scan_result: ScanResult,
  namespace_object_ref: SymbolRef,
}

impl RuntimeModuleTask {
  pub fn new(id: ModuleIdx, tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>) -> Self {
    Self { module_id: id, tx, errors: Vec::new() }
  }

  #[tracing::instrument(name = "RuntimeNormalModuleTaskResult::run", level = "debug", skip_all)]
  pub fn run(mut self) -> anyhow::Result<()> {
    let source: ArcStr = arcstr::literal!(include_str!("../runtime/runtime-without-comments.js"));

    let ecma_ast_result = self.make_ecma_ast(RUNTIME_MODULE_ID, &source);

    let ecma_ast_result = match ecma_ast_result {
      Ok(ecma_ast_result) => ecma_ast_result,
      Err(errs) => {
        self.errors.extend(errs.into_vec());
        return Ok(());
      }
    };

    let MakeEcmaAstResult { ast, ast_scope, scan_result, namespace_object_ref } = ecma_ast_result;

    let runtime = RuntimeModuleBrief::new(self.module_id, &ast_scope);

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
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
      hashbang_range: _,
      has_star_exports,
      new_url_references,
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
        sourcemap_chain: vec![],
        // The internal runtime module `importers/imported` should be skip.
        importers: vec![],
        dynamic_importers: vec![],
        imported_ids: vec![],
        dynamically_imported_ids: vec![],
        side_effects: DeterminedSideEffects::Analyzed(false),
        named_imports,
        named_exports,
        stmt_infos,
        imports,
        default_export_ref,
        scope: ast_scope,
        exports_kind: ExportsKind::Esm,
        namespace_object_ref,
        def_format: ModuleDefFormat::EsmMjs,
        ast_usage,
        self_referenced_class_decl_symbol_ids: FxHashSet::default(),
        hashbang_range: None,
        meta: {
          let mut meta = EcmaViewMeta::default();
          meta.set_included(false);
          meta.set_eval(has_eval);
          meta.set_has_lazy_export(false);
          meta.set_has_star_exports(has_star_exports);
          meta
        },
        mutations: vec![],
        new_url_references,
      },
      css_view: None,
      asset_view: None,
    };

    if let Err(_err) =
      self.tx.try_send(ModuleLoaderMsg::RuntimeNormalModuleDone(RuntimeModuleTaskResult {
        // warnings: self.warnings,
        local_symbol_ref_db: symbol_ref_db,
        module,
        runtime,
        ast,
      }))
    {
      // hyf0: If main thread is dead, we should handle errors of main thread. So we just ignore the error here.
    };

    Ok(())
  }

  fn make_ecma_ast(&mut self, filename: &str, source: &ArcStr) -> BuildResult<MakeEcmaAstResult> {
    let source_type = SourceType::default();

    let mut ast = EcmaCompiler::parse(filename, source, source_type)?;

    ast.program.with_mut(|fields| {
      let mut pre_processor = PreProcessor::new(fields.allocator);
      pre_processor.visit_program(fields.program);
      ast.contains_use_strict = pre_processor.contains_use_strict;
    });

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
      ast.comments(),
      None,
    );
    let namespace_object_ref = scanner.namespace_object_ref;
    let scan_result = scanner.scan(ast.program())?;

    Ok(MakeEcmaAstResult { ast, ast_scope, scan_result, namespace_object_ref })
  }
}
