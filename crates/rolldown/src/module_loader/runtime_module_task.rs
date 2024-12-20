use arcstr::ArcStr;
use oxc::ast::VisitMut;
use oxc::span::SourceType;
use oxc_index::IndexVec;
use rolldown_common::{
  side_effects::DeterminedSideEffects, AstScopes, EcmaView, EcmaViewMeta, ExportsKind,
  ModuleDefFormat, ModuleId, ModuleIdx, ModuleType, NormalModule, SymbolRef,
};
use rolldown_common::{
  ModuleLoaderMsg, ResolvedId, RuntimeModuleBrief, RuntimeModuleTaskResult,
  SharedNormalizedBundlerOptions, RUNTIME_MODULE_ID,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_error::BuildResult;
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_scanner::{AstScanner, ScanResult},
  utils::tweak_ast_for_scanning::PreProcessor,
};

pub struct RuntimeModuleTask {
  tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
  module_idx: ModuleIdx,
  options: SharedNormalizedBundlerOptions,
}

pub struct MakeEcmaAstResult {
  ast: EcmaAst,
  ast_scope: AstScopes,
  scan_result: ScanResult,
  namespace_object_ref: SymbolRef,
}

impl RuntimeModuleTask {
  pub fn new(
    module_idx: ModuleIdx,
    tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
    options: SharedNormalizedBundlerOptions,
  ) -> Self {
    Self { module_idx, tx, options }
  }

  #[tracing::instrument(name = "RuntimeNormalModuleTaskResult::run", level = "debug", skip_all)]
  pub fn run(mut self) {
    if let Err(errs) = self.run_inner() {
      self
        .tx
        .try_send(ModuleLoaderMsg::BuildErrors(errs.into_vec()))
        .expect("Send should not fail");
    }
  }

  fn run_inner(&mut self) -> BuildResult<()> {
    let source = if self.options.is_esm_format_with_node_platform() {
      arcstr::literal!(concat!(
        include_str!("../runtime/runtime-head-node.js"),
        include_str!("../runtime/runtime-base.js"),
        include_str!("../runtime/runtime-tail-node.js"),
      ))
    } else {
      arcstr::literal!(concat!(
        include_str!("../runtime/runtime-base.js"),
        include_str!("../runtime/runtime-tail.js"),
      ))
    };

    let ecma_ast_result = self.make_ecma_ast(RUNTIME_MODULE_ID, &source)?;

    let MakeEcmaAstResult { ast, ast_scope, scan_result, namespace_object_ref } = ecma_ast_result;

    let runtime = RuntimeModuleBrief::new(self.module_idx, &ast_scope);

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      default_export_ref,
      imports,
      import_records: raw_import_records,
      exports_kind: _,
      warnings: _,
      has_eval,
      errors: _,
      ast_usage,
      symbol_ref_db,
      self_referenced_class_decl_symbol_ids: _,
      hashbang_range: _,
      has_star_exports,
      dynamic_import_rec_exports_usage: _,
      new_url_references,
      this_expr_replace_map: _,
    } = scan_result;

    let module = NormalModule {
      idx: self.module_idx,
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
        importers: FxIndexSet::default(),
        dynamic_importers: FxIndexSet::default(),
        imported_ids: FxIndexSet::default(),
        dynamically_imported_ids: FxIndexSet::default(),
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
        this_expr_replace_map: FxHashMap::default(),
      },
      css_view: None,
      asset_view: None,
    };

    let resolved_deps = raw_import_records
      .iter()
      .map(|rec| {
        // We assume the runtime module only has external dependencies.
        ResolvedId::new_external_without_side_effects(rec.module_request.to_string().into())
      })
      .collect();

    if let Err(_err) =
      self.tx.try_send(ModuleLoaderMsg::RuntimeNormalModuleDone(RuntimeModuleTaskResult {
        ast,
        module,
        runtime,
        resolved_deps,
        raw_import_records,
        local_symbol_ref_db: symbol_ref_db,
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
      let mut pre_processor = PreProcessor::new(fields.allocator, false);
      pre_processor.visit_program(fields.program);
      ast.contains_use_strict = pre_processor.contains_use_strict;
    });

    let (symbol_table, scope) = ast.make_symbol_table_and_scope_tree();
    let ast_scope = AstScopes::new(scope);
    let facade_path = ModuleId::new("runtime");
    let scanner = AstScanner::new(
      self.module_idx,
      &ast_scope,
      symbol_table,
      "rolldown_runtime",
      ModuleDefFormat::EsmMjs,
      source,
      &facade_path,
      ast.comments(),
      &self.options,
    );
    let namespace_object_ref = scanner.namespace_object_ref;
    let scan_result = scanner.scan(ast.program())?;

    Ok(MakeEcmaAstResult { ast, ast_scope, scan_result, namespace_object_ref })
  }
}
