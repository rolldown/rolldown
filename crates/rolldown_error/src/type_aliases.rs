use crate::BuildDiagnostic;

pub type DiagnosableResult<T> = Result<T, Vec<BuildDiagnostic>>;

/// This is used for returning errors that are not expected to be handled by rolldown. Such as
/// - Error of converting u64 to usize in a platform that usize is 32-bit.
/// - ...
///   Handling such errors is meaningless.
///
/// Notice:
/// - We might mark some errors as unhandleable for faster development, but we should convert them
///   to `BuildDiagnostic` to provide better error messages to users.
pub type UnhandleableResult<T> = anyhow::Result<T>;
