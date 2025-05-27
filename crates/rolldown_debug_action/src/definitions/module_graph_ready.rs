#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct ModuleGraphReady {
  #[ts(type = "'ModuleGraphReady'")]
  pub action: &'static str,
  pub modules: Vec<Module>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct Module {
  pub id: String,
  pub is_external: bool,
  pub imports: Option<Vec<ModuleImport>>,
  pub importers: Option<Vec<String>>,
}

#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct ModuleImport {
  pub id: String,
  // Refer to crates/rolldown_common/src/types/import_kind.rs
  /// - `import-statement`: `import { foo } from './lib.js';`
  /// - `dynamic-import`: `import('./lib.js')`
  /// - `require-call`: `require('./lib.js')`
  /// - `import-rule`: `@import 'bg-color.css'`
  /// - `url-token`: `url('./icon.png')`
  /// - `new-url`: `new URL('./worker.js', import.meta.url)`
  /// - `hot-accept`: `import.meta.hot.accept('./lib.js', () => {})`
  #[ts(
    type = "'import-statement' | 'dynamic-import' | 'require-call' | 'import-rule' | 'url-token' | 'new-url' | 'hot-accept'"
  )]
  pub kind: String,
  /// `./lib.js` in `import { foo } from './lib.js';`
  pub module_request: String,
}
