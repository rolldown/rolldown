# TODO: Complete Import Chain Implementation

This PR adds the infrastructure for displaying import chains in UNRESOLVED_IMPORT errors, but the actual chain building is not yet connected.

## What's Done
- Added `import_chain: Option<Vec<String>>` field to `DiagnosableResolveError`
- Updated `resolve_error` constructor to accept import chain parameter
- Implemented formatting logic to display import chain in help message
- Created `build_import_chain` helper function in `resolve_utils.rs`
- Created test case demonstrating expected behavior

## What's Not Done
The import chain is currently always `None`, so errors display as before. To complete this:

### Option 1: Post-process errors in module_loader
After errors are collected but before returning them (around line 637 in module_loader.rs):
```rust
// Augment UNRESOLVED_IMPORT errors with import chains
errors = errors.into_iter().map(|error| {
  if error.kind() == EventKind::UnresolvedImport {
    // Extract importer ID from error
    if let Some(importer_id) = error.id() {
      // Look up module index
      if let Some(module_idx) = /* lookup from module table */ {
        // Build import chain
        if let Some(chain) = self.build_import_chain(module_idx) {
          // Create new error with chain
          // Problem: Can't easily extract all fields from boxed error
        }
      }
    }
  }
  error
}).collect();
```

### Option 2: Pass importers through call chain
Pass importers data to `resolve_dependencies`:
- Add Arc<RwLock<IndexVec<ModuleIdx, Vec<ImporterRecord>>>> to TaskContext
- Update resolve_dependencies signature to accept importers
- Build chain when creating UNRESOLVED_IMPORT errors
- Issue: Importers may be incomplete at error creation time

### Option 3: Extend DiagnosticOptions
Add import chain lookup capability to DiagnosticOptions:
```rust
pub struct DiagnosticOptions {
  pub cwd: PathBuf,
  pub import_chain_lookup: Option<Box<dyn Fn(&str) -> Option<Vec<String>>>>,
}
```
Then compute chain in `on_diagnostic` method of `DiagnosableResolveError`.

## Recommendation
Option 1 or 3 seems most feasible. Option 1 requires solving the "can't modify boxed error" problem (perhaps by downcasting or providing accessor methods). Option 3 is cleaner architecturally but requires changes to more files.
