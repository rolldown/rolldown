use std::sync::Arc;

use index_vec::IndexVec;
use rolldown_common::{
  EntryPoint, EntryPointKind, ExternalModule, ImportKind, ImportRecordId, ModuleId, NormalModule,
  NormalModuleId,
};
use rolldown_error::BuildError;
use rolldown_fs::OsFileSystem;
use rolldown_oxc_utils::OxcProgram;
use rolldown_plugin::SharedPluginDriver;
use rustc_hash::{FxHashMap, FxHashSet};

use super::normal_module_task::NormalModuleTask;
use super::runtime_normal_module_task::RuntimeNormalModuleTask;
use super::task_result::NormalModuleTaskResult;
use super::Msg;
use crate::module_loader::module_task_context::ModuleTaskCommonData;
use crate::module_loader::runtime_normal_module_task::RuntimeNormalModuleTaskResult;
use crate::options::normalized_input_options::SharedNormalizedInputOptions;
use crate::runtime::RuntimeModuleBrief;
use crate::types::module_table::{ExternalModuleVec, ModuleTable};
use crate::types::resolved_request_info::ResolvedRequestInfo;
use crate::types::symbols::Symbols;

use crate::error::{BatchedErrors, BatchedResult};
use crate::SharedResolver;

pub struct IntermediateNormalModules {
  pub modules: IndexVec<NormalModuleId, Option<NormalModule>>,
  pub ast_table: IndexVec<NormalModuleId, Option<OxcProgram>>,
}

impl IntermediateNormalModules {
  pub fn new() -> Self {
    Self { modules: IndexVec::new(), ast_table: IndexVec::new() }
  }

  pub fn alloc_module_id(&mut self, symbols: &mut Symbols) -> NormalModuleId {
    let id = self.modules.push(None);
    self.ast_table.push(None);
    symbols.alloc_one();
    id
  }
}

pub struct ModuleLoader {
  input_options: SharedNormalizedInputOptions,
  common_data: ModuleTaskCommonData,
  rx: tokio::sync::mpsc::UnboundedReceiver<Msg>,
  visited: FxHashMap<Arc<str>, ModuleId>,
  runtime_id: Option<NormalModuleId>,
  remaining: u32,
  intermediate_normal_modules: IntermediateNormalModules,
  external_modules: ExternalModuleVec,
  symbols: Symbols,
}

pub struct ModuleLoaderOutput {
  // Stored all modules
  pub module_table: ModuleTable,
  pub ast_table: IndexVec<NormalModuleId, OxcProgram>,
  pub symbols: Symbols,
  // Entries that user defined + dynamic import entries
  pub entry_points: Vec<EntryPoint>,
  pub runtime: RuntimeModuleBrief,
  pub warnings: Vec<BuildError>,
}

impl ModuleLoader {
  pub fn new(
    input_options: SharedNormalizedInputOptions,
    plugin_driver: SharedPluginDriver,
    fs: OsFileSystem,
    resolver: SharedResolver,
  ) -> Self {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();

    let common_data = ModuleTaskCommonData {
      input_options: Arc::clone(&input_options),
      tx,
      resolver,
      fs,
      plugin_driver,
    };

    Self {
      common_data,
      rx,
      input_options,
      visited: FxHashMap::default(),
      runtime_id: None,
      remaining: 0,
      intermediate_normal_modules: IntermediateNormalModules::new(),
      external_modules: IndexVec::new(),
      symbols: Symbols::default(),
    }
  }

  fn try_spawn_new_task(&mut self, info: &ResolvedRequestInfo) -> ModuleId {
    match self.visited.entry(Arc::<str>::clone(&info.path.path)) {
      std::collections::hash_map::Entry::Occupied(visited) => *visited.get(),
      std::collections::hash_map::Entry::Vacant(not_visited) => {
        if info.is_external {
          let id = self.external_modules.len_idx();
          not_visited.insert(id.into());
          let ext = ExternalModule::new(id, info.path.path.to_string());
          self.external_modules.push(ext);
          id.into()
        } else {
          let id = self.intermediate_normal_modules.alloc_module_id(&mut self.symbols);
          not_visited.insert(id.into());
          self.remaining += 1;
          let module_path = info.path.clone();

          let task = NormalModuleTask::new(
            // safety: Data in `ModuleTaskContext` are alive as long as the `NormalModuleTask`, but rustc doesn't know that.
            unsafe { self.common_data.assume_static() },
            id,
            module_path,
            info.module_type,
          );
          tokio::spawn(async move { task.run().await });
          id.into()
        }
      }
    }
  }

  pub fn try_spawn_runtime_module_task(&mut self) -> NormalModuleId {
    *self.runtime_id.get_or_insert_with(|| {
      let id = self.intermediate_normal_modules.alloc_module_id(&mut self.symbols);
      self.remaining += 1;
      let task = RuntimeNormalModuleTask::new(id, self.common_data.tx.clone());
      tokio::spawn(async move { task.run() });
      id
    })
  }

  #[allow(clippy::too_many_lines)]
  pub async fn fetch_all_modules(
    mut self,
    user_defined_entries: Vec<(Option<String>, ResolvedRequestInfo)>,
  ) -> BatchedResult<ModuleLoaderOutput> {
    assert!(!self.input_options.input.is_empty(), "You must supply options.input to rolldown");

    let mut errors = BatchedErrors::default();
    let mut all_warnings: Vec<BuildError> = Vec::new();

    self
      .intermediate_normal_modules
      .modules
      .reserve(user_defined_entries.len() + 1 /* runtime */);
    self
      .intermediate_normal_modules
      .ast_table
      .reserve(user_defined_entries.len() + 1 /* runtime */);

    // Store the already consider as entry module
    let mut user_defined_entry_ids = {
      let mut tmp = FxHashSet::default();
      tmp.reserve(user_defined_entries.len());
      tmp
    };

    let mut entry_points = user_defined_entries
      .into_iter()
      .map(|(name, info)| EntryPoint {
        name,
        id: self.try_spawn_new_task(&info).expect_normal(),
        kind: EntryPointKind::UserDefined,
      })
      .inspect(|e| {
        user_defined_entry_ids.insert(e.id);
      })
      .collect::<Vec<_>>();

    let mut dynamic_import_entry_ids = FxHashSet::default();

    let mut runtime_brief: Option<RuntimeModuleBrief> = None;

    let mut panic_errors = vec![];

    while self.remaining > 0 {
      let Some(msg) = self.rx.recv().await else {
        break;
      };
      match msg {
        Msg::NormalModuleDone(task_result) => {
          let NormalModuleTaskResult {
            module_id,
            ast_symbol,
            resolved_deps,
            mut builder,
            raw_import_records,
            warnings,
            ast,
          } = task_result;
          all_warnings.extend(warnings);
          let import_records = raw_import_records
            .into_iter()
            .zip(resolved_deps)
            .map(|(raw_rec, info)| {
              let id = self.try_spawn_new_task(&info);
              // Dynamic imported module will be considered as an entry
              if let ModuleId::Normal(id) = id {
                if matches!(raw_rec.kind, ImportKind::DynamicImport)
                  && !user_defined_entry_ids.contains(&id)
                {
                  dynamic_import_entry_ids.insert(id);
                }
              }
              raw_rec.into_import_record(id)
            })
            .collect::<IndexVec<ImportRecordId, _>>();
          builder.import_records = Some(import_records);
          builder.is_user_defined_entry = Some(user_defined_entry_ids.contains(&module_id));
          self.intermediate_normal_modules.modules[module_id] = Some(builder.build());
          self.intermediate_normal_modules.ast_table[module_id] = Some(ast);

          self.symbols.add_ast_symbol(module_id, ast_symbol);
        }
        Msg::RuntimeNormalModuleDone(task_result) => {
          let RuntimeNormalModuleTaskResult { ast_symbol, builder, runtime, warnings: _, ast } =
            task_result;

          self.intermediate_normal_modules.modules[runtime.id()] = Some(builder.build());
          self.intermediate_normal_modules.ast_table[runtime.id()] = Some(ast);

          self.symbols.add_ast_symbol(runtime.id(), ast_symbol);
          runtime_brief = Some(runtime);
        }
        Msg::BuildErrors(errs) => {
          errors.extend(errs);
        }
        Msg::Panics(err) => {
          panic_errors.push(err);
        }
      }
      self.remaining -= 1;
    }

    assert!(panic_errors.is_empty(), "Panics occurred during module loading: {panic_errors:?}");

    if !errors.is_empty() {
      return Err(errors);
    }

    let modules: IndexVec<NormalModuleId, NormalModule> =
      self.intermediate_normal_modules.modules.into_iter().map(Option::unwrap).collect();

    let ast_table: IndexVec<NormalModuleId, OxcProgram> =
      self.intermediate_normal_modules.ast_table.into_iter().map(Option::unwrap).collect();

    let mut dynamic_import_entry_ids = dynamic_import_entry_ids.into_iter().collect::<Vec<_>>();
    dynamic_import_entry_ids.sort_by_key(|id| &modules[*id].resource_id);

    entry_points.extend(dynamic_import_entry_ids.into_iter().map(|id| EntryPoint {
      name: None,
      id,
      kind: EntryPointKind::DynamicImport,
    }));

    Ok(ModuleLoaderOutput {
      module_table: ModuleTable {
        normal_modules: modules,
        external_modules: self.external_modules,
      },
      symbols: self.symbols,
      ast_table,
      entry_points,
      runtime: runtime_brief.expect("Failed to find runtime module. This should not happen"),
      warnings: all_warnings,
    })
  }
}
