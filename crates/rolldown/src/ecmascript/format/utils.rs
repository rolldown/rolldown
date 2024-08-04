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
  let parts: Vec<&str> = name.split('.').collect();

  let initialization_code = parts
    .iter()
    .enumerate()
    .scan(String::new(), |state, (i, part)| {
      // We use `scan` to generate the declaration sentence level-by-level.
      let callee = generate_callee(part);
      state.push_str(&callee);
      let line = if i < parts.len() - 1 {
        Some(format!("this{state} = this{state} || {{}};\n"))
      } else {
        None
      };
      Some(line)
    })
    .flatten()
    .collect::<String>();

  // TODO do not call the `generate_callee` function twice.
  let final_code =
    format!("this{}", parts.iter().map(|&part| generate_callee(part)).collect::<String>());

  (initialization_code, final_code)
}

/// This function generates a namespace definition for the given name, especially for IIFE format or UMD format.
/// If the name contains a dot, it will be regarded as a namespace definition.
/// Otherwise, it will be regarded as a variable definition.
pub fn generate_identifier(ctx: &mut GenerateContext<'_>) -> DiagnosableResult<(String, String)> {
  if let Some(name) = &ctx.options.name {
    // It is same as Rollup.
    if name.contains('.') {
      let (decl, expr) = generate_namespace_definition(name);
      Ok((
        decl,
        if ctx.options.extend && matches!(&ctx.options.exports, OutputExports::Named) {
          format!("{expr} = {expr} || {{}}")
        } else {
          expr
        },
      ))
    } else if ctx.options.extend {
      if matches!(ctx.options.exports, OutputExports::Named) {
        Ok((String::new(), format!("this{name} = this{name} || {{}}")))
      } else {
        Ok((String::new(), format!("this{name}")))
      }
    } else if is_validate_assignee_identifier_name(name) {
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

// It is a helper function to generate a callee for the given name.
fn generate_callee(name: &str) -> String {
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
