#![expect(dead_code)]

/// A thread-safe wrapper around [`napi::Error`] for cross-thread error sharing.
///
/// ## Problem
///
/// [`napi::Error`] contains raw pointers to the NAPI runtime that are not [`Send`] + [`Sync`].
/// In WASM worker scenarios, sharing these errors causes `napi_reference_unref` errors.
///
/// ## Solution
///
/// - **Non-WASM**: Stores the original [`napi::Error`] directly (zero-overhead)
/// - **WASM**: Extracts only thread-safe components (status + message)
#[derive(Debug)]
pub struct SafeNapiError(
  #[cfg(not(target_family = "wasm"))] napi::Error,
  #[cfg(target_family = "wasm")] SerializedNapiError,
);

/// Thread-safe representation of [`napi::Error`] for WASM platforms.
///
/// Stores only the status code and error message, discarding non-thread-safe runtime references.
#[derive(Debug, Clone)]
struct SerializedNapiError {
  status: napi::Status,
  reason: String,
  // Future: could add more fields here to preserve custom error properties
  // custom_properties: HashMap<String, serde_json::Value>,
}

impl SafeNapiError {
  /// Creates a new thread-safe wrapper around a [`napi::Error`].
  ///
  /// On WASM, extracts only the status and reason for thread safety.
  #[cfg(feature = "napi")]
  pub fn new(error: napi::Error) -> Self {
    #[cfg(not(target_family = "wasm"))]
    {
      Self(error)
    }
    #[cfg(target_family = "wasm")]
    {
      Self(SerializedNapiError { status: error.status, reason: error.reason.clone() })
    }
  }

  /// Converts back into a [`napi::Error`].
  ///
  /// On WASM, reconstructs the error from the serialized status and reason.
  #[cfg(feature = "napi")]
  #[expect(clippy::wrong_self_convention)]
  pub fn to_napi_error(self) -> napi::Error {
    #[cfg(not(target_family = "wasm"))]
    {
      self.0
    }
    #[cfg(target_family = "wasm")]
    {
      napi::Error::new(self.0.status, self.0.reason)
    }
  }

  /// Returns the NAPI status code.
  pub fn status(&self) -> napi::Status {
    #[cfg(not(target_family = "wasm"))]
    {
      self.0.status
    }
    #[cfg(target_family = "wasm")]
    {
      self.0.status
    }
  }

  /// Returns the error message.
  pub fn reason(&self) -> &str {
    #[cfg(not(target_family = "wasm"))]
    {
      self.0.reason.as_str()
    }
    #[cfg(target_family = "wasm")]
    {
      self.0.reason.as_str()
    }
  }
}

/// Enables automatic conversion from [`napi::Error`] using the `?` operator.
#[cfg(feature = "napi")]
impl From<napi::Error> for SafeNapiError {
  fn from(error: napi::Error) -> Self {
    Self::new(error)
  }
}

impl std::fmt::Display for SafeNapiError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.reason())
  }
}

impl std::error::Error for SafeNapiError {}
