use std::{sync::Arc, vec};

use anyhow::Result;
use futures::future::join_all;
use oxc_index::IndexVec;
use rolldown_common::{
  side_effects::{DeterminedSideEffects, HookSideEffects},
  AstScope, ImportRecordId, ModuleType, NormalModule, NormalModuleId, PackageJson, RawImportRecord,
  ResolvedPath, ResolvedRequestInfo, ResourceId, SymbolRef,
};
use rolldown_error::BuildError;
use rolldown_oxc_utils::OxcAst;
use rolldown_plugin::{HookResolveIdExtraOptions, SharedPluginDriver};
use rolldown_resolver::ResolveError;
use sugar_path::SugarPath;

use super::{task_context::TaskContext, Msg};
use crate::module_loader::error::ResolveDependenciesError;
use crate::{
  ast_scanner::{AstScanner, ScanResult},
  module_loader::NormalModuleTaskResult,
  types::ast_symbols::AstSymbols,
  utils::{
    load_source::load_source, parse_to_ast::parse_to_ast, resolve_id::resolve_id,
    transform_source::transform_source, tweak_ast_for_scanning::tweak_ast_for_scanning,
  },
  SharedOptions, SharedResolver,
};

pub struct NormalModuleTask {
  ctx: Arc<TaskContext>,
  module_id: NormalModuleId,
  resolved_path: ResolvedPath,
  package_json: Option<Arc<PackageJson>>,
  module_type: ModuleType,
  errors: Vec<BuildError>,
  is_user_defined_entry: bool,
  side_effects: Option<HookSideEffects>,
}

impl NormalModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    id: NormalModuleId,
    path: ResolvedPath,
    module_type: ModuleType,
    is_user_defined_entry: bool,
    package_json: Option<Arc<PackageJson>>,
    side_effects: Option<HookSideEffects>,
  ) -> Self {
    Self {
      ctx,
      module_id: id,
      resolved_path: path,
      module_type,
      errors: vec![],
      is_user_defined_entry,
      package_json,
      side_effects,
    }
  }

  #[tracing::instrument(name = "NormalModuleTask::run", level = "trace", skip_all, fields(
    module_path = ? self.resolved_path
  ))]
  pub async fn run(mut self) {
    match self.run_inner().await {
      Ok(()) => {
        if !self.errors.is_empty() {
          self.ctx.tx.send(Msg::BuildErrors(self.errors)).await.expect("Send should not fail");
        }
      }
      Err(err) => {
        self.ctx.tx.send(Msg::Panics(err)).await.expect("Send should not fail");
      }
    }
  }

  #[allow(clippy::too_many_lines)]
  async fn run_inner(&mut self) -> Result<()> {
    let mut hook_side_effects = self.side_effects.take();
    let mut sourcemap_chain = vec![];
    let mut warnings = vec![];

    // Run plugin load to get content first, if it is None using read fs as fallback.
    let source = load_source(
      &self.ctx.plugin_driver,
      &self.resolved_path,
      &self.ctx.fs,
      &mut sourcemap_chain,
      &mut hook_side_effects,
    )
    .await?;

    // Run plugin transform.
    let source: Arc<str> = transform_source(
      &self.ctx.plugin_driver,
      &self.resolved_path,
      source,
      &mut sourcemap_chain,
      &mut hook_side_effects,
    )
    .await?
    .into();

    let mut ast = parse_to_ast(
      &self.ctx.input_options,
      self.resolved_path.path.as_path(),
      Arc::clone(&source),
    )?;

    let (scope, scan_result, ast_symbol, namespace_symbol) = self.scan(&mut ast, &source);

    let resolved_deps_res =
      self.resolve_dependencies(&scan_result.import_records, &mut warnings).await;

    match resolved_deps_res {
      Ok(resolved_deps) => {
        let ScanResult {
          named_imports,
          named_exports,
          stmt_infos,
          import_records,
          star_exports,
          default_export_ref,
          imports,
          exports_kind,
          repr_name,
          warnings: scan_warnings,
        } = scan_result;
        warnings.extend(scan_warnings);

        let mut imported_ids = vec![];
        let mut dynamically_imported_ids = vec![];

        for (record, info) in import_records.iter().zip(&resolved_deps) {
          if record.kind.is_static() {
            imported_ids.push(Arc::clone(&info.path.path).into());
          } else {
            dynamically_imported_ids.push(Arc::clone(&info.path.path).into());
          }
        }

        let resource_id = ResourceId::new(Arc::clone(&self.resolved_path.path));
        let stable_resource_id = resource_id.stabilize(&self.ctx.input_options.cwd);

        // The side effects priority is:
        // 1. Hook side effects
        // 2. Package.json side effects
        // 3. Analyzed side effects
        // We should skip the `check_side_effects_for` if the hook side effects is not `None`.
        let lazy_check_side_effects = || {
          self
            .package_json
            .as_ref()
            .and_then(|p| {
              p.check_side_effects_for(&stable_resource_id).map(DeterminedSideEffects::UserDefined)
            })
            .unwrap_or_else(|| {
              let analyzed_side_effects = stmt_infos.iter().any(|stmt_info| stmt_info.side_effect);
              DeterminedSideEffects::Analyzed(analyzed_side_effects)
            })
        };

        let side_effects = match hook_side_effects {
          Some(side_effects) => match side_effects {
            HookSideEffects::True => DeterminedSideEffects::UserDefined(true),
            HookSideEffects::False => DeterminedSideEffects::UserDefined(false),
            HookSideEffects::NoTreeshake => unimplemented!(),
          },
          None => lazy_check_side_effects(),
        };
        // TODO: Should we check if there are `check_side_effects_for` returns false but there are side effects in the module?

        let module = NormalModule {
          source,
          id: self.module_id,
          repr_name,
          stable_resource_id,
          resource_id,
          named_imports,
          named_exports,
          stmt_infos,
          imports,
          star_exports,
          default_export_ref,
          scope,
          exports_kind,
          namespace_symbol,
          module_type: self.module_type,
          debug_resource_id: self.resolved_path.debug_display(&self.ctx.input_options.cwd),
          sourcemap_chain,
          exec_order: u32::MAX,
          is_user_defined_entry: self.is_user_defined_entry,
          import_records: IndexVec::default(),
          is_included: false,
          importers: vec![],
          dynamic_importers: vec![],
          imported_ids,
          dynamically_imported_ids,
          package_json: self.package_json.take(),
          side_effects,
        };

        self.ctx.plugin_driver.module_parsed(Arc::new(module.to_module_info())).await?;

        self
          .ctx
          .tx
          .send(Msg::NormalModuleDone(NormalModuleTaskResult {
            resolved_deps,
            module_id: self.module_id,
            warnings,
            ast_symbol,
            module,
            raw_import_records: import_records,
            ast,
          }))
          .await
          .expect("Send should not fail");
        Ok(())
      }
      Err(e) => match e {
        ResolveDependenciesError::AnyhowError(ex) => Err(ex),
        ResolveDependenciesError::ResolveError(ex) => {
          for e in ex {
            match e {
              ResolveError::JSON(e) => {
                let build_error =
                  BuildError::unresolved_json_illegally(e.path, e.column, e.line, e.message);
                self.errors.push(build_error);
              }
              _ => {
                let build_error = BuildError::unresolved_other_unknown(e.to_string());
                self.errors.push(build_error);
              }
            }
          }
          Ok(())
        }
      },
    }
  }

  fn scan(
    &self,
    ast: &mut OxcAst,
    source: &Arc<str>,
  ) -> (AstScope, ScanResult, AstSymbols, SymbolRef) {
    let (mut symbol_table, scope) = ast.make_symbol_table_and_scope_tree();
    let ast_scope = AstScope::new(
      scope,
      std::mem::take(&mut symbol_table.references),
      std::mem::take(&mut symbol_table.resolved_references),
    );
    let mut symbol_for_module = AstSymbols::from_symbol_table(symbol_table);
    let file_path = Arc::<str>::clone(&self.resolved_path.path).into();
    let repr_name = ResourceId::representative_name(&file_path);
    tweak_ast_for_scanning(ast);
    let scanner = AstScanner::new(
      self.module_id,
      &ast_scope,
      &mut symbol_for_module,
      repr_name.into_owned(),
      self.module_type,
      source,
      &file_path,
      &ast.trivias,
    );
    let namespace_symbol = scanner.namespace_ref;
    let scan_result = scanner.scan(ast.program());

    (ast_scope, scan_result, symbol_for_module, namespace_symbol)
  }

  #[allow(clippy::option_if_let_else)]
  pub(crate) async fn resolve_id(
    input_options: &SharedOptions,
    resolver: &SharedResolver,
    plugin_driver: &SharedPluginDriver,
    importer: &str,
    specifier: &str,
    options: HookResolveIdExtraOptions,
  ) -> anyhow::Result<Result<ResolvedRequestInfo, ResolveError>> {
    // Check external with unresolved path
    if let Some(is_external) = input_options.external.as_ref() {
      if is_external(specifier, Some(importer), false).await? {
        return Ok(Ok(ResolvedRequestInfo {
          path: specifier.to_string().into(),
          module_type: ModuleType::Unknown,
          is_external: true,
          package_json: None,
          side_effects: None,
        }));
      }
    }

    let resolved_id =
      resolve_id(resolver, plugin_driver, specifier, Some(importer), options).await?;

    match resolved_id {
      Ok(mut resolved_id) => {
        if !resolved_id.is_external {
          // Check external with resolved path
          if let Some(is_external) = input_options.external.as_ref() {
            resolved_id.is_external = is_external(specifier, Some(importer), true).await?;
          }
        }
        Ok(Ok(resolved_id))
      }
      Err(e) => Ok(Err(e)),
    }
  }

  async fn resolve_dependencies(
    &mut self,
    dependencies: &IndexVec<ImportRecordId, RawImportRecord>,
    warnings: &mut Vec<BuildError>,
  ) -> std::result::Result<IndexVec<ImportRecordId, ResolvedRequestInfo>, ResolveDependenciesError>
  {
    let jobs = dependencies.iter_enumerated().map(|(idx, item)| {
      let specifier = item.module_request.clone();
      let input_options = Arc::clone(&self.ctx.input_options);
      // FIXME(hyf0): should not use `Arc<Resolver>` here
      let resolver = Arc::clone(&self.ctx.resolver);
      let plugin_driver = Arc::clone(&self.ctx.plugin_driver);
      let importer = self.resolved_path.clone();
      let kind = item.kind;
      async move {
        Self::resolve_id(
          &input_options,
          &resolver,
          &plugin_driver,
          &importer.path,
          &specifier,
          HookResolveIdExtraOptions { is_entry: false, kind },
        )
        .await
        .map(|id| (specifier, idx, id))
      }
    });

    let resolved_ids = join_all(jobs).await;

    let mut ret = IndexVec::with_capacity(dependencies.len());
    let mut build_errors = vec![];
    for resolved_id in resolved_ids {
      match resolved_id {
        Ok(resolved_id) => {
          let (specifier, idx, resolved_id) = resolved_id;
          match resolved_id {
            Ok(info) => {
              ret.push(info);
            }
            Err(e) => match &e {
              ResolveError::NotFound(..) => {
                warnings.push(
                  BuildError::unresolved_import_treated_as_external(
                    specifier.to_string(),
                    self.resolved_path.path.to_string(),
                    Some(e),
                  )
                  .with_severity_warning(),
                );
                ret.push(ResolvedRequestInfo {
                  path: specifier.to_string().into(),
                  module_type: ModuleType::Unknown,
                  is_external: true,
                  package_json: None,
                  side_effects: None,
                });
              }
              _ => {
                build_errors.push((&dependencies[idx], e));
              }
            },
          }
        }
        Err(e) => {
          return Err(ResolveDependenciesError::AnyhowError(e));
        }
      }
    }

    if build_errors.is_empty() {
      Ok(ret)
    } else {
      let resolved_err: Vec<ResolveError> = build_errors.into_iter().map(|x| x.1).collect();
      Err(ResolveDependenciesError::ResolveError(resolved_err))
    }
  }
}
