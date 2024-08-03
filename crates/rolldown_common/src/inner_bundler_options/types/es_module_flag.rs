#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// This configuration determines how the bundler should handle the synthetic `__esModule` property in the CJS and IIFE format.
/// It is rollup-capable, and the rollup's default is `IfDefaultProp`.
/// You may find rollup's explanation [here](https://rollupjs.org/configuration-options/#output-esmodule).
#[derive(Debug, Default)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "kebab-case", deny_unknown_fields)
)]
pub enum EsModuleFlag {
  /// Always generate `Object.defineProperty(exports, "__esModule", { value: true });` in the CJS and IIFE format.
  Always,
  /// Never generate the synthetic `__esModule` property in the CJS and IIFE format.
  Never,
  /// Generate the synthetic `__esModule` property in the CJS and IIFE format only if the module has a default export.
  #[default]
  IfDefaultProp,
}

impl From<String> for EsModuleFlag {
  fn from(value: String) -> Self {
    match value.as_str() {
      "always" => EsModuleFlag::Always,
      "never" => EsModuleFlag::Never,
      "if-default-prop" => EsModuleFlag::IfDefaultProp,
      _ => unreachable!("unknown es module type"),
    }
  }
}
