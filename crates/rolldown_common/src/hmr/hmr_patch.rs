/// The answer to a client's pull. One pull serves an update, a lazy `import()` and a
/// `loadDynamic`.
#[derive(Debug, Clone)]
pub struct HmrPatch {
  /// Ends with an inline `sourceMappingURL` when `sourcemap` is `Inline`. Has no
  /// `sourceMappingURL` comment otherwise.
  pub code: String,
  /// The map as JSON when `sourcemap` is `File` or `Hidden`. No file is kept for a pulled
  /// payload, so the consumer serves the map itself.
  pub sourcemap: Option<String>,
}

/// The server answers a client request with a payload, or tells the client to reload: it
/// belongs to another engine, its generation is too old to catch up, or the request is bad.
#[derive(Debug, Clone)]
pub enum HmrPullResponse<T> {
  Payload(T),
  Reload { reason: String },
}
