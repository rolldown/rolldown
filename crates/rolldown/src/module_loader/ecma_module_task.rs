use std::{path::Path, sync::Arc};

use anyhow::Result;
use futures::future::join_all;
use oxc::{
  index::IndexVec,
  semantic::{ScopeTree, SymbolTable},
};
use rolldown_common::{
  side_effects::{DeterminedSideEffects, HookSideEffects},
  AstScopes, EcmaModule, ImportRecordIdx, ModuleDefFormat, ModuleIdx, ModuleType, PackageJson,
  RawImportRecord, ResolvedPath, ResolvedRequestInfo, ResourceId, SymbolRef, TreeshakeOptions,
};
use rolldown_ecmascript::EcmaAst;
use rolldown_error::BuildError;
use rolldown_plugin::{HookResolveIdExtraOptions, SharedPluginDriver};
use rolldown_resolver::ResolveError;
use rolldown_utils::{ecma_script::legitimize_identifier_name, path_ext::PathExt};
use sugar_path::SugarPath;

use super::{task_context::TaskContext, Msg};
use crate::{
  ast_scanner::{AstScanner, ScanResult},
  module_loader::NormalModuleTaskResult,
  runtime::ROLLDOWN_RUNTIME_RESOURCE_ID,
  types::ast_symbols::AstSymbols,
  utils::{
    load_source::load_source, make_ast_symbol_and_scope::make_ast_scopes_and_symbols,
    parse_to_ecma_ast::parse_to_ecma_ast, resolve_id::resolve_id,
    transform_source::transform_source,
  },
  SharedOptions, SharedResolver,
};
pub struct EcmaModuleTask {
  ctx: Arc<TaskContext>,
  module_id: ModuleIdx,
  resolved_path: ResolvedPath,
  package_json: Option<Arc<PackageJson>>,
  module_type: ModuleDefFormat,
  errors: Vec<BuildError>,
  is_user_defined_entry: bool,
  side_effects: Option<HookSideEffects>,
}

impl EcmaModuleTask {
  pub fn new(
    ctx: Arc<TaskContext>,
    id: ModuleIdx,
    path: ResolvedPath,
    module_type: ModuleDefFormat,
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

  #[tracing::instrument(name="NormalModuleTask::run", level = "trace", skip_all, fields(module_path = ?self.resolved_path))]
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

    let module_type = {
      let ext =
        self.resolved_path.path.as_path().extension().and_then(|ext| ext.to_str()).unwrap_or("js");
      let module_type = self.ctx.input_options.module_types.get(ext);

      // FIXME: Once we support more types, we should return error instead of defaulting to JS.
      module_type.copied().unwrap_or(ModuleType::Js)
    };

    // Run plugin load to get content first, if it is None using read fs as fallback.
    let source = load_source(
      &self.ctx.plugin_driver,
      &self.resolved_path,
      module_type,
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

    let (mut ast, symbols, scopes) = parse_to_ecma_ast(
      &self.ctx.plugin_driver,
      Path::new(&self.resolved_path.path.as_ref()),
      &self.ctx.input_options,
      module_type,
      Arc::clone(&source),
    )?;

    let (scope, scan_result, ast_symbol, namespace_object_ref) =
      self.scan(&mut ast, &source, symbols, scopes);

    let resolved_deps =
      self.resolve_dependencies(&scan_result.import_records, &mut warnings).await?;

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
        HookSideEffects::True => lazy_check_side_effects(),
        HookSideEffects::False => DeterminedSideEffects::UserDefined(false),
        HookSideEffects::NoTreeshake => DeterminedSideEffects::NoTreeshake,
      },
      // If user don't specify the side effects, we use fallback value from `option.treeshake.moduleSideEffects`;
      None => match self.ctx.input_options.treeshake {
        // Actually this convert is not necessary, just for passing type checking
        TreeshakeOptions::False => DeterminedSideEffects::NoTreeshake,
        TreeshakeOptions::Option(ref opt) => {
          if opt.module_side_effects.resolve(&stable_resource_id) {
            lazy_check_side_effects()
          } else {
            DeterminedSideEffects::UserDefined(false)
          }
        }
      },
    };
    // TODO: Should we check if there are `check_side_effects_for` returns false but there are side effects in the module?
    let module = EcmaModule {
      source,
      idx: self.module_id,
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
      namespace_object_ref,
      def_format: self.module_type,
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
      side_effects,
      module_type,
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

  fn scan(
    &self,
    ast: &mut EcmaAst,
    source: &Arc<str>,
    symbols: SymbolTable,
    scopes: ScopeTree,
  ) -> (AstScopes, ScanResult, AstSymbols, SymbolRef) {
    let (mut ast_symbols, ast_scopes) = make_ast_scopes_and_symbols(symbols, scopes);
    let file_path: ResourceId = Arc::<str>::clone(&self.resolved_path.path).into();
    let repr_name = file_path.as_path().representative_file_name();
    let repr_name = legitimize_identifier_name(&repr_name);

    let scanner = AstScanner::new(
      self.module_id,
      &ast_scopes,
      &mut ast_symbols,
      repr_name.into_owned(),
      self.module_type,
      source,
      &file_path,
      &ast.trivias,
    );
    let namespace_object_ref = scanner.namespace_object_ref;
    let scan_result = scanner.scan(ast.program());

    (ast_scopes, scan_result, ast_symbols, namespace_object_ref)
  }

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
          module_type: ModuleDefFormat::Unknown,
          is_external: true,
          package_json: None,
          side_effects: None,
        }));
      }
    }

    // Check runtime module
    if specifier == ROLLDOWN_RUNTIME_RESOURCE_ID {
      return Ok(Ok(ResolvedRequestInfo {
        path: specifier.to_string().into(),
        module_type: ModuleDefFormat::EsmMjs,
        is_external: false,
        package_json: None,
        side_effects: None,
      }));
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
    dependencies: &IndexVec<ImportRecordIdx, RawImportRecord>,
    warnings: &mut Vec<BuildError>,
  ) -> Result<IndexVec<ImportRecordIdx, ResolvedRequestInfo>> {
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
      let (specifier, idx, resolved_id) = resolved_id?;

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
              module_type: ModuleDefFormat::Unknown,
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

    if build_errors.is_empty() {
      Ok(ret)
    } else {
      let resolved_err = anyhow::format_err!(
        "Unexpectedly failed to resolve dependencies of {importer}. Got errors {build_errors:#?}",
        importer = self.resolved_path.path,
      );
      Err(resolved_err)
    }
  }
}
