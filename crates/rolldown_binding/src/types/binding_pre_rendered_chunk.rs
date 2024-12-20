#[napi_derive::napi(object)]
#[derive(Default, Debug)]
pub struct PreRenderedChunk {
  pub name: String,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
}

impl From<rolldown_common::RollupPreRenderedChunk> for PreRenderedChunk {
  fn from(value: rolldown_common::RollupPreRenderedChunk) -> Self {
    Self {
      name: value.name.to_string(),
      is_entry: value.is_entry,
      is_dynamic_entry: value.is_dynamic_entry,
      facade_module_id: value.facade_module_id.map(|x| x.to_string()),
      module_ids: value.module_ids.iter().map(|x| x.to_string()).collect(),
      exports: value.exports.iter().map(ToString::to_string).collect(),
    }
  }
}
