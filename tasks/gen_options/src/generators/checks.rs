use heck::{ToLowerCamelCase, ToUpperCamelCase};
use oxc::span::Span;
use syn::{File, parse_str};

use crate::{
  define_generator,
  output::{add_header, output_path, rust_output_path},
  utils::extract_toplevel_item_span,
};

use super::{Context, Generator, Runner};

pub struct CheckOptionsGenerator;

define_generator!(CheckOptionsGenerator);

impl Generator for CheckOptionsGenerator {
  fn generate_many(&self, ctx: &Context) -> anyhow::Result<Vec<crate::output::Output>> {
    let source_path = ctx.workspace_root.join("crates/rolldown_error/src/event_kind.rs");
    let source = std::fs::read_to_string(&source_path)?;
    let ast = parse_str::<File>(&source)?;
    let variant_and_number_pairs = extract_event_kind_enum(&ast);

    // inline replace validator.ts
    let validator_path = ctx.workspace_root.join("packages/rolldown/src/utils/validator.ts");
    let validator_source = std::fs::read_to_string(&validator_path)?;

    let (replaced_validator_code, span) = generate_validate_check_options(
      &variant_and_number_pairs,
      &validator_source,
      &validator_path.to_string_lossy(),
    );

    Ok(vec![
      crate::output::Output::RustString {
        path: rust_output_path("crates/rolldown_error", "event_kind_switcher.rs"),
        code: add_header(
          &generate_event_kind_switch_config(&variant_and_number_pairs),
          self.file_path(),
          "//",
        ),
      },
      crate::output::Output::EcmaString {
        path: output_path("packages/rolldown/src/options", "checks-options.ts"),
        code: add_header(
          &generate_check_options(&variant_and_number_pairs),
          self.file_path(),
          "//",
        ),
      },
      // TODO: should generate the option inline
      crate::output::Output::EcmaStringInline {
        path: validator_path.to_string_lossy().to_string(),
        code: replaced_validator_code,
        span,
      },
    ])
  }
}
/// Extract event *Variant* and *Number* pairs from the `EventKind` enum.
fn extract_event_kind_enum(ast: &File) -> Vec<(String, usize)> {
  let event_kind_enum = ast
    .items
    .iter()
    .find_map(|item| match item {
      syn::Item::Enum(e) => (e.ident == "EventKind").then_some(e),
      _ => None,
    })
    .unwrap();
  let mut ret = vec![];
  for variant in &event_kind_enum.variants {
    let name = variant.ident.to_string();
    let number = variant
      .discriminant
      .as_ref()
      .map(|(_, expr)| match expr {
        syn::Expr::Lit(lit) => match &lit.lit {
          syn::Lit::Int(int) => int.base10_parse::<usize>().unwrap(),
          _ => panic!("Unexpected discriminant type"),
        },
        _ => panic!("Unexpected discriminant type"),
      })
      .unwrap();
    ret.push((name, number));
  }
  ret
}

fn generate_event_kind_switch_config(variant_and_number_pairs: &Vec<(String, usize)>) -> String {
  let mut fields = vec![];
  let type_size = match variant_and_number_pairs.len() {
    0..=8 => 8,
    9..=16 => 16,
    17..=32 => 32,
    33..=64 => 64,
    65..=128 => 128,
    _ => panic!("Too many variants"),
  };
  for (variant, number) in variant_and_number_pairs {
    fields.push(format!("const {} = 1 << {};", variant.to_upper_camel_case(), number));
  }
  format!(
    r"
use bitflags::bitflags;
bitflags! {{
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub struct EventKindSwitcher: u{type_size} {{
    {}
  }}
}}
  ",
    fields.join("\n    "),
  )
}

fn generate_check_options(variant_and_number_pairs: &[(String, usize)]) -> String {
  let mut fields = vec![];
  for (variant, _) in variant_and_number_pairs {
    if variant.ends_with("Error") {
      continue;
    }
    fields.push(format!("{}?: boolean", variant.to_lower_camel_case()));
  }
  format!(
    r"
  export interface ChecksOptions {{
    {}
  }}
      ",
    fields.join("\n    ")
  )
}

/// (replaced_code, range)
fn generate_validate_check_options(
  variant_and_number_pairs: &[(String, usize)],
  source: &str,
  path: &str,
) -> (String, Span) {
  let mut fields = vec![];
  for (variant, _) in variant_and_number_pairs {
    if variant.ends_with("Error") {
      continue;
    }
    // TODO: add real descriptions
    // v.description
    fields.push(format!(
      r"{}: v.pipe(
    v.optional(v.boolean()),
  ),",
      variant.to_lower_camel_case()
    ));
  }
  let replaced_code = format!(
    r"
const ChecksOptionsSchema = v.strictObject({{
  {}
}})
      ",
    fields.join("\n    ")
  )
  .trim()
  .to_string();

  let span = extract_toplevel_item_span(source, path, "ChecksOptionsSchema").unwrap();
  (replaced_code, span)
}
