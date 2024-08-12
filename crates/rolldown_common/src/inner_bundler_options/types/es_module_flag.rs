#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// This configuration determines how the bundler should handle the synthetic `__esModule` property in the CJS and IIFE format.
/// It is rollup-capable, and the rollup default is `IfDefaultProp`.
/// You may find rollup explanation [here](https://rollupjs.org/configuration-options/#output-esmodule).
///
/// > Whether to add a `__esModule: true` property when generating exports for non-ES formats.
/// > This property signifies that the exported value is the namespace of an ES module and that the default
/// > export of this module corresponds to the `.default` property of the exported object.
/// >
/// > *From rollupjs.org*
#[derive(Debug, Default)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "kebab-case", deny_unknown_fields)
)]
pub enum EsModuleFlag {
  /// Always generate `Object.defineProperty(exports, "__esModule", { value: true });`
  /// in the CJS and IIFE format. It is similar to other bundlers' behavior.
  Always,
  /// Never generate the synthetic `__esModule` property in the CJS and IIFE format.
  Never,
  /// Generate the synthetic `__esModule` property in the CJS and IIFE format only
  /// if the module has a default export.
  ///
  /// > It will only add the property when using named exports mode and there also is a default export.
  /// > The subtle difference is that if there is no default export,
  /// > consumers of the CommonJS version of your library will get all named exports as
  /// > default export instead of an error or `undefined`.
  /// > We chose to make this the default value as the `__esModule` property is not a standard
  /// > followed by any JavaScript runtime and leads to many interop issues,
  /// > so we want to limit its use to the cases where it is really needed.
  /// >
  /// > *From rollupjs.org*
  ///
  /// For example, rolldown will define the `__esModule` property in the following entry code:
  ///
  /// ```js
  /// export default function() {}
  /// export const a = 1; // For this module, rolldown will automatically regard it as the `named` export mode.
  /// ```
  ///
  /// And rolldown won't generate the `__esModule` property in the following entry code:
  ///
  /// ```js
  /// export const a = 1;
  /// ```
  #[default]
  IfDefaultProp,
}

impl From<bool> for EsModuleFlag {
  fn from(value: bool) -> Self {
    if value {
      Self::Always
    } else {
      Self::Never
    }
  }
}

impl From<String> for EsModuleFlag {
  fn from(value: String) -> Self {
    if value == "if-default-prop" {
      Self::IfDefaultProp
    } else {
      unreachable!("unknown es module type")
    }
  }
}
