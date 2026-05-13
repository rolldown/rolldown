use heck::{ToLowerCamelCase, ToSnakeCase, ToTitleCase, ToUpperCamelCase};
use oxc::span::Span;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{File, parse_str};

use crate::{
  define_generator,
  output::{add_header, output_path, rust_output_path},
  utils::{extract_toplevel_item_span, syn_utils::extract_doc_comments},
};

use super::{Context, Generator, Runner};

#[derive(Debug)]
struct EventKindInfo {
  variant: String,
  index: usize,
  doc_comments: Option<String>,
}

pub struct CheckOptionsGenerator {
  pub disabled_event: Vec<&'static str>,
}

define_generator!(CheckOptionsGenerator);

impl Generator for CheckOptionsGenerator {
  fn generate_many(&self, ctx: &Context) -> anyhow::Result<Vec<crate::output::Output>> {
    let event_kind_source_path =
      ctx.workspace_root.join("crates/rolldown_error/src/types/event_kind.rs");
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
fn extract_event_kind_enum(ast: &File) -> Vec<EventKindInfo> {
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
    ret.push(EventKindInfo {
      variant: name,
      index: number,
      doc_comments: extract_doc_comments(&variant.attrs).map(|item| item.trim().to_string()),
    });
  }
  ret
}

/// `quote!` can not generate bitflags properly(The format is mess)
fn generate_event_kind_switch_config(variant_and_number_pairs: &Vec<EventKindInfo>) -> String {
  let mut fields = vec![];
  let type_size = match variant_and_number_pairs.len() {
    0..=8 => 8,
    9..=16 => 16,
    17..=32 => 32,
    33..=64 => 64,
    65..=128 => 128,
    _ => panic!("Too many variants"),
  };
  for EventKindInfo { variant, index: number, .. } in variant_and_number_pairs {
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
  variant_and_number_pairs: &Vec<EventKindInfo>,
  generator: &CheckOptionsGenerator,
) -> (TokenStream, TokenStream) {
  let mut binding_struct_fields = vec![];
  let mut inner_struct_fields = vec![];
  let mut field_initializer_list = vec![];
  let mut match_arms = vec![];
  for EventKindInfo { variant, .. } in variant_and_number_pairs {
    if variant.ends_with("Error") {
      continue;
    }
    let snake_case = quote::format_ident!("{}", variant.to_snake_case());
    let ident = quote::format_ident!("{}", variant);
    let default_status = !generator.disabled_event.contains(&variant.as_str());
    binding_struct_fields.push(quote! {
      #[napi(ts_type = "false | 'warn' | 'error'")]
      pub #snake_case: Option<napi::Either<bool, String>>,
    });
    inner_struct_fields.push(quote! {
      pub #snake_case: Option<crate::CheckSetting>,
    });
    field_initializer_list.push(quote! {
      #snake_case: value.#snake_case.map(crate::utils::checks_severity::either_to_check_setting),
    });
    // When the user didn't set a value, use the check's built-in default. For
    // most checks this means "emit a warning" (the bit is already set in `warn_checks`
    // from `all()`); a few (e.g. `circularDependency`) default to off and behave
    // identically to `Some(Off)`, so we merge the patterns.
    let none_and_off_arm = if default_status {
      quote! {
        None => {}
        Some(crate::CheckSetting::Off) => {
          warn_checks.remove(rolldown_error::EventKindSwitcher::#ident);
        }
      }
    } else {
      quote! {
        None | Some(crate::CheckSetting::Off) => {
          warn_checks.remove(rolldown_error::EventKindSwitcher::#ident);
        }
      }
    };
    match_arms.push(quote! {
      match value.#snake_case {
        #none_and_off_arm
        Some(crate::CheckSetting::Warn) => {
          warn_checks.insert(rolldown_error::EventKindSwitcher::#ident);
        }
        Some(crate::CheckSetting::Error) => {
          // Disjoint flags: an Error check fires only at error level, so clear
          // its warn bit and set its error bit.
          warn_checks.remove(rolldown_error::EventKindSwitcher::#ident);
          error_checks.insert(rolldown_error::EventKindSwitcher::#ident);
        }
      }
    });
  }
  let check_options_struct = quote! {
    #[napi_derive::napi(object, object_to_js = false)]
    #[derive(Debug, Default)]
    pub struct BindingChecksOptions {
      #(#binding_struct_fields)*
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
      #(#inner_struct_fields)*
    }
    /// Resolves the configured checks into two disjoint bitflags:
    /// - `warn_checks`: kinds whose emissions fire at warning severity.
    /// - `error_checks`: kinds whose emissions fire at hard-error severity.
    ///
    /// Per kind, at most one flag is set (a check is either off, warn, or error).
    /// `warn_checks` starts as `all()` so non-user-controllable kinds (errors, plugin
    /// warnings) remain visible to `filter_out_disabled_diagnostics`. User-controllable
    /// kinds are then explicitly placed in the right flag per the user's setting
    /// (or the check's built-in default).
    impl From<ChecksOptions> for (rolldown_error::EventKindSwitcher, rolldown_error::EventKindSwitcher) {
      #[expect(clippy::too_many_lines)]
      fn from(value: ChecksOptions) -> Self {
        let mut warn_checks = rolldown_error::EventKindSwitcher::all();
        let mut error_checks = rolldown_error::EventKindSwitcher::empty();
        #(#match_arms)*
        (warn_checks, error_checks)
      }
    }
  };

  (binding_ts, inner_option_ts)
}

fn generate_check_options(
  variant_and_number_pairs: &[EventKindInfo],
  generator: &CheckOptionsGenerator,
) -> String {
  let mut fields = vec![];
  for EventKindInfo { variant, doc_comments, .. } in variant_and_number_pairs {
    if variant.ends_with("Error") {
      continue;
    }
    let camel_case = variant.to_lower_camel_case();
    let related_comments = doc_comments
      .clone()
      .unwrap_or(format!(
        "Whether to emit warnings when detecting {}.",
        variant.to_title_case().to_lowercase()
      ))
      .replace('\n', "\n     *");
    let default_value =
      if generator.disabled_event.contains(&variant.as_str()) { "false" } else { "'warn'" };
    fields.push(format!(
      r"
    /**
     * {related_comments}
     *
     * - `false` disables the check.
     * - `'warn'` emits a warning (default when the check is enabled).
     * - `'error'` promotes the emission to a hard build error.
     * @default {default_value}
     * */
    {camel_case}?: false | 'warn' | 'error'",
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
  variant_and_number_pairs: &[EventKindInfo],
  source: &str,
  path: &str,
) -> (String, Span) {
  let mut fields = vec![];
  for EventKindInfo { variant, doc_comments, .. } in variant_and_number_pairs {
    if variant.ends_with("Error") {
      continue;
    }

    let camel_case = variant.to_lower_camel_case();
    let mut related_comments = doc_comments.clone().unwrap_or(format!(
      "Whether to emit warnings when detecting {}",
      variant.to_title_case().to_lowercase()
    ));
    if let Some(pos) = related_comments.find('\n') {
      related_comments.truncate(pos);
    }
    related_comments = related_comments.trim_end_matches('.').to_string();
    let quote_kind = '"';
    fields.push(format!(
      r"{camel_case}: v.pipe(
    v.optional(v.union([v.literal(false), v.picklist(['warn', 'error'])])),
    v.description(
      {quote_kind}{related_comments}{quote_kind},
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
