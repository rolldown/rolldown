#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// Controls whether `"use strict"` is emitted in the output.
///
/// See [`rollupjs.org/configuration-options/#output-strict`](https://rollupjs.org/configuration-options/#output-strict).
#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "kebab-case", deny_unknown_fields)
)]
pub enum StrictMode {
  /// Respect the `"use strict"` directives from the source code.
  /// This is the default behavior.
  #[default]
  Auto,
  /// Always emit `"use strict"` at the top of the output (not applicable for ESM format).
  Always,
  /// Never emit `"use strict"` in the output.
  Never,
}

impl From<bool> for StrictMode {
  fn from(value: bool) -> Self {
    if value { Self::Always } else { Self::Never }
  }
}

impl From<String> for StrictMode {
  fn from(value: String) -> Self {
    if value == "auto" { Self::Auto } else { unreachable!("unknown strict mode: {value}") }
  }
}
