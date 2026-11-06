#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct ModuleRenderedReady {
  #[ts(type = "'ModuleRenderedReady'")]
  pub action: &'static str,
  pub modules: Vec<ModuleRendered>,
}

/// Per-module rendered size, emitted after chunks are instantiated. `bytes` is the module's
/// rendered length summed across every chunk it was bundled into (a duplicated module ships
/// that many bytes in total), post tree-shaking and pre whole-chunk minification — the same
/// accounting `PackageGraphReady` uses for package sizes.
#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct ModuleRendered {
  pub id: String,
  pub bytes: u32,
}
