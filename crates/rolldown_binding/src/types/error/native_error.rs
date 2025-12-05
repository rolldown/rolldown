/// Location information for errors
#[napi_derive::napi(object, object_from_js = false)]
pub struct NativeErrorLocation {
  /// 1-based
  pub line: u32,
  /// 0-based position in the line in UTF-16 code units
  pub column: u32,
  pub file: Option<String>,
}

/// Error emitted from native side, it only contains kind and message, no stack trace.
// TODO: hyf0 do we want to rust stack trace?
#[napi_derive::napi(object, object_from_js = false)]
pub struct NativeError {
  pub kind: String,
  pub message: String,
  /// The id of the file associated with the error
  pub id: Option<String>,
  /// The exporter associated with the error (for import/export errors)
  pub exporter: Option<String>,
  /// Location information (line, column, file)
  pub loc: Option<NativeErrorLocation>,
  /// Position in the source file in UTF-16 code units
  pub pos: Option<u32>,
}
