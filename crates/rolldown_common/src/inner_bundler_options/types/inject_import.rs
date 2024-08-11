#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// # Usage
/// - `import { Promise } from 'es6-promise'` => `InjectImport::named("Promise", None,"es6-promise")`
/// - `import { Promise as P } from 'es6-promise'` => `InjectImport::named("Promise", Some("P"), "es6-promise")`
/// - `import $ from 'jquery'` => `InjectImport::named("default", Some("$"), "jquery")`
/// - `import $ from 'jquery'` => `InjectImport::default("$", "jquery")`
/// - `import * as fs from 'node:fs'` => `InjectImport::namespace("fs", "node:fs")`
///
/// ---
///
/// - `InjectImport::named("default", Some("Object.assign"), "es6-object-assign")`
/// - `InjectImport::default("Object.assign", "es6-object-assign")`
///
/// are special forms to inject shims to the following code:
/// ```js
/// console.log(Object.assign({ a: 1 }, { b: 2 }));
/// ```
///
/// will be, after the injection, transformed to:
///
/// ```js
/// import object_assign from "es6-object-assign";
/// console.log(object_assign({ a: 1 }, { b: 2 }));
///```
#[derive(Debug)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields, tag = "type")
)]
pub enum InjectImport {
  Named { imported: String, alias: Option<String>, from: String },
  Namespace { alias: String, from: String },
}

impl InjectImport {
  pub fn named(imported: String, alias: Option<String>, from: String) -> Self {
    Self::Named { imported, from, alias }
  }

  pub fn namespace(alias: String, from: String) -> Self {
    Self::Namespace { from, alias }
  }

  pub fn default(alias: String, from: String) -> Self {
    Self::Named { imported: "default".to_string(), alias: Some(alias), from }
  }
}
