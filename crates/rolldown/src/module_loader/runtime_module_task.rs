use std::sync::Arc;

use arcstr::ArcStr;
use oxc::ast_visit::VisitMut;
use oxc::span::SourceType;
use oxc_index::IndexVec;
use rolldown_common::{
  EcmaView, ExportsKind, FlatOptions, ModuleDefFormat, ModuleIdx, ModuleType, NormalModule,
  SideEffectDetail, StableModuleId, side_effects::DeterminedSideEffects,
  side_effects::HookSideEffects,
};
use rolldown_common::{
  ModuleLoaderMsg, RUNTIME_MODULE_ID, RUNTIME_MODULE_KEY, ResolvedId, RuntimeModuleBrief,
  RuntimeModuleTaskResult,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rolldown_error::BuildResult;
use rolldown_utils::concat_string;
use rolldown_utils::indexmap::FxIndexSet;
use rolldown_utils::stabilize_id::stabilize_id;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_scanner::{AstScanner, ScanResult},
  utils::tweak_ast_for_scanning::PreProcessor,
};

use super::resolve_utils::resolve_dependencies;
use super::task_context::TaskContext;

const RUNTIME_BASE_JS: &str = include_str!("../runtime/runtime-base.js");
const RUNTIME_TAIL_JS: &str = include_str!("../runtime/runtime-tail.js");
const RUNTIME_HEAD_NODE_JS: &str = include_str!("../runtime/runtime-head-node.js");
const RUNTIME_TAIL_NODE_JS: &str = include_str!("../runtime/runtime-tail-node.js");

fn get_runtime_js() -> String {
  concat_string!(RUNTIME_BASE_JS, RUNTIME_TAIL_JS)
}

fn get_runtime_js_with_node_platform() -> String {
  concat_string!(RUNTIME_HEAD_NODE_JS, RUNTIME_BASE_JS, RUNTIME_TAIL_NODE_JS)
}

pub struct RuntimeModuleTask {
  module_idx: ModuleIdx,
  ctx: Arc<TaskContext>,
  flat_options: FlatOptions,
}

impl RuntimeModuleTask {
  pub fn new(
    module_idx: ModuleIdx,
    shared_context: Arc<TaskContext>,
    flat_options: FlatOptions,
  ) -> Self {
    Self { module_idx, ctx: shared_context, flat_options }
  }

  #[tracing::instrument(name = "RuntimeNormalModuleTaskResult::run", level = "debug", skip_all)]
  pub async fn run(self) {
    if let Err(errs) = self.run_inner().await {
      self
        .ctx
        .tx
        .try_send(ModuleLoaderMsg::BuildErrors(errs.into_vec().into_boxed_slice()))
        .expect("Send should not fail");
    }
  }

  async fn run_inner(&self) -> BuildResult<()> {
    let source: String = if self.ctx.options.is_esm_format_with_node_platform() {
      get_runtime_js_with_node_platform()
    } else {
      get_runtime_js()
    };

    let original_source = source.clone();

    // Call transform hook on runtime module
    let mut sourcemap_chain = vec![];
    let mut side_effects: Option<HookSideEffects> = None;
    let mut module_type = ModuleType::Js;
    let mut code_changed_by_plugins: Option<Vec<String>> = Some(vec![]);

    let source: ArcStr = self
      .ctx
      .plugin_driver
      .transform(
        RUNTIME_MODULE_KEY,
        self.module_idx,
        source,
        &mut sourcemap_chain,
        &mut side_effects,
        &mut module_type,
        None,
        &mut code_changed_by_plugins,
      )
      .await?
      .into();

    // Track which plugins modified the runtime module, so we can provide
    // helpful error messages if symbol validation fails.
    let mut modified_by_plugins: Vec<String> = vec![];
    if let Some(plugin_names) = code_changed_by_plugins {
      if !plugin_names.is_empty() && source.as_str() != original_source {
        modified_by_plugins = plugin_names;
      }
    }

    let (ast, scan_result) = self.make_ecma_ast(RUNTIME_MODULE_KEY, &source)?;

    let ScanResult {
      named_imports,
      named_exports,
      stmt_infos,
      default_export_ref,
      namespace_object_ref,
      imports,
      import_records: raw_import_records,
      ast_usage,
      symbol_ref_db,
      new_url_references,
      dummy_record_set,
      ecma_view_meta,
      ..
    } = scan_result;

    let determined_side_effects = match side_effects {
      Some(HookSideEffects::False) => DeterminedSideEffects::UserDefined(false),
      Some(HookSideEffects::NoTreeshake) => DeterminedSideEffects::NoTreeshake,
      Some(HookSideEffects::True) | None => {
        let has_side_effects = stmt_infos
          .iter()
          .any(|stmt_info| stmt_info.side_effect.contains(SideEffectDetail::Unknown));
        DeterminedSideEffects::Analyzed(has_side_effects)
      }
    };

    let mut resolved_id = ResolvedId::make_dummy();
    resolved_id.id = RUNTIME_MODULE_ID;
    let module_type = ModuleType::Js;

    let resolved_deps = resolve_dependencies(
      &resolved_id,
      &self.ctx.options,
      &self.ctx.resolver,
      &self.ctx.plugin_driver,
      &raw_import_records,
      source.clone(),
      &mut vec![],
      &module_type,
    )
    .await?;

    let module = NormalModule {
      idx: self.module_idx,
      repr_name: "rolldown_runtime".to_string(),
      stable_id: StableModuleId::new(&RUNTIME_MODULE_ID, &self.ctx.options.cwd),
      id: RUNTIME_MODULE_ID,

      debug_id: stabilize_id(RUNTIME_MODULE_KEY, &self.ctx.options.cwd),
      exec_order: u32::MAX,
      is_user_defined_entry: false,
      module_type,

      ecma_view: EcmaView {
        source,

        import_records: IndexVec::default(),
        sourcemap_chain,
        // The internal runtime module `importers/imported` should be skip.
        importers: FxIndexSet::default(),
        importers_idx: FxIndexSet::default(),
        dynamic_importers: FxIndexSet::default(),
        imported_ids: FxIndexSet::default(),
        dynamically_imported_ids: FxIndexSet::default(),
        side_effects: determined_side_effects,
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
        meta: ecma_view_meta,
        mutations: vec![],
        new_url_references,
        this_expr_replace_map: FxHashMap::default(),
        hmr_info: scan_result.hmr_info,
        hmr_hot_ref: None,
        directive_range: vec![],
        dummy_record_set,
        constant_export_map: FxHashMap::default(),
        depended_runtime_helper: Box::default(),
        import_attribute_map: FxHashMap::default(),
        json_module_none_self_reference_included_symbol: None,
      },
      css_view: None,
      asset_view: None,
      // TODO(hyf0/hmr): We might need to find a better way to handle this.
      originative_resolved_id: resolved_id,
    };

    let mut runtime = RuntimeModuleBrief::new(self.module_idx, &symbol_ref_db.ast_scopes);
    runtime.set_modified_by_plugins(modified_by_plugins);
    let result = ModuleLoaderMsg::RuntimeNormalModuleDone(Box::new(RuntimeModuleTaskResult {
      ast,
      module,
      runtime,
      resolved_deps,
      raw_import_records,
      local_symbol_ref_db: symbol_ref_db,
    }));

    // If the main thread is dead, nothing we can do to handle these send failures.
    let _ = self.ctx.tx.try_send(result);

    Ok(())
  }

  fn make_ecma_ast(&self, filename: &str, source: &ArcStr) -> BuildResult<(EcmaAst, ScanResult)> {
    let source_type = SourceType::default();

    let mut ast = EcmaCompiler::parse(filename, source.clone(), source_type)?;

    ast.program.with_mut(|fields| {
      let mut pre_processor = PreProcessor::new(fields.allocator, false);
      pre_processor.visit_program(fields.program);
    });

    let scoping = ast.make_scoping();
    let facade_path = RUNTIME_MODULE_ID;
    // Always respect annotations in the runtime module, regardless of user config.
    // The runtime is trusted internal code.
    let runtime_flat_options = self.flat_options - FlatOptions::IgnoreAnnotations;
    let scanner = AstScanner::new(
      self.module_idx,
      scoping,
      "rolldown_runtime",
      ModuleDefFormat::EsmMjs,
      source,
      &facade_path,
      ast.comments(),
      &self.ctx.options,
      ast.allocator(),
      runtime_flat_options,
    );
    let scan_result = scanner.scan(ast.program())?;

    Ok((ast, scan_result))
  }
}
