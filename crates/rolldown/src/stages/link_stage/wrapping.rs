use petgraph::prelude::DiGraphMap;
use rolldown_common::{
  ExportsKind, ImportDeferExternal, ImportKind, IndexModules, Module, ModuleIdx, NormalModule,
  NormalizedBundlerOptions, OutputFormat, RuntimeModuleBrief, Specifier, StmtInfo, StmtInfoMeta,
  StmtInfos, SymbolRefDb, TaggedSymbolRef, WrapKind,
};
use rolldown_error::BuildDiagnostic;
use rolldown_utils::IndexBitSet;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::smallvec;

use crate::types::linking_metadata::{DeferredCjsNamespace, LinkingMetadata, LinkingMetadataVec};

use super::LinkStage;

struct Context<'a> {
  pub visited_modules: &'a mut IndexBitSet<ModuleIdx>,
  pub linking_infos: &'a mut LinkingMetadataVec,
  pub modules: &'a IndexModules,
  pub runtime_idx: ModuleIdx,
}

fn wrap_module_recursively(ctx: &mut Context, target: ModuleIdx) {
  // Only consider `NormalModule`
  if !ctx.visited_modules.set_bit(target) {
    return;
  }

  let Module::Normal(module) = &ctx.modules[target] else {
    return;
  };

  if target == ctx.runtime_idx {
    // Runtime module should not be wrapped.
    // FIXME(hyf0): Currently, only hmr situation will fall into this branch, we should find a better way to handle this.
    return;
  }

  if matches!(ctx.linking_infos[target].wrap_kind(), WrapKind::None) {
    let new_wrap_kind = match module.exports_kind {
      ExportsKind::Esm | ExportsKind::None => WrapKind::Esm,
      ExportsKind::CommonJs => WrapKind::Cjs,
    };
    ctx.linking_infos[target].set_wrap_kind(new_wrap_kind);
  }

  module.import_records.iter().for_each(|rec| {
    let Some(module_idx) = rec.state.resolved_module else { return };
    if matches!(rec.kind, ImportKind::Require) {
      ctx.linking_infos[module_idx].required_by_other_module = true;
    }
    wrap_module_recursively(ctx, module_idx);
  });
}

/// Marks CommonJS and external modules reached through `export *`, and every module that
/// re-exports one of them through a chain of `export *`, cycles included, as having dynamic
/// exports.
fn determine_has_dynamic_exports(modules: &IndexModules, linking_infos: &mut LinkingMetadataVec) {
  let mut star_importers: FxHashMap<ModuleIdx, Vec<ModuleIdx>> = FxHashMap::default();
  let mut queue = vec![];
  let mut mark = |idx: ModuleIdx, queue: &mut Vec<ModuleIdx>| {
    if !linking_infos[idx].has_dynamic_exports {
      linking_infos[idx].has_dynamic_exports = true;
      queue.push(idx);
    }
  };

  for module in modules.iter().filter_map(Module::as_normal).filter(|m| m.has_star_export()) {
    if matches!(module.exports_kind, ExportsKind::CommonJs) {
      mark(module.idx, &mut queue);
    }
    for importee_id in module.star_export_module_ids() {
      if importee_id == module.idx {
        continue;
      }
      star_importers.entry(importee_id).or_default().push(module.idx);
      let is_dynamic = match &modules[importee_id] {
        Module::Normal(importee) => matches!(importee.exports_kind, ExportsKind::CommonJs),
        Module::External(_) => true,
      };
      if is_dynamic {
        mark(importee_id, &mut queue);
      }
    }
  }

  while let Some(idx) = queue.pop() {
    if let Some(importers) = star_importers.get(&idx) {
      for &importer in importers {
        mark(importer, &mut queue);
      }
    }
  }
}

impl LinkStage<'_> {
  /// Returns the ESM targets of `import defer`, sorted. `create_deferred_namespaces` gives them a
  /// deferred namespace object after `generate_lazy_export`.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn wrap_modules(&mut self) -> Vec<ModuleIdx> {
    let mut visited_modules_for_wrapping = IndexBitSet::new(self.module_table.modules.len());
    determine_has_dynamic_exports(&self.module_table.modules, &mut self.metas);
    let deferred_esm_targets = if self.options.experimental.is_import_defer_enabled() {
      self.check_deferred_externals();
      self.wrap_import_defer_closures()
    } else {
      vec![]
    };

    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      let module_id = module.idx;

      let need_to_wrap = !self.metas[module_id].wrap_kind().is_none();

      if need_to_wrap {
        wrap_module_recursively(
          &mut Context {
            visited_modules: &mut visited_modules_for_wrapping,
            linking_infos: &mut self.metas,
            modules: &self.module_table.modules,
            runtime_idx: self.runtime.id(),
          },
          module_id,
        );
      } else {
        // Make sure depended cjs modules got wrapped.
        module.import_records.iter().for_each(|rec| {
          let Some(module_idx) = rec.resolved_module else { return };
          let Module::Normal(importee) = &self.module_table[module_idx] else {
            return;
          };
          if matches!(rec.kind, ImportKind::Require) {
            self.metas[importee.idx].required_by_other_module = true;
          }
          // Commonjs as a dependency must be wrapped. The wrapper is like a commonjs runtime to help initialize the commonjs module correctly.
          if matches!(importee.exports_kind, ExportsKind::CommonJs) {
            wrap_module_recursively(
              &mut Context {
                visited_modules: &mut visited_modules_for_wrapping,
                linking_infos: &mut self.metas,
                modules: &self.module_table.modules,
                runtime_idx: self.runtime.id(),
              },
              importee.idx,
            );
          }
        });
      }
    }

    // Under strict execution order every CommonJS module must stay behind its lazy `require_*`
    // wrapper once a co-locating `codeSplitting` group is in play. The generate-stage order
    // lowering only wraps ESM modules, and the interop rules above leave a CommonJS module that
    // nothing imports (a CommonJS entry in cjs output) unwrapped — its body would run eagerly at
    // the top level of whatever chunk hosts it, so a group capturing it would leak one entry's
    // execution into another and let competing top-level `module.exports` assignments clobber
    // each other. Without groups no chunking mechanism moves a never-imported CommonJS module out
    // of its own entry chunk, and rendering it raw there is not only safe but load-bearing: the
    // raw body keeps the entry's real Node module contract (`module.filename`,
    // `require.main === module`, the `exports` object shape), which a wrapper's synthetic
    // `module`/`exports` parameters cannot reproduce. The runtime module is ESM, so the
    // `exports_kind` check below skips it.
    if self.options.is_strict_execution_order_enabled()
      && self.options.has_manual_code_splitting_groups()
    {
      for (idx, linking_info) in self.metas.iter_mut_enumerated() {
        let Some(module) = self.module_table[idx].as_normal() else {
          continue;
        };
        if matches!(module.exports_kind, ExportsKind::CommonJs) {
          linking_info.set_wrap_kind(WrapKind::Cjs);
        }
      }
    }

    for m in &self.module_table.modules {
      let Some(ecma_module) = m.as_normal() else { continue };
      let idx = ecma_module.idx;
      let linking_info = &mut self.metas[idx];
      let stmt_infos = &mut self.stmt_infos[idx];
      create_wrapper(
        ecma_module,
        stmt_infos,
        linking_info,
        &mut self.symbols,
        &self.runtime,
        self.options,
      );
    }
    // The runtime module has `__deferNs` only when the option is on.
    if self.options.experimental.is_import_defer_enabled() {
      self.create_deferred_cjs_namespaces();
      self.reference_tracked_dependency_wrappers();
    }
    deferred_esm_targets
  }
}

impl LinkStage<'_> {
  /// Checks each deferred import of an external module, static or `import.defer()`:
  /// - ESM output: a build error, unless `experimental.importDeferExternal` is `'preserve'`. Then
  ///   the output keeps `import defer`.
  /// - IIFE and UMD output: the external module is a global or a wrapper argument, so it
  ///   evaluates eagerly. This is a warning.
  /// - CommonJS output loads the external module lazily with `require()`. No diagnostic.
  fn check_deferred_externals(&mut self) {
    let format = self.options.format;
    let preserve =
      matches!(self.options.experimental.import_defer_external(), ImportDeferExternal::Preserve);
    if matches!(format, OutputFormat::Cjs) || (matches!(format, OutputFormat::Esm) && preserve) {
      return;
    }
    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      for rec in &module.import_records {
        if !rec.is_defer_phase() {
          continue;
        }
        let Some(target) = rec.resolved_module else { continue };
        let Module::External(external) = &self.module_table.modules[target] else { continue };
        let diagnostic = if matches!(format, OutputFormat::Esm) {
          BuildDiagnostic::import_defer_external(
            module.id.as_arc_str().clone(),
            module.source.clone(),
            rec.importer_span,
            external.id.as_arc_str().clone(),
          )
        } else {
          let ext = external.id.as_arc_str();
          let format = if matches!(format, OutputFormat::Iife) { "IIFE" } else { "UMD" };
          let message = if rec.kind == ImportKind::DynamicImport {
            // The output keeps a native `import()` of the external module.
            format!(
              "`import.defer()` of the external module \"{ext}\" is lowered to `import()` in {format} output. The module evaluates when the `import()` call loads it, not at the first access to its namespace."
            )
          } else {
            format!(
              "`import defer` of the external module \"{ext}\" is lowered to a normal import in {format} output, because the module is a global or a wrapper argument. The module evaluates when the bundle starts, not at the first access to its namespace."
            )
          };
          BuildDiagnostic::unsupported_feature(
            module.id.as_arc_str().clone(),
            module.source.clone(),
            rec.importer_span,
            message,
          )
          .with_severity_warning()
        };
        self.diagnostics.push(diagnostic);
      }
    }
  }

  /// Wraps each target of `import defer` and the modules of its static dependency closure. A
  /// deferred access can evaluate a module of the closure before its eager position, so each one
  /// needs a lazy wrapper. The loop in `wrap_modules` then wraps the rest of their graph as for
  /// any wrapped module. Returns the ESM targets, sorted, which get a deferred namespace.
  fn wrap_import_defer_closures(&mut self) -> Vec<ModuleIdx> {
    let runtime_idx = self.runtime.id();
    let mut stack = vec![];
    let mut esm_targets = vec![];
    let mut dynamic_targets = vec![];
    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      for (local_ref, named_import) in &module.named_imports {
        let rec = &module.import_records[named_import.record_idx];
        if !rec.is_defer_phase() || !matches!(named_import.imported, Specifier::Star) {
          continue;
        }
        let Some(target) = rec.resolved_module else { continue };
        stack.push(target);
        let Module::Normal(target_module) = &self.module_table.modules[target] else {
          // CommonJS output loads a deferred external module lazily with `require()`, with the
          // same deferred namespace as a CommonJS module. Other formats report it
          // (`check_deferred_externals`).
          if matches!(self.options.format, OutputFormat::Cjs) {
            self.metas[module.idx]
              .deferred_cjs_namespace_imports
              .insert(*local_ref, named_import.record_idx);
          }
          continue;
        };
        if target == runtime_idx {
          continue;
        }
        if matches!(target_module.exports_kind, ExportsKind::CommonJs) {
          self.metas[module.idx]
            .deferred_cjs_namespace_imports
            .insert(*local_ref, named_import.record_idx);
        } else {
          esm_targets.push(target);
        }
      }
      // `import.defer('./x')`. A CommonJS target keeps the lowering of `import()`: the
      // proposal does not cover CommonJS modules.
      for rec in &module.import_records {
        if rec.kind != ImportKind::DynamicImport || !rec.is_defer_phase() {
          continue;
        }
        let Some(target) = rec.resolved_module else { continue };
        let Module::Normal(target_module) = &self.module_table.modules[target] else { continue };
        if target == runtime_idx || matches!(target_module.exports_kind, ExportsKind::CommonJs) {
          continue;
        }
        stack.push(target);
        esm_targets.push(target);
        dynamic_targets.push(target);
      }
    }
    esm_targets.sort_unstable();
    esm_targets.dedup();
    for target in dynamic_targets {
      self.metas[target].dynamic_defer_target = true;
    }

    let mut visited = IndexBitSet::new(self.module_table.modules.len());
    while let Some(module_idx) = stack.pop() {
      if module_idx == runtime_idx || !visited.set_bit(module_idx) {
        continue;
      }
      let Module::Normal(module) = &self.module_table.modules[module_idx] else { continue };
      let meta = &mut self.metas[module_idx];
      meta.wrapped_by_import_defer = true;
      if meta.wrap_kind().is_none() {
        meta.set_wrap_kind(match module.exports_kind {
          ExportsKind::Esm | ExportsKind::None => WrapKind::Esm,
          ExportsKind::CommonJs => WrapKind::Cjs,
        });
      }
      stack.extend(
        module
          .import_records
          .iter()
          .filter(|rec| rec.kind.is_static())
          .filter_map(|rec| rec.resolved_module),
      );
    }
    self.track_import_defer_modules(&visited, &esm_targets);
    esm_targets
  }

  /// Marks the modules that need the state records of `__esmState`, in the graph of the requested
  /// modules of all phases (`import`, `import defer`, `import.defer()`) between the ESM modules of
  /// the deferred closures:
  /// - each module of a strongly connected component that contains a defer target. Only these
  ///   modules can be evaluating at a deferred access to the target.
  /// - each module with top-level await or a dependency with top-level await
  ///   (`is_tla_or_contains_tla_dependency`), and the other modules of its component. A deferred
  ///   import waits for these modules first (spec `GatherAsynchronousTransitiveDependencies`).
  ///
  /// The dependency list of a tracked module has its tracked requested modules, in source order.
  fn track_import_defer_modules(
    &mut self,
    closure: &IndexBitSet<ModuleIdx>,
    targets: &[ModuleIdx],
  ) {
    let requested = |module: &NormalModule| {
      let mut deps = module
        .import_records
        .iter()
        .filter(|rec| {
          matches!(rec.kind, ImportKind::Import)
            || (matches!(rec.kind, ImportKind::DynamicImport) && rec.is_defer_phase())
        })
        .filter_map(|rec| rec.resolved_module)
        .filter(|&dep| {
          closure.has_bit(dep)
            && self.module_table.modules[dep]
              .as_normal()
              .is_some_and(|dep| !matches!(dep.exports_kind, ExportsKind::CommonJs))
        })
        .collect::<Vec<_>>();
      let mut seen = FxHashSet::default();
      deps.retain(|dep| seen.insert(*dep));
      deps
    };
    let mut graph = DiGraphMap::<ModuleIdx, ()>::new();
    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      if !closure.has_bit(module.idx) || matches!(module.exports_kind, ExportsKind::CommonJs) {
        continue;
      }
      graph.add_node(module.idx);
      for dep in requested(module) {
        graph.add_edge(module.idx, dep, ());
      }
    }
    let targets = targets.iter().copied().collect::<FxHashSet<_>>();
    let mut tracked = FxHashSet::default();
    for component in petgraph::algo::tarjan_scc(&graph) {
      let is_cycle = component.len() > 1 || graph.contains_edge(component[0], component[0]);
      let has_target = is_cycle && component.iter().any(|idx| targets.contains(idx));
      if has_target
        || component.iter().any(|&idx| self.metas[idx].is_tla_or_contains_tla_dependency)
      {
        tracked.extend(component);
      }
    }
    for &idx in &tracked {
      let Module::Normal(module) = &self.module_table.modules[idx] else { continue };
      let deps = requested(module).into_iter().filter(|dep| tracked.contains(dep)).collect();
      self.metas[idx].import_defer_tracked_deps = Some(deps);
    }
  }

  /// Creates the deferred namespace object `x_deferred` of each ESM target of `import defer`:
  /// `var x_deferred = __deferNs(() => (init_x(), x_exports), x_exports)`. The finalizer emits the
  /// declaration after the namespace object, when tree shaking keeps the facade statement.
  /// Call it after `generate_lazy_export`: for a JSON module, `json_object_expr_to_esm` makes a
  /// new symbol table and new statement infos, and `unwrap_json_module` removes the wrapper.
  pub(super) fn create_deferred_namespaces(&mut self, targets: &[ModuleIdx]) {
    for &target in targets {
      let Module::Normal(module) = &self.module_table.modules[target] else { continue };
      let deferred_ref = self
        .symbols
        .create_facade_root_symbol_ref(target, &format!("{}_deferred", module.repr_name));
      let meta = &mut self.metas[target];
      let mut referenced_symbols =
        vec![self.runtime.resolve_symbol("__deferNs").into(), module.namespace_object_ref.into()];
      if let Some(wrapper_ref) = meta.wrapper_ref {
        referenced_symbols.push(wrapper_ref.into());
      }
      if meta.import_defer_tracked_deps.is_some() {
        referenced_symbols.push(self.runtime.resolve_symbol("__esmDefer").into());
      }
      let stmt_info = StmtInfo {
        declared_symbols: smallvec![TaggedSymbolRef::normal(deferred_ref)],
        referenced_symbols,
        eval_flags: false.into(),
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),
        force_tree_shaking: true,
      };
      meta.deferred_namespace_stmt_info = Some(self.stmt_infos[target].add_stmt_info(stmt_info));
      meta.deferred_namespace_ref = Some(deferred_ref);
    }
  }

  /// Creates the deferred namespace object of each `import defer * as ns from './x'` where `./x`
  /// is a CommonJS module. The importer owns the object, because the interop mode of `__toESM`
  /// comes from the importer. The finalizer declares it with `import_x`, when tree shaking keeps
  /// the facade statement:
  /// `var import_x, x_deferred = __deferNs(() => import_x || (import_x = __toESM(require_x())))`.
  /// Only an escape (`f(ns)`, `export { ns }`) reads the object. An access through the local
  /// binding reads `import_x` directly. An external module in CommonJS output gets the same
  /// object, with `require("ext")` in place of `require_x()`.
  fn create_deferred_cjs_namespaces(&mut self) {
    let defer_ns_ref = self.runtime.resolve_symbol("__deferNs");
    for importer_idx in self.metas.indices() {
      if self.metas[importer_idx].deferred_cjs_namespace_imports.is_empty() {
        continue;
      }
      let Module::Normal(importer) = &self.module_table.modules[importer_idx] else { continue };
      let mut records = self.metas[importer_idx]
        .deferred_cjs_namespace_imports
        .values()
        .copied()
        .collect::<Vec<_>>();
      records.sort_unstable();
      for rec_idx in records {
        let rec = &importer.import_records[rec_idx];
        let Some(target) = rec.resolved_module else { continue };
        let mut referenced_symbols = vec![defer_ns_ref.into(), rec.namespace_ref.into()];
        let name = match &self.module_table.modules[target] {
          Module::Normal(target_module) => {
            let Some(wrapper_ref) = self.metas[target].wrapper_ref else { continue };
            referenced_symbols.push(wrapper_ref.into());
            target_module.repr_name.as_str()
          }
          // The import statement declares `import_ext` and references `__toESM`.
          Module::External(external) => external.identifier_name.as_str(),
        };
        let symbol_ref =
          self.symbols.create_facade_root_symbol_ref(importer_idx, &format!("{name}_deferred"));
        let stmt_info = StmtInfo {
          declared_symbols: smallvec![TaggedSymbolRef::normal(symbol_ref)],
          referenced_symbols,
          eval_flags: false.into(),
          import_records: Vec::new(),
          #[cfg(debug_assertions)]
          debug_label: None,
          meta: StmtInfoMeta::default(),
          force_tree_shaking: true,
        };
        let stmt_info_idx = self.stmt_infos[importer_idx].add_stmt_info(stmt_info);
        self.metas[importer_idx]
          .deferred_cjs_namespaces
          .insert(rec_idx, DeferredCjsNamespace { symbol_ref, stmt_info_idx });
      }
    }
  }

  /// The wrapper of a tracked module lists the init functions of its tracked dependencies, for the
  /// ready check: `__esmState(() => { ... }, () => [init_y, init_z])`.
  fn reference_tracked_dependency_wrappers(&mut self) {
    for idx in self.metas.indices() {
      let Some(deps) = &self.metas[idx].import_defer_tracked_deps else { continue };
      let Some(stmt_idx) = self.metas[idx].wrapper_stmt_info else { continue };
      let wrappers = deps
        .iter()
        .filter_map(|&dep| self.metas[dep].wrapper_ref)
        .map(Into::into)
        .collect::<Vec<_>>();
      self.stmt_infos[idx][stmt_idx].referenced_symbols.extend(wrappers);
    }
  }
}

fn create_wrapper(
  module: &NormalModule,
  stmt_infos: &mut StmtInfos,
  linking_info: &mut LinkingMetadata,
  symbols: &mut SymbolRefDb,
  runtime: &RuntimeModuleBrief,
  options: &NormalizedBundlerOptions,
) {
  match linking_info.wrap_kind() {
    // If this is a CommonJS file, we're going to need to generate a wrapper
    // for the CommonJS closure. That will end up looking something like this:
    //
    //   var require_foo = __commonJS((exports, module) => {
    //     ...
    //   });
    //
    WrapKind::Cjs => {
      let wrapper_ref =
        symbols.create_facade_root_symbol_ref(module.idx, &format!("require_{}", module.repr_name));

      let stmt_info = StmtInfo {
        declared_symbols: smallvec![TaggedSymbolRef::normal(wrapper_ref)],
        referenced_symbols: vec![if options.profiler_names {
          runtime.resolve_symbol("__commonJS").into()
        } else {
          runtime.resolve_symbol("__commonJSMin").into()
        }],
        eval_flags: false.into(),
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),

        force_tree_shaking: true,
      };

      linking_info.wrapper_stmt_info = Some(stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    // If this is a lazily-initialized ESM file, we're going to need to
    // generate a wrapper for the ESM closure. That will end up looking
    // something like this:
    //
    //   var init_foo = __esm(() => {
    //     ...
    //   });
    //
    WrapKind::Esm => {
      let wrapper_ref =
        symbols.create_facade_root_symbol_ref(module.idx, &format!("init_{}", module.repr_name));

      let stmt_info = StmtInfo {
        declared_symbols: smallvec![TaggedSymbolRef::normal(wrapper_ref)],
        referenced_symbols: if linking_info.import_defer_tracked_deps.is_none() {
          vec![if options.profiler_names {
            runtime.resolve_symbol("__esm").into()
          } else {
            runtime.resolve_symbol("__esmMin").into()
          }]
        } else if linking_info.is_tla_or_contains_tla_dependency {
          // An async body: `try { ... } catch (e) { throw __esmFail(init_x, e) } __esmEnd(init_x)`.
          vec![
            runtime.resolve_symbol("__esmState").into(),
            runtime.resolve_symbol("__esmEnd").into(),
            runtime.resolve_symbol("__esmFail").into(),
          ]
        } else {
          vec![runtime.resolve_symbol("__esmState").into()]
        },
        eval_flags: true.into(),
        import_records: Vec::new(),
        #[cfg(debug_assertions)]
        debug_label: None,
        meta: StmtInfoMeta::default(),
        force_tree_shaking: true,
      };

      linking_info.wrapper_stmt_info = Some(stmt_infos.add_stmt_info(stmt_info));
      linking_info.wrapper_ref = Some(wrapper_ref);
    }
    WrapKind::None => {}
  }
}
