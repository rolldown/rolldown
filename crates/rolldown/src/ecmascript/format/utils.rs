use crate::types::generator::GenerateContext;
use arcstr::ArcStr;
use rolldown_error::{BuildDiagnostic, DiagnosableResult};
use rolldown_utils::ecma_script::is_validate_assignee_identifier_name;

fn generate_namespace_definition(name: &str) -> (String, String) {
  let parts: Vec<&str> = name.split('.').collect();

  let initialization_code = parts
    .iter()
    .enumerate()
    .scan(String::new(), |state, (i, part)| {
      let callee = generate_callee(part);
      state.push_str(&callee);
      let line = if i < parts.len() - 1 {
        Some(format!("this{0} = this{0} || {{}};\n", state))
      } else {
        None
      };
      Some(line)
    })
    .flatten()
    .collect::<String>();

  let final_code = format!("this{}", parts.iter().map(|&part| generate_callee(part)).collect::<String>());

  (initialization_code, final_code)
}

pub fn generate_identifier(ctx: &mut GenerateContext<'_>) -> DiagnosableResult<(String, String)> {
  if let Some(name) = &ctx.options.name {
    if name.contains('.') {
      Ok(generate_namespace_definition(name))
    } else {
      if is_validate_assignee_identifier_name(name) {
        Ok((String::new(), format!("var {name}")))
      } else {
        let name = ArcStr::from(name);
        Err(vec![BuildDiagnostic::illegal_identifier_as_name(name)])
      }
    }
  } else {
    ctx
      .warnings
      .push(BuildDiagnostic::missing_name_option_for_iife_export().with_severity_warning());
    Ok((String::new(), String::new()))
  }
}

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
    assert_eq!(result.0, "this[\"1\"] = this[\"1\"] || {};\nthis[\"1\"][\"2\"] = this[\"1\"][\"2\"] || {};\n");
    assert_eq!(result.1, "this[\"1\"][\"2\"][\"3\"]");
  }

  #[test]
  fn test_invalid_identifier_as_name() {
    let result = generate_namespace_definition("toString.valueOf.constructor");
    assert_eq!(result.0, "this.toString = this.toString || {};\nthis.toString.valueOf = this.toString.valueOf || {};\n");
    assert_eq!(result.1, "this.toString.valueOf.constructor");
  }
}
