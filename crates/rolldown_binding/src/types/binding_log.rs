#[napi_derive::napi(object, object_from_js = false)]
pub struct BindingLogLocation {
  /// 1-based
  pub line: u32,
  /// 0-based position in the line in UTF-16 code units
  pub column: u32,
  pub file: Option<String>,
}

impl From<rolldown_common::LogLocation> for BindingLogLocation {
  fn from(value: rolldown_common::LogLocation) -> Self {
    Self { line: value.line, column: value.column, file: value.file }
  }
}

#[napi_derive::napi(object, object_from_js = false)]
pub struct BindingLog {
  pub message: String,
  pub id: Option<String>,
  pub code: Option<String>,
  pub exporter: Option<String>,
  pub plugin: Option<String>,
  /// Location information (line, column, file)
  pub loc: Option<BindingLogLocation>,
  /// Position in the source file in UTF-16 code units
  pub pos: Option<u32>,
}

impl From<rolldown_common::Log> for BindingLog {
  fn from(value: rolldown_common::Log) -> Self {
    Self {
      code: value.code,
      message: value.message,
      id: value.id,
      exporter: value.exporter,
      plugin: value.plugin,
      loc: value.loc.map(Into::into),
      pos: value.pos,
    }
  }
}
