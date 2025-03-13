use heck::{ToLowerCamelCase, ToSnakeCase, ToTitleCase, ToUpperCamelCase};
use oxc::span::Span;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{File, parse_str};

use crate::{
  define_generator,
  output::{add_header, output_path, rust_output_path},
  utils::extract_toplevel_item_span,
};

use super::{Context, Generator, Runner};

/// For now auto generate fallback diagnostic
/// Because the limitation of `syn` crate, https://github.com/dtolnay/syn/issues/1745
/// We need to put the custom `Variant` related comments here
/// e.g.
/// ("CircularDependency", "Some description")
static VARIANT_RELATED_COMMENTS: [(&str, &str); 0] = [];

pub struct CheckOptionsGenerator {
  pub disabled_event: Vec<&'static str>,
}

define_generator!(CheckOptionsGenerator);

impl Generator for CheckOptionsGenerator {
  fn generate_many(&self, ctx: &Context) -> anyhow::Result<Vec<crate::output::Output>> {
    let event_kind_source_path = ctx.workspace_root.join("crates/rolldown_error/src/event_kind.rs");
    let event_kind_source = std::fs::read_to_string(&event_kind_source_path)?;
    let ast = parse_str::<File>(&event_kind_source)?;
    let variant_and_number_pairs = extract_event_kind_enum(&ast);

    // generate inline check options in validator.ts
    let validator_path = ctx.workspace_root.join("packages/rolldown/src/utils/validator.ts");
    let validator_source = std::fs::read_to_string(&validator_path)?;

    let (replaced_validator_code, span) = generate_validate_check_options(
      &variant_and_number_pairs,
      &validator_source,
      &validator_path.to_string_lossy(),
    );
    let (checks_binding_option_ts, checks_inner_option_ts) =
      generate_check_inner_options_and_binding(&variant_and_number_pairs, self);
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
          &generate_check_options(&variant_and_number_pairs, self),
          self.file_path(),
          "//",
        ),
      },
      crate::output::Output::EcmaStringInline {
        path: validator_path.to_string_lossy().to_string(),
        code: replaced_validator_code,
        span,
      },
      // Generate binding checks options
      crate::output::Output::Rust {
        path: rust_output_path("crates/rolldown_binding/", "binding_checks_options.rs"),
        tokens: checks_binding_option_ts,
      },
      // Generate rolldown_common checks options
      crate::output::Output::Rust {
        path: rust_output_path("crates/rolldown_common", "checks_options.rs"),
        tokens: checks_inner_option_ts,
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

/// `quote!` can not generate bitflags properly(The format is mess)
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
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct EventKindSwitcher: u{type_size} {{
    {}
  }}
}}
  ",
    fields.join("\n    "),
  )
}

fn generate_check_inner_options_and_binding(
  variant_and_number_pairs: &Vec<(String, usize)>,
  generator: &CheckOptionsGenerator,
) -> (TokenStream, TokenStream) {
  let mut struct_fields = vec![];
  let mut field_initializer_list = vec![];
  let mut event_kind_switcher_initializer = vec![];
  for (variant, _) in variant_and_number_pairs {
    if variant.ends_with("Error") {
      continue;
    }
    let snake_case = quote::format_ident!("{}", variant.to_snake_case());
    let ident = quote::format_ident!("{}", variant);
    let default_status = !generator.disabled_event.contains(&variant.as_str());
    struct_fields.push(quote! {
      pub #snake_case: Option<bool>,
    });
    field_initializer_list.push(quote! {
      #snake_case: value.#snake_case,
    });
    event_kind_switcher_initializer.push(quote! {
        flag.set(rolldown_error::EventKindSwitcher::#ident, value.#snake_case.unwrap_or(#default_status));
    });
  }
  let check_options_struct = quote! {
    #[napi_derive::napi(object)]
    #[derive(Debug, Default)]
    pub struct BindingChecksOptions {
      #(#struct_fields)*
    }
  };
  let check_options_impl = quote! {
    impl From<BindingChecksOptions> for rolldown_common::ChecksOptions {
      fn from(value: BindingChecksOptions) -> Self {
        Self {
          #(#field_initializer_list)*
        }
      }
    }
  };
  let binding_ts = quote! {
    #check_options_struct
    #check_options_impl
  };

  let inner_option_ts = quote! {
    #[cfg(feature = "deserialize_bundler_options")]
    use schemars::JsonSchema;
    #[cfg(feature = "deserialize_bundler_options")]
    use serde::Deserialize;

    #[derive(Default, Debug, Clone)]
    #[cfg_attr(
      feature = "deserialize_bundler_options",
      derive(Deserialize, JsonSchema),
      serde(rename_all = "camelCase", deny_unknown_fields)
    )]
    pub struct ChecksOptions {
      #(#struct_fields)*
    }
    impl From<ChecksOptions> for rolldown_error::EventKindSwitcher {
      fn from(value: ChecksOptions) -> Self {
        let mut flag = rolldown_error::EventKindSwitcher::all();
        #(#event_kind_switcher_initializer)*
        flag
      }
    }
  };

  (binding_ts, inner_option_ts)
}

fn generate_check_options(
  variant_and_number_pairs: &[(String, usize)],
  generator: &CheckOptionsGenerator,
) -> String {
  let mut fields = vec![];
  for (variant, _) in variant_and_number_pairs {
    if variant.ends_with("Error") {
      continue;
    }
    let camel_case = variant.to_lower_camel_case();
    let related_comments = VARIANT_RELATED_COMMENTS
      .iter()
      .find_map(|(name, comment_content)| {
        (name == &variant.as_str()).then_some((*comment_content).to_string())
      })
      .unwrap_or(format!(
        "Whether to emit warning when detecting {}",
        variant.to_title_case().to_lowercase()
      ));
    let default_value = !generator.disabled_event.contains(&variant.as_str());
    fields.push(format!(
      r"
    /**  
     * {related_comments}
     * @default {default_value}
     * */
    {camel_case}?: boolean",
    ));
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

/// (range_replaced_code, range)
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

    let camel_case = variant.to_lower_camel_case();
    let related_comments = VARIANT_RELATED_COMMENTS
      .iter()
      .find_map(|(name, comment_content)| {
        (name == &variant.as_str()).then_some((*comment_content).to_string())
      })
      .unwrap_or(format!(
        "Whether to emit warning when detecting {}",
        variant.to_title_case().to_lowercase()
      ));
    fields.push(format!(
      r"{camel_case}: v.pipe(
    v.optional(v.boolean()),
    v.description(
      '{related_comments}',
    ),
  ),",
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
