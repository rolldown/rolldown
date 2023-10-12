use rolldown_error::Error as BuildError;

pub(crate) trait NapiErrorExt {
  fn into_bundle_error(self) -> BuildError;
}

impl NapiErrorExt for napi::Error {
  fn into_bundle_error(self) -> BuildError {
    BuildError::napi_error(self.status.to_string(), self.reason.clone())
  }
}
