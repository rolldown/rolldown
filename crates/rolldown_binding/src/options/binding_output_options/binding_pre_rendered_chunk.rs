#[napi_derive::napi(object, object_from_js = false)]
#[derive(Default, Debug)]
pub struct PreRenderedChunk {
  /// The name of this chunk, which is used in naming patterns.
  pub name: String,
  /// Whether this chunk is a static entry point.
  pub is_entry: bool,
  /// Whether this chunk is a dynamic entry point.
  pub is_dynamic_entry: bool,
  /// The id of a module that this chunk corresponds to.
  pub facade_module_id: Option<String>,
  /// The list of ids of modules included in this chunk.
  pub module_ids: Vec<String>,
  /// Exported variable names from this chunk.
  pub exports: Vec<String>,
}

impl From<rolldown_common::RollupPreRenderedChunk> for PreRenderedChunk {
  fn from(value: rolldown_common::RollupPreRenderedChunk) -> Self {
    Self {
      name: value.name.to_string(),
      is_entry: value.is_entry,
      is_dynamic_entry: value.is_dynamic_entry,
      facade_module_id: value.facade_module_id.map(|x| x.to_string()),
      module_ids: value.module_ids.iter().map(ToString::to_string).collect(),
      exports: value.exports.iter().map(ToString::to_string).collect(),
    }
  }
}
