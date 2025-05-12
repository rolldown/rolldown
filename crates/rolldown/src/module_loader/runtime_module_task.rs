use arcstr::ArcStr;
use oxc::ast_visit::VisitMut;
use oxc::span::SourceType;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaView, EcmaViewMeta, ExportsKind, ModuleDefFormat, ModuleIdx, ModuleType, NormalModule,
  side_effects::DeterminedSideEffects,
};
use rolldown_common::{
  ModuleLoaderMsg, Platform, RUNTIME_MODULE_ID, RUNTIME_MODULE_KEY, ResolvedId, RuntimeModuleBrief,
  RuntimeModuleTaskResult, SharedNormalizedBundlerOptions,
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

impl RuntimeModuleTask {
  pub fn new(
    module_idx: ModuleIdx,
    tx: tokio::sync::mpsc::Sender<ModuleLoaderMsg>,
    options: SharedNormalizedBundlerOptions,
  ) -> Self {
    Self { module_idx, tx, options }
  }

  #[tracing::instrument(name = "RuntimeNormalModuleTaskResult::run", level = "debug", skip_all)]
  pub fn run(self) {
    if let Err(errs) = self.run_inner() {
      self
        .tx
        .try_send(ModuleLoaderMsg::BuildErrors(errs.into_vec().into_boxed_slice()))
        .expect("Send should not fail");
    }
  }

  #[expect(clippy::too_many_lines)]
  fn run_inner(&self) -> BuildResult<()> {
    let source = if let Some(hmr_options) = &self.options.experimental.hmr {
      let mut runtime_source = String::new();
      match self.options.platform {
        Platform::Node => {
          runtime_source.push_str("import { WebSocket } from 'ws';\n");
        }
        Platform::Browser | Platform::Neutral => {
          // Browser platform should use the native WebSocket and neutral platform doesn't have any assumptions.
        }
      }
      runtime_source.push_str(&arcstr::literal!(concat!(
        include_str!("../runtime/runtime-base.js"),
        include_str!("../runtime/runtime-tail.js"),
      )));
      if let Some(implement) = hmr_options.implement.as_deref() {
        runtime_source.push_str(implement);
      } else {
        let content = include_str!("../runtime/runtime-extra-dev.js");
        let host = hmr_options.host.as_deref().unwrap_or("localhost");
        let port = hmr_options.port.unwrap_or(3000);
        let addr = format!("{host}:{port}");
        runtime_source.push_str(&content.replace("$ADDR", &addr));
      }
      ArcStr::from(runtime_source)
    } else if self.options.is_esm_format_with_node_platform() {
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

    let (ast, scan_result) = self.make_ecma_ast(RUNTIME_MODULE_KEY, &source)?;

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      default_export_ref,
      namespace_object_ref,
      imports,
      import_records: raw_import_records,
      has_eval,
      ast_usage,
      symbol_ref_db,
      has_star_exports,
      new_url_references,
      ..
    } = scan_result;

    let module = NormalModule {
      idx: self.module_idx,
      repr_name: "rolldown_runtime".to_string(),
      stable_id: RUNTIME_MODULE_KEY.to_string(),
      id: RUNTIME_MODULE_ID,

      debug_id: RUNTIME_MODULE_KEY.to_string(),
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
        importers_idx: FxIndexSet::default(),
        dynamic_importers: FxIndexSet::default(),
        imported_ids: FxIndexSet::default(),
        dynamically_imported_ids: FxIndexSet::default(),
        side_effects: DeterminedSideEffects::Analyzed(false),
        named_imports,
        named_exports,
        stmt_infos,
        imports,
        default_export_ref,
        exports_kind: ExportsKind::Esm,
        namespace_object_ref,
        def_format: ModuleDefFormat::EsmMjs,
        ast_usage,
        self_referenced_class_decl_symbol_ids: FxHashSet::default(),
        hashbang_range: None,
        meta: {
          let mut meta = EcmaViewMeta::default();
          meta.set(self::EcmaViewMeta::EVAL, has_eval);
          meta.set(self::EcmaViewMeta::HAS_STAR_EXPORT, has_star_exports);
          meta
        },
        mutations: vec![],
        new_url_references,
        this_expr_replace_map: FxHashMap::default(),
        hmr_info: scan_result.hmr_info,
        hmr_hot_ref: None,
      },
      css_view: None,
      asset_view: None,
      // TODO(hyf0/hmr): We might need to find a better way to handle this.
      originative_resolved_id: ResolvedId::make_dummy(),
    };

    let resolved_deps = raw_import_records
      .iter()
      .map(|rec| {
        // We assume the runtime module only has external dependencies.
        ResolvedId::new_external_without_side_effects(rec.module_request.as_str().into())
      })
      .collect();

    let runtime = RuntimeModuleBrief::new(self.module_idx, &symbol_ref_db.ast_scopes);
    let result = ModuleLoaderMsg::RuntimeNormalModuleDone(Box::new(RuntimeModuleTaskResult {
      ast,
      module,
      runtime,
      resolved_deps,
      raw_import_records,
      local_symbol_ref_db: symbol_ref_db,
    }));

    // If the main thread is dead, nothing we can do to handle these send failures.
    let _ = self.tx.try_send(result);

    Ok(())
  }

  fn make_ecma_ast(&self, filename: &str, source: &ArcStr) -> BuildResult<(EcmaAst, ScanResult)> {
    let source_type = SourceType::default();

    let mut ast = EcmaCompiler::parse(filename, source, source_type)?;

    ast.program.with_mut(|fields| {
      let mut pre_processor = PreProcessor::new(fields.allocator, false);
      pre_processor.visit_program(fields.program);
      ast.contains_use_strict = pre_processor.contains_use_strict;
    });

    let scoping = ast.make_scoping();
    let facade_path = RUNTIME_MODULE_ID;
    let scanner = AstScanner::new(
      self.module_idx,
      scoping,
      "rolldown_runtime",
      ModuleDefFormat::EsmMjs,
      source,
      &facade_path,
      ast.comments(),
      &self.options,
    );
    let scan_result = scanner.scan(ast.program())?;

    Ok((ast, scan_result))
  }
}
