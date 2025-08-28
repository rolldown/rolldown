use arcstr::ArcStr;
use rolldown_common::{ImportRecordIdx, ModuleIdx};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::VecDeque;

/// Barrel file optimization manager
/// Tracks barrel files and manages their lazy loading with caching
#[derive(Debug, Default)]
pub struct BarrelOptimizer {
  /// Whether barrel optimization is enabled
  enabled: bool,
  /// Map of barrel modules to their state and metadata
  barrel_modules: FxHashMap<ModuleIdx, BarrelModuleState>,
  /// Cache of resolved export mappings for faster lookups
  export_resolution_cache: FxHashMap<(ModuleIdx, ArcStr), ExportResolution>,
  /// Track which modules have been resolved to get export info
  resolved_modules: FxHashMap<ModuleIdx, ResolvedModuleInfo>,
}

/// Information about a resolved module's exports
#[derive(Debug, Clone)]
struct ResolvedModuleInfo {
  /// All named exports from this module
  named_exports: FxHashSet<ArcStr>,
  /// Whether this module has a default export
  has_default: bool,
}

/// State of a barrel module's loading and analysis
#[derive(Debug, Clone)]
pub struct BarrelModuleState {
  /// All import records in this barrel module
  all_imports: FxHashSet<ImportRecordIdx>,
  /// Import records that are actually used (should be loaded)
  used_imports: FxHashSet<ImportRecordIdx>,
  /// Map export name to import record (for quick lookup)
  export_to_import: FxHashMap<ArcStr, ImportRecordIdx>,
  /// Export conflicts - when multiple imports provide the same export
  export_conflicts: FxHashMap<ArcStr, Vec<ImportRecordIdx>>,
  /// Star export import records
  star_exports: Vec<ImportRecordIdx>,
  /// Current load state of this barrel
  load_state: BarrelLoadState,
  /// Nested barrel modules detected within imports
  nested_barrels: FxHashSet<ModuleIdx>,
}

/// Loading state of a barrel module
#[derive(Debug, Clone)]
pub enum BarrelLoadState {
  /// Initial state, not analyzed yet
  Initial,
  /// Currently being analyzed (prevents infinite recursion)
  Analyzing,
  /// Partially loaded with specific exports
  PartiallyLoaded { loaded_exports: FxHashSet<ArcStr> },
  /// Fully loaded - all exports have been processed
  FullyLoaded,
}

/// Result of export resolution
#[derive(Debug, Clone)]
struct ExportResolution {
  /// The import record that provides this export
  import_record: ImportRecordIdx,
}

impl Default for BarrelModuleState {
  fn default() -> Self {
    Self {
      all_imports: FxHashSet::default(),
      used_imports: FxHashSet::default(),
      export_to_import: FxHashMap::default(),
      export_conflicts: FxHashMap::default(),
      star_exports: Vec::new(),
      load_state: BarrelLoadState::Initial,
      nested_barrels: FxHashSet::default(),
    }
  }
}

impl BarrelOptimizer {
  pub fn new(enabled: bool) -> Self {
    Self {
      enabled,
      barrel_modules: FxHashMap::default(),
      export_resolution_cache: FxHashMap::default(),
      resolved_modules: FxHashMap::default(),
    }
  }

  /// Register a barrel module
  pub fn register_barrel_module(&mut self, module_idx: ModuleIdx) {
    if !self.enabled {
      return;
    }
    self.barrel_modules.entry(module_idx).or_default();
  }

  /// Check if a module is a barrel module
  pub fn is_barrel_module(&self, module_idx: ModuleIdx) -> bool {
    self.barrel_modules.contains_key(&module_idx)
  }

  /// Add import record info to a barrel module with export tracking
  pub fn add_barrel_import(
    &mut self,
    barrel_idx: ModuleIdx,
    import_idx: ImportRecordIdx,
    export_names: Vec<ArcStr>,
    is_star: bool,
    imported_module_idx: Option<ModuleIdx>,
  ) {
    // Check if imported module is a barrel before getting mutable state
    let is_nested_barrel =
      imported_module_idx.map(|idx| self.is_barrel_module(idx)).unwrap_or(false);

    if let Some(state) = self.barrel_modules.get_mut(&barrel_idx) {
      state.all_imports.insert(import_idx);

      // Handle export name mapping based on import type
      if is_star {
        // Star exports need special handling
        state.star_exports.push(import_idx);

        // If we know the imported module's exports, add them all
        if let Some(module_idx) = imported_module_idx {
          if let Some(module_info) = self.resolved_modules.get(&module_idx) {
            for export_name in &module_info.named_exports {
              // Star exports make all exports available
              // Track conflicts when multiple imports provide same export
              match state.export_to_import.entry(export_name.clone()) {
                std::collections::hash_map::Entry::Occupied(_) => {
                  // Export conflict detected
                  state
                    .export_conflicts
                    .entry(export_name.clone())
                    .or_insert_with(Vec::new)
                    .push(import_idx);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                  entry.insert(import_idx);
                }
              }
            }
            if module_info.has_default {
              // Note: export * doesn't re-export default in ES modules
              // but we track it for completeness
            }
          }
        }
      } else if !export_names.is_empty() {
        // Explicit named exports provided
        for name in export_names {
          state.export_to_import.insert(name, import_idx);
        }
      } else if let Some(module_idx) = imported_module_idx {
        // Try to infer exports from resolved module info
        if let Some(module_info) = self.resolved_modules.get(&module_idx) {
          for export_name in &module_info.named_exports {
            state.export_to_import.entry(export_name.clone()).or_insert(import_idx);
          }
          if module_info.has_default {
            state.export_to_import.entry("default".into()).or_insert(import_idx);
          }
        }
      }

      // Track nested barrels
      if is_nested_barrel {
        if let Some(module_idx) = imported_module_idx {
          state.nested_barrels.insert(module_idx);
        }
      }
    }
  }

  /// Process imports from a barrel file with recursive handling
  pub fn process_barrel_imports(
    &mut self,
    barrel_idx: ModuleIdx,
    requested_names: &[ArcStr],
    is_namespace: bool,
    is_default: bool,
  ) -> Vec<ImportRecordIdx> {
    if !self.enabled {
      return vec![];
    }

    // Handle namespace imports - need everything
    if is_namespace {
      self.mark_all_imports_used_recursive(barrel_idx);
      return self.get_all_imports(barrel_idx);
    }

    // Handle default imports
    if is_default {
      // Try to find which import provides the default export
      let mut required_imports = FxHashSet::default();
      let mut visited = FxHashSet::default();

      // Look for "default" export specifically
      self.resolve_export_recursive(
        barrel_idx,
        "default".into(),
        &mut required_imports,
        &mut visited,
      );

      // If we found the default export, return only required imports
      if !required_imports.is_empty() {
        return required_imports.into_iter().collect();
      }

      // No default export found - barrel might not have one
      // Return empty to indicate no imports needed for non-existent default
      return vec![];
    }

    // Process specific named imports
    let mut required_imports = FxHashSet::default();
    let mut visited = FxHashSet::default();

    for name in requested_names {
      self.resolve_export_recursive(barrel_idx, name.clone(), &mut required_imports, &mut visited);
    }

    required_imports.into_iter().collect()
  }

  /// Recursively resolve an export through potentially nested barrels
  fn resolve_export_recursive(
    &mut self,
    barrel_idx: ModuleIdx,
    export_name: ArcStr,
    required_imports: &mut FxHashSet<ImportRecordIdx>,
    visited: &mut FxHashSet<(ModuleIdx, ArcStr)>,
  ) {
    // Prevent infinite recursion
    let key = (barrel_idx, export_name.clone());
    if !visited.insert(key.clone()) {
      return;
    }

    // Check cache first
    if let Some(resolution) = self.export_resolution_cache.get(&key).cloned() {
      required_imports.insert(resolution.import_record);

      // Mark as used
      if let Some(state) = self.barrel_modules.get_mut(&barrel_idx) {
        state.used_imports.insert(resolution.import_record);
        // Update load state without borrowing self
        match &mut state.load_state {
          BarrelLoadState::Initial | BarrelLoadState::Analyzing => {
            let mut loaded = FxHashSet::default();
            loaded.insert(export_name);
            state.load_state = BarrelLoadState::PartiallyLoaded { loaded_exports: loaded };
          }
          BarrelLoadState::PartiallyLoaded { loaded_exports } => {
            loaded_exports.insert(export_name);
          }
          BarrelLoadState::FullyLoaded => {}
        }
      }
      return;
    }

    // Extract needed info before mutable borrow
    let (star_exports, nested_barrels) = {
      let state = self.barrel_modules.get_mut(&barrel_idx);
      if state.is_none() {
        return;
      }
      let state = state.unwrap();

      // Mark state as analyzing to prevent loops
      if matches!(state.load_state, BarrelLoadState::Initial) {
        state.load_state = BarrelLoadState::Analyzing;
      }

      // Check direct exports
      if let Some(&import_idx) = state.export_to_import.get(&export_name) {
        required_imports.insert(import_idx);
        state.used_imports.insert(import_idx);

        // Update load state inline
        match &mut state.load_state {
          BarrelLoadState::Initial | BarrelLoadState::Analyzing => {
            let mut loaded = FxHashSet::default();
            loaded.insert(export_name.clone());
            state.load_state = BarrelLoadState::PartiallyLoaded { loaded_exports: loaded };
          }
          BarrelLoadState::PartiallyLoaded { loaded_exports } => {
            loaded_exports.insert(export_name.clone());
          }
          BarrelLoadState::FullyLoaded => {}
        }

        // Return early with found import
        self.export_resolution_cache.insert(key, ExportResolution { import_record: import_idx });
        return;
      }

      // Export needed info for processing outside the borrow
      (state.star_exports.clone(), state.nested_barrels.clone())
    };

    // Process star exports
    let mut found_in_star = false;
    if !star_exports.is_empty() {
      for &star_import_idx in &star_exports {
        required_imports.insert(star_import_idx);
      }

      // Update state with used imports
      if let Some(state) = self.barrel_modules.get_mut(&barrel_idx) {
        for &star_import_idx in &star_exports {
          state.used_imports.insert(star_import_idx);
        }
      }
      found_in_star = true;
    }

    // Handle nested barrels recursively
    for nested_barrel_idx in nested_barrels {
      // Check if nested barrel has this export
      let has_export = self
        .barrel_modules
        .get(&nested_barrel_idx)
        .map(|s| s.export_to_import.contains_key(&export_name) || !s.star_exports.is_empty())
        .unwrap_or(false);

      if has_export {
        self.resolve_export_recursive(
          nested_barrel_idx,
          export_name.clone(),
          required_imports,
          visited,
        );
        found_in_star = true;
      }
    }

    // Cache star export resolution
    if found_in_star && !star_exports.is_empty() {
      self.export_resolution_cache.insert(key, ExportResolution { import_record: star_exports[0] });
    }
  }

  /// Mark all imports as used recursively for namespace imports
  fn mark_all_imports_used_recursive(&mut self, barrel_idx: ModuleIdx) {
    let mut visited = FxHashSet::default();
    let mut queue = VecDeque::new();
    queue.push_back(barrel_idx);

    while let Some(current_idx) = queue.pop_front() {
      if !visited.insert(current_idx) {
        continue;
      }

      if let Some(state) = self.barrel_modules.get_mut(&current_idx) {
        // Mark all imports as used
        state.used_imports = state.all_imports.clone();
        state.load_state = BarrelLoadState::FullyLoaded;

        // Queue nested barrels for processing
        for &nested_idx in &state.nested_barrels {
          queue.push_back(nested_idx);
        }
      }
    }
  }

  /// Get all imports from a barrel module
  fn get_all_imports(&self, barrel_idx: ModuleIdx) -> Vec<ImportRecordIdx> {
    self
      .barrel_modules
      .get(&barrel_idx)
      .map(|state| state.all_imports.iter().copied().collect())
      .unwrap_or_default()
  }
}
