#[derive(ts_rs::TS, serde::Serialize)]
#[ts(export)]
pub struct ModuleTransformDone {
  pub r#type: &'static str,
  pub module_id: String,
  pub source: String,
  pub imports: Vec<String>,
  pub importers: Vec<String>,
}

impl ModuleTransformDone {
  pub fn new(
    module_id: String,
    source: String,
    imports: Vec<String>,
    importers: Vec<String>,
  ) -> Self {
    Self { r#type: "module_transform_done", module_id, source, imports, importers }
  }
}
