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
  pub imports: Option<Vec<String>>,
  pub importers: Option<Vec<String>>,
}
