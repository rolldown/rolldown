//! Items related to wrapper function. Related parameters:
//! - The `export_mode`: `named` or `default`;
//! - The `name`: whether includes a dot or not, and whether is a valid identifier or not;
//!    - If it is a namespaced name;
//!    - If it is a valid identifier;
//! - The `extend`: whether extends the object or not.
use std::fmt::Write as _;

use arcstr::ArcStr;
use rolldown_common::OutputExports;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_utils::{
  concat_string,
  ecmascript::{is_validate_assignee_identifier_name, is_validate_identifier_name},
};

use crate::types::generator::GenerateContext;

/// According to the amount of `.` in the name (levels),
/// it generates the initialization code and the final code.
///
/// # Example
///
/// for a IIFE named `namespace.module.hello`, it will generate:
///
/// - The initialization code:
///    ```js
///    this.namespace = this.namespace || {};
///    this.namespace.module = this.namespace.module || {};
///    ```
///  - The final code:
///    ```js
///    this.namespace.module.hello
///    ```
pub fn generate_namespace_definition(
  name: &str,
  global: &str,
  delimiter: &str,
) -> (String, String) {
  let parts: Vec<&str> = name.split('.').collect();
  let mut stmts = String::new();
  let mut namespace = String::from(global);
  let global_len = global.len();

  for (i, part) in parts.iter().enumerate() {
    let property = render_property_access(part);
    namespace.push_str(&property);

    if i < parts.len() - 1 {
      let property = &namespace[global_len..];
      write!(stmts, "{global}{property} = {global}{property} || {{}}{delimiter}").unwrap();
    }
  }

  (stmts, namespace)
}

/// This function generates a namespace definition for the given name, especially for IIFE format or UMD format.
/// If the name contains a dot, it will be regarded as a namespace definition.
/// Otherwise, it will be regarded as a variable definition.
///
/// - If you are using `extend: false` with a name, it will generate a variable definition (using `default` as an example):
///    ```js
///    var name = (function() { ... })();
///    ```
/// - If you are using `extend: true` with a name, it will generate an object definition (using `named` as an example):
///    ```js
///    (function(exports) { ... })(this.named = this.named || {});
///    ```
///
/// As for the namespaced name (including `.`), please refer to the `generate_namespace_definition` function.
pub fn generate_identifier(
  warnings: &mut Vec<BuildDiagnostic>,
  ctx: &GenerateContext<'_>,
  export_mode: OutputExports,
) -> BuildResult<(String, String)> {
  // Handle the diagnostic warning
  if ctx.options.name.as_ref().is_none_or(String::is_empty)
    && !matches!(export_mode, OutputExports::None)
  {
    warnings.push(BuildDiagnostic::missing_name_option_for_iife_export().with_severity_warning());
  }

  // Early return if `name` is None
  let Some(name) = &ctx.options.name else {
    return Ok((String::new(), String::new()));
  };

  // It is same as Rollup.
  if name.contains('.') {
    let (stmts, namespace) = generate_namespace_definition(name, "this", ";\n");
    // Extend the object if the `extend` option is enabled.
    let final_expr = if ctx.options.extend && matches!(export_mode, OutputExports::Named) {
      format!("{namespace} = {namespace} || {{}}")
    } else {
      namespace
    };

    return Ok((stmts, final_expr));
  }

  if ctx.options.extend {
    let property = render_property_access(name.as_str());
    let final_expr = if matches!(export_mode, OutputExports::Named) {
      // In named exports, the `extend` option will make the assignment disappear and
      // the modification will be done extending the existed object (the `name` option).
      format!("this{property} = this{property} || {{}}")
    } else {
      // If there isn't a name in default export, we shouldn't assign the function to `this[""]`.
      // If there is, we should assign the function to `this["name"]`,
      // because there isn't an object that we can extend.
      if name.is_empty() { String::new() } else { format!("this{property}") }
    };

    return Ok((String::new(), final_expr));
  }

  if is_validate_assignee_identifier_name(name) {
    // If valid, we can use the `var` statement to declare the variable.
    Ok((String::new(), format!("var {name}")))
  } else {
    // This behavior is aligned with Rollup. If using `output.extend: true`, this error won't be triggered.
    let name = ArcStr::from(name);
    Err(vec![BuildDiagnostic::illegal_identifier_as_name(name)].into())
  }
}

/// It is a helper function to generate a caller for the given name.
///
/// - If the name is not an invalid identifier, it will generate a caller like `.name`.
/// - Otherwise, it will generate a caller like `["-foo"]`.
pub fn render_property_access(name: &str) -> String {
  if is_validate_identifier_name(name) {
    concat_string!(".", name)
  } else {
    concat_string!("[\"", name, "\"]")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_generate_namespace_definition() {
    let result = generate_namespace_definition("a.b.c", "this", ";\n");
    assert_eq!(result.0, "this.a = this.a || {};\nthis.a.b = this.a.b || {};\n");
    assert_eq!(result.1, "this.a.b.c");
  }

  #[test]
  fn test_non_identifier_as_name() {
    let result = generate_namespace_definition("1.2.3", "this", ";\n");
    assert_eq!(
      result.0,
      "this[\"1\"] = this[\"1\"] || {};\nthis[\"1\"][\"2\"] = this[\"1\"][\"2\"] || {};\n"
    );
    assert_eq!(result.1, "this[\"1\"][\"2\"][\"3\"]");
  }

  #[test]
  fn test_reserved_identifier_as_name() {
    let result = generate_namespace_definition("if.else", "this", ";\n");
    assert_eq!(result.0, "this.if = this.if || {};\n");
    assert_eq!(result.1, "this.if.else");
  }

  #[test]
  /// It is related a bug in rollup. Check it out in [rollup/rollup#5603](https://github.com/rollup/rollup/issues/5603).
  fn test_invalid_identifier_as_name() {
    let result = generate_namespace_definition("toString.valueOf.constructor", "this", ";\n");
    assert_eq!(
      result.0,
      "this.toString = this.toString || {};\nthis.toString.valueOf = this.toString.valueOf || {};\n"
    );
    assert_eq!(result.1, "this.toString.valueOf.constructor");
  }
}
