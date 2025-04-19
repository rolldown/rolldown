#[derive(Debug, Default, Clone)]
pub struct CompilerAssumptions {
  pub ignore_function_length: Option<bool>,
  pub no_document_all: Option<bool>,
  pub object_rest_no_symbols: Option<bool>,
  pub pure_getters: Option<bool>,
  pub set_public_class_fields: Option<bool>,
}

impl From<CompilerAssumptions> for oxc::transformer::CompilerAssumptions {
  fn from(value: CompilerAssumptions) -> Self {
    let default = oxc::transformer::CompilerAssumptions::default();
    Self {
      ignore_function_length: value
        .ignore_function_length
        .unwrap_or(default.ignore_function_length),
      no_document_all: value.no_document_all.unwrap_or(default.no_document_all),
      object_rest_no_symbols: value
        .object_rest_no_symbols
        .unwrap_or(default.object_rest_no_symbols),
      pure_getters: value.pure_getters.unwrap_or(default.pure_getters),
      set_public_class_fields: value
        .set_public_class_fields
        .unwrap_or(default.set_public_class_fields),
      ..default
    }
  }
}
