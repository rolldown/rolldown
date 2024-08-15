//! Items related to wrapper function. Related parameters:
//! - The `export_mode`: `named` or `default`;
//! - The `name`: whether includes a dot or not, and whether is a valid identifier or not;
//!    - If it is a namespaced name;
//!    - If it is a valid identifier;
//! - The `extend`: whether extends the object or not.
use crate::types::generator::GenerateContext;
use arcstr::ArcStr;
use rolldown_common::OutputExports;
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_utils::ecma_script::is_validate_assignee_identifier_name;

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
fn generate_namespace_definition(name: &str) -> (String, String) {
  let mut initial_code = String::new();
  let mut final_code = String::from("this");

  let context_len = final_code.len();
  let parts: Vec<&str> = name.split('.').collect();

  for (i, part) in parts.iter().enumerate() {
    let caller = generate_caller(part);
    final_code.push_str(&caller);

    if i < parts.len() - 1 {
      let callers = &final_code[context_len..];
      initial_code.push_str(&format!("this{callers} = this{callers} || {{}};\n"));
    }
  }

  (initial_code, final_code)
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
  ctx: &mut GenerateContext<'_>,
  export_mode: &OutputExports,
) -> DiagnosableResult<(String, String)> {
  if let Some(name) = &ctx.options.name {
    // It is same as Rollup.
    if name.contains('.') {
      let (decl, expr) = generate_namespace_definition(name);
      Ok((
        decl,
        // Extend the object if the `extend` option is enabled.
        if ctx.options.extend && matches!(export_mode, OutputExports::Named) {
          format!("{expr} = {expr} || {{}}")
        } else {
          expr
        },
      ))
    } else if ctx.options.extend {
      let caller = generate_caller(name.as_str());
      if matches!(export_mode, OutputExports::Named) {
        // In named exports, the `extend` option will make the assignment disappear and
        // the modification will be done extending the existed object (the `name` option).
        Ok((String::new(), format!("this{caller} = this{caller} || {{}}")))
      } else {
        Ok((
          String::new(),
          // If there isn't a name in default export, we shouldn't assign the function to `this[""]`.
          // If there is, we should assign the function to `this["name"]`,
          // because there isn't an object that we can extend.
          if name.is_empty() { String::new() } else { format!("this{caller}") },
        ))
      }
    } else if is_validate_assignee_identifier_name(name) {
      // If valid, we can use the `var` statement to declare the variable.
      Ok((String::new(), format!("var {name}")))
    } else {
      // This behavior is aligned with Rollup. If using `output.extend: true`, this error won't be triggered.
      let name = ArcStr::from(name);
      Err(vec![BuildDiagnostic::illegal_identifier_as_name(name)])
    }
  } else {
    // If the `name` is empty, you may be impossible to call the result.
    // But it is normal if we do not have exports.
    // However, if there is no export, it is recommended to use `app` format.
    ctx
      .warnings
      .push(BuildDiagnostic::missing_name_option_for_iife_export().with_severity_warning());
    Ok((String::new(), String::new()))
  }
}

/// It is a helper function to generate a caller for the given name.
///
/// - If the name is not a reserved word and not an invalid identifier, it will generate a caller like `.name`.
/// - Otherwise, it will generate a caller like `["if"]`.
fn generate_caller(name: &str) -> String {
  if is_validate_assignee_identifier_name(name) {
    format!(".{name}")
  } else {
    format!("[\"{name}\"]")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_generate_namespace_definition() {
    let result = generate_namespace_definition("a.b.c");
    assert_eq!(result.0, "this.a = this.a || {};\nthis.a.b = this.a.b || {};\n");
    assert_eq!(result.1, "this.a.b.c");
  }

  #[test]
  fn test_reserved_identifier_as_name() {
    let result = generate_namespace_definition("1.2.3");
    assert_eq!(
      result.0,
      "this[\"1\"] = this[\"1\"] || {};\nthis[\"1\"][\"2\"] = this[\"1\"][\"2\"] || {};\n"
    );
    assert_eq!(result.1, "this[\"1\"][\"2\"][\"3\"]");
  }

  #[test]
  /// It is related a bug in rollup. Check it out in [rollup/rollup#5603](https://github.com/rollup/rollup/issues/5603).
  fn test_invalid_identifier_as_name() {
    let result = generate_namespace_definition("toString.valueOf.constructor");
    assert_eq!(result.0, "this.toString = this.toString || {};\nthis.toString.valueOf = this.toString.valueOf || {};\n");
    assert_eq!(result.1, "this.toString.valueOf.constructor");
  }
}
