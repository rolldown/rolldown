use std::{
  ops::{Deref, DerefMut},
  sync::Arc,
};

use arcstr::ArcStr;
use oxc::ast_visit::VisitMut;
use rolldown_common::{
  EcmaModuleAstUsage, HmrBoundary, HmrBoundaryOutput, HmrOutput, Module, ModuleIdx, ModuleTable,
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler, PrintOptions};
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_error::BuildResult;
use rolldown_fs::OsFileSystem;
use rolldown_plugin::SharedPluginDriver;
use rolldown_sourcemap::{SourceJoiner, SourceMapSource};
use rolldown_utils::indexmap::FxIndexSet;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  SharedOptions, SharedResolver, hmr::hmr_ast_finalizer::HmrAstFinalizer,
  module_loader::ModuleLoader, type_alias::IndexEcmaAst, types::scan_stage_cache::ScanStageCache,
  utils::process_code_and_sourcemap::process_code_and_sourcemap,
};

pub struct HmrManagerInput {
  pub module_db: ModuleTable,
  pub options: SharedOptions,
  pub fs: OsFileSystem,
  pub resolver: SharedResolver,
  pub plugin_driver: SharedPluginDriver,
  pub index_ecma_ast: IndexEcmaAst,
  pub cache: ScanStageCache,
  pub session_span: tracing::Span,
}

pub struct HmrManager {
  input: HmrManagerInput,
  module_idx_by_abs_path: FxHashMap<ArcStr, ModuleIdx>,
}

impl Deref for HmrManager {
  type Target = HmrManagerInput;

  fn deref(&self) -> &Self::Target {
    &self.input
  }
}

impl DerefMut for HmrManager {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.input
  }
}

impl HmrManager {
  pub fn new(input: HmrManagerInput) -> Self {
    let module_idx_by_abs_path = input
      .module_db
      .modules
      .iter()
      .filter_map(|m| m.as_normal())
      .map(|m| {
        let filename = m.id.resource_id().clone();
        let module_idx = m.idx;
        (filename, module_idx)
      })
      .collect();
    Self { input, module_idx_by_abs_path }
  }

  pub async fn hmr_invalidate(
    &mut self,
    file: String,
    first_invalidated_by: Option<String>,
  ) -> BuildResult<HmrOutput> {
    let module_idx = self
      .cache
      .module_id_to_idx
      .get(&ArcStr::from(file))
      .expect("Not found hmr invalidate module")
      .idx();
    let module = self.module_db.modules[module_idx].as_normal().unwrap();

    // only self accept modules can be invalidated
    if !module.ast_usage.contains(EcmaModuleAstUsage::HmrSelfAccept) {
      return Ok(HmrOutput {
        is_self_accepting: false,
        first_invalidated_by,
        ..Default::default()
      });
    }

    // Modules could be empty if a root module is invalidated via import.meta.hot.invalidate()
    if module.importers_idx.is_empty() {
      return Ok(HmrOutput {
        is_self_accepting: true,
        first_invalidated_by,
        full_reload: true,
        ..Default::default()
      });
    }

    let mut ret =
      self.generate_hmr_patch(module.importers_idx.clone(), first_invalidated_by).await?;
    ret.is_self_accepting = true;
    Ok(ret)
  }

  #[expect(clippy::dbg_macro)] // FIXME: Remove dbg! macro once the feature is stable
  pub async fn hmr(&mut self, changed_file_paths: Vec<String>) -> BuildResult<HmrOutput> {
    let mut changed_modules = FxIndexSet::default();
    for changed_file_path in changed_file_paths {
      let changed_file_path = ArcStr::from(changed_file_path);
      match self.module_idx_by_abs_path.get(&changed_file_path) {
        Some(module_idx) => {
          changed_modules.insert(*module_idx);
        }
        _ => {
          dbg!("No corresponding module found for changed file path: {:?}", changed_file_path);
        }
      }
    }
    tracing::debug!(
      target: "hmr",
      "initial changed modules {:?}",
      changed_modules.iter()
        .map(|module_idx| self.module_db.modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );

    self.generate_hmr_patch(changed_modules, None).await
  }

  #[expect(clippy::too_many_lines)]
  pub async fn generate_hmr_patch(
    &mut self,
    mut changed_modules: FxIndexSet<ModuleIdx>,
    first_invalidated_by: Option<String>,
  ) -> BuildResult<HmrOutput> {
    let mut affected_modules = FxIndexSet::default();
    let mut hmr_boundaries = FxIndexSet::default();
    let mut need_to_full_reload = false;
    let mut full_reload_reason = None;
    while let Some(changed_module_idx) = changed_modules.pop() {
      if need_to_full_reload {
        break;
      }
      let mut visited_modules = FxHashSet::default();
      let mut boundaries = FxIndexSet::default();
      let is_reach_to_hmr_root_boundary = self.propagate_update(
        changed_module_idx,
        &mut visited_modules,
        &mut boundaries,
        &mut affected_modules,
      );

      if is_reach_to_hmr_root_boundary {
        need_to_full_reload = true;
        continue;
      }

      // If import.meta.hot.invalidate was called already on that module for the same update,
      // it means any importer of that module can't hot update. We should fallback to full reload.
      if let Some(first_invalidated_by) = first_invalidated_by.as_ref() {
        if boundaries.iter().any(|boundary| {
          self.module_db.modules[boundary.accepted_via].stable_id() == first_invalidated_by
        }) {
          need_to_full_reload = true;
          full_reload_reason = Some("circular import invalidate".to_string());
          continue;
        }
      }

      hmr_boundaries.extend(boundaries);
    }

    if need_to_full_reload {
      return Ok(HmrOutput {
        full_reload_reason,
        first_invalidated_by,
        full_reload: true,
        ..Default::default()
      });
    }

    tracing::debug!(
      target: "hmr",
      "computed out `affected_modules` {:?}",
      affected_modules.iter()
        .map(|module_idx| self.module_db.modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );

    let mut modules_to_invalidate = changed_modules.clone();
    // FIXME(hyf0): In general, only modules got edited need to be invalidated, because we need to refetch their latest content.
    // For those modules that are not edited, we should be able to reuse their AST. But currently we don't have a good way to do that
    // due to architecture limitation.
    modules_to_invalidate.extend(affected_modules.clone());

    let module_infos_to_be_updated = modules_to_invalidate
      .iter()
      .map(|module_idx| {
        let module = &self.module_db.modules[*module_idx];
        let Module::Normal(module) = module else {
          unreachable!("HMR only supports normal module");
        };
        module.originative_resolved_id.clone()
      })
      .collect::<Vec<_>>();

    let mut scan_stage_cache = std::mem::take(&mut self.cache);

    let mut module_loader = ModuleLoader::new(
      self.fs,
      Arc::clone(&self.options),
      Arc::clone(&self.resolver),
      Arc::clone(&self.plugin_driver),
      &mut scan_stage_cache,
      false,
      self.session_span.clone(),
    )?;

    let module_loader_output =
      module_loader.fetch_modules(vec![], &module_infos_to_be_updated).await?;

    // We manually impl `Drop` for `ModuleLoader` to avoid missing assign `importers` to
    // `self.cache`, but rustc is not smart enough to infer actually we don't touch it in `drop`
    // implementation, so we need to manually drop it.
    drop(module_loader);

    self.cache = scan_stage_cache;

    tracing::debug!(
      target: "hmr",
      "New added modules` {:?}",
      module_loader_output
        .new_added_modules_from_partial_scan
        .iter()
        .map(|module_idx| module_loader_output.module_table.get(*module_idx).stable_id())
        .collect::<Vec<_>>(),
    );

    affected_modules.extend(module_loader_output.new_added_modules_from_partial_scan);

    let mut updated_modules =
      module_loader_output.module_table.into_iter_enumerated().into_iter().collect::<Vec<_>>();
    tracing::debug!(
      target: "hmr",
      "updated_modules` {:?}",
      updated_modules
        .iter().map(|(idx, module)| (idx, module.stable_id()))
        .collect::<Vec<_>>(),
    );
    updated_modules.sort_by_key(|(idx, _)| *idx);

    // TODO(hyf0): This is a temporary merging solution. We need to find a better way to handle this.
    for (idx, module) in updated_modules {
      if idx.index() >= self.module_db.modules.len() {
        // This module is newly added, we need to insert it into the module db.
        let generated_id = self.module_db.modules.push(module);
        assert_eq!(generated_id, idx, "Module index mismatch");
      } else {
        // This module is already in the module db, we need to update it.
        self.module_db.modules[idx] = module;
      }
    }
    tracing::debug!(
      target: "hmr",
      "New added modules2` {:?}",
      affected_modules
        .iter()
        .map(|module_idx| self.module_db.modules[*module_idx].stable_id())
        .collect::<Vec<_>>(),
    );
    self.index_ecma_ast = module_loader_output.index_ecma_ast;

    // Remove external modules from affected_modules.
    affected_modules.retain(|idx| {
      let module = &self.module_db.modules[*idx];
      // It's possible that affected modules are external modules. New added code might contains import from external modules.
      // However, HMR doesn't need to deal with them.
      !matches!(module, Module::External(_))
    });

    let module_idx_to_init_fn_name = affected_modules
      .iter()
      .enumerate()
      .map(|(index, module_idx)| {
        let Module::Normal(module) = &self.module_db.modules[*module_idx] else {
          unreachable!(
            "External modules should be removed before. But got {:?}",
            self.module_db.modules[*module_idx].id()
          );
        };

        (*module_idx, format!("init_{}_{}", module.repr_name, index))
      })
      .collect::<FxHashMap<_, _>>();

    let mut source_joiner = SourceJoiner::default();

    for affected_module_idx in affected_modules {
      let affected_module = &self.input.module_db.modules[affected_module_idx];
      let Module::Normal(affected_module) = affected_module else {
        unreachable!("HMR only supports normal module");
      };

      let enable_sourcemap = self.options.sourcemap.is_some() && !affected_module.is_virtual();
      let ecma_ast_idx = affected_module.ecma_ast_idx.unwrap();
      let modules = &self.input.module_db.modules;
      let ast = &mut self.input.index_ecma_ast[ecma_ast_idx].0;

      ast.program.with_mut(|fields| {
        let scoping = EcmaAst::make_semantic(fields.program, /*with_cfg*/ false).into_scoping();
        let mut finalizer = HmrAstFinalizer {
          modules,
          alloc: fields.allocator,
          snippet: AstSnippet::new(fields.allocator),
          scoping: &scoping,
          import_binding: FxHashMap::default(),
          module: affected_module,
          exports: oxc::allocator::Vec::new_in(fields.allocator),
          affected_module_idx_to_init_fn_name: &module_idx_to_init_fn_name,
          dependencies: FxIndexSet::default(),
          imports: FxHashSet::default(),
        };

        finalizer.visit_program(fields.program);
      });

      let codegen = EcmaCompiler::print_with(
        ast,
        PrintOptions {
          sourcemap: enable_sourcemap,
          filename: affected_module.id.to_string(),
          print_legal_comments: false, // ignore hmr chunk comments
        },
      );
      if let Some(map) = codegen.map {
        source_joiner.append_source(SourceMapSource::new(codegen.code, map));
      } else {
        source_joiner.append_source(codegen.code);
      }
    }

    hmr_boundaries.iter().for_each(|boundary| {
      let init_fn_name = &module_idx_to_init_fn_name[&boundary.boundary];
      source_joiner.append_source(format!("{init_fn_name}()"));
    });

    source_joiner.append_source(format!(
      "__rolldown_runtime__.applyUpdates([{}]);",
      hmr_boundaries
        .iter()
        .map(|boundary| {
          let module = &self.module_db.modules[boundary.boundary];
          format!("'{}'", module.stable_id())
        })
        .collect::<Vec<_>>()
        .join(",")
    ));

    let (mut code, mut map) = source_joiner.join();

    let filename = format!(
      "{}.js",
      std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()
    );

    let file_dir = self.options.cwd.as_path().join(&self.options.out_dir);

    let sourcemap_asset = if let Some(map) = map.as_mut() {
      process_code_and_sourcemap(
        &self.options,
        &mut code,
        map,
        &file_dir,
        filename.as_str(),
        0,
        /*is_css*/ false,
      )
      .await?
    } else {
      None
    };

    Ok(HmrOutput {
      code,
      filename,
      sourcemap_filename: sourcemap_asset.as_ref().map(|asset| asset.filename.to_string()),
      sourcemap: sourcemap_asset.map(|asset| asset.source.try_into_string()).transpose()?,
      first_invalidated_by,
      hmr_boundaries: hmr_boundaries
        .into_iter()
        .map(|boundary| HmrBoundaryOutput {
          boundary: self.module_db.modules[boundary.boundary].stable_id().into(),
          accepted_via: self.module_db.modules[boundary.accepted_via].stable_id().into(),
        })
        .collect(),
      ..Default::default()
    })
  }

  fn propagate_update(
    &self,
    module_idx: ModuleIdx,
    visited_modules: &mut FxHashSet<ModuleIdx>,
    hmr_boundaries: &mut FxIndexSet<HmrBoundary>,
    affected_modules: &mut FxIndexSet<ModuleIdx>,
  ) -> bool /* is reached to hmr boundary  */ {
    let Module::Normal(module) = &self.module_db.modules[module_idx] else { return false };
    if visited_modules.contains(&module_idx) {
      // At this point, we consider circular dependencies as a full reload. We can improve this later.
      return true;
    }

    visited_modules.insert(module_idx);

    affected_modules.insert(module_idx);

    if module.ast_usage.contains(EcmaModuleAstUsage::HmrSelfAccept) {
      hmr_boundaries.insert(HmrBoundary { boundary: module_idx, accepted_via: module_idx });
      return false;
    }

    // If the module is not imported by one module, it should be a hmr root boundary.
    if module.importers_idx.is_empty() {
      return true;
    }

    for importer_idx in &module.importers_idx {
      let Module::Normal(importer) = &self.module_db.modules[*importer_idx] else {
        continue;
      };

      if importer.hmr_info.deps.contains(&module.id) {
        affected_modules.insert(*importer_idx);
        hmr_boundaries.insert(HmrBoundary { boundary: *importer_idx, accepted_via: module_idx });
        continue;
      }

      if self.propagate_update(*importer_idx, visited_modules, hmr_boundaries, affected_modules) {
        return true;
      }
    }

    false
  }
}
