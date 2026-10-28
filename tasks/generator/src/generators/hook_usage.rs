use heck::{ToLowerCamelCase, ToUpperCamelCase};

use crate::{
  define_generator,
  output::{add_header, output_path, rust_output_path},
};

use super::{Context, Generator, Runner};

pub struct HookUsageGenerator;

define_generator!(HookUsageGenerator);

const HOOK_KIND: [&str; 23] = [
  "build_start",
  "resolve_id",
  "resolve_dynamic_import",
  "load",
  "transform",
  "module_parsed",
  "build_end",
  "render_start",
  "render_error",
  "render_chunk",
  "augment_chunk_hash",
  "generate_bundle",
  "write_bundle",
  "close_bundle",
  "watch_change",
  "close_watcher",
  "transform_ast",
  "banner",
  "footer",
  "intro",
  "outro",
  "resolve_file_url",
  "hot_update",
];

/// Hooks that JS plugins cannot register.
const DISABLE_JS_HOOK: [&str; 1] = ["transform_ast"];

impl Generator for HookUsageGenerator {
  fn generate_many(&self, _ctx: &Context) -> anyhow::Result<Vec<crate::output::Output>> {
    Ok(vec![
      crate::output::Output::EcmaString {
        path: output_path("packages/rolldown/src/plugin", "hook-usage.ts"),
        code: add_header(&generate_hook_usage_ts(), self.file_path(), "//"),
      },
      crate::output::Output::RustString {
        path: rust_output_path("crates/rolldown_plugin", "hook_usage.rs"),
        code: add_header(&generate_hook_usage_rs(), self.file_path(), "//"),
      },
    ])
  }
}

/// The order that assigns bit positions, shared by the Rust bitflags and the TS enum:
/// hooks JS plugins can register first, in `HOOK_KIND` order, then the `DISABLE_JS_HOOK` ones.
/// The TS enum is the prefix of this order, so its bits stay contiguous from `1 << 0`
/// and each one matches the Rust bit of the same hook.
fn hook_bit_order() -> Vec<&'static str> {
  let (js_hooks, rust_only_hooks): (Vec<_>, Vec<_>) =
    HOOK_KIND.iter().copied().partition(|kind| !DISABLE_JS_HOOK.contains(kind));
  assert_eq!(rust_only_hooks.len(), DISABLE_JS_HOOK.len(), "DISABLE_JS_HOOK lists an unknown hook");
  js_hooks.into_iter().chain(rust_only_hooks).collect()
}

fn generate_hook_usage_ts() -> String {
  let order = hook_bit_order();
  let js_hooks = &order[..HOOK_KIND.len() - DISABLE_JS_HOOK.len()];
  let hook_usage_kind_list = js_hooks
    .iter()
    .enumerate()
    .map(|(i, kind)| format!("  {} = 1 << {},", kind.to_lower_camel_case(), i))
    .collect::<Vec<_>>()
    .join("\n");

  let union_hook_usage_list = js_hooks
    .iter()
    .map(|kind| {
      format!(
        r"
      if (plugin.{}) {{
        hookUsage.union(HookUsageKind.{});

      }}
      ",
        kind.to_lower_camel_case(),
        kind.to_lower_camel_case()
      )
    })
    .collect::<Vec<_>>()
    .join("\n");
  format!(
    r"
   export enum HookUsageKind {{
    {hook_usage_kind_list}
   }};

  export class HookUsage {{
    private bitflag: bigint = BigInt(0);
  	constructor() {{}}

    union(kind: HookUsageKind): void {{
      this.bitflag |= BigInt(kind);
    }}

    // napi generate binding type `number` for `u32` in rust
    // this is only used for compatible with the behavior
    // Note: Number.MAX_SAFE_INTEGER (which is 2 ^53 - 1) so it is safe to convert bigint to number
    inner(): number {{
      return Number(this.bitflag)
    }}
  }}

import type {{ PluginWithInternalHooks }} from '../internal-hooks';
export function extractHookUsage(plugin: PluginWithInternalHooks): HookUsage {{
  let hookUsage = new HookUsage();
  {union_hook_usage_list}
  return hookUsage;
}}
  ",
  )
}

/// `quote!` can not generate bitflags properly(The format is mess)
fn generate_hook_usage_rs() -> String {
  let mut fields = vec![];
  let type_size = match HOOK_KIND.len() {
    0..=8 => 8,
    9..=16 => 16,
    17..=32 => 32,
    33..=64 => 64,
    65..=128 => 128,
    _ => panic!("Too many variants"),
  };
  for (i, item) in hook_bit_order().iter().enumerate() {
    fields.push(format!("const {} = 1 << {};", item.to_upper_camel_case(), i));
  }
  format!(
    r"
use bitflags::bitflags;
bitflags! {{
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct HookUsage: u{type_size} {{
    {}
  }}
}}
  ",
    fields.join("\n    "),
  )
}

#[cfg(test)]
mod tests {
  use heck::{ToLowerCamelCase, ToUpperCamelCase};

  use super::{DISABLE_JS_HOOK, HOOK_KIND, generate_hook_usage_rs, generate_hook_usage_ts};

  /// Reads `<name> = 1 << <bit><terminator>` lines out of generated code.
  fn bits(generated: &str, prefix: &str, terminator: char) -> Vec<(String, u32)> {
    generated
      .lines()
      .filter_map(|line| line.trim().strip_prefix(prefix)?.strip_suffix(terminator))
      .map(|entry| {
        let (name, bit) = entry.split_once(" = 1 << ").unwrap();
        (name.to_string(), bit.parse().unwrap())
      })
      .collect()
  }

  #[test]
  fn ts_enum_is_contiguous_and_matches_rust_bits() {
    let ts = bits(&generate_hook_usage_ts(), "", ',');
    let rust = bits(&generate_hook_usage_rs(), "const ", ';');
    assert_eq!(rust.len(), HOOK_KIND.len());
    assert_eq!(ts.len(), HOOK_KIND.len() - DISABLE_JS_HOOK.len());

    // No hook that JS plugins cannot register appears in the TS enum.
    for disabled in DISABLE_JS_HOOK {
      assert!(ts.iter().all(|(name, _)| *name != disabled.to_lower_camel_case()));
    }

    // TS bits run from `1 << 0` with no gap.
    for (i, (name, bit)) in ts.iter().enumerate() {
      assert_eq!(*bit as usize, i, "`{name}` leaves a gap in HookUsageKind");
    }

    // Every TS hook has the same bit as the Rust flag of the same hook.
    for (name, bit) in &ts {
      let rust_name = name.to_upper_camel_case();
      let rust_bit = rust.iter().find(|(n, _)| *n == rust_name).map(|(_, b)| *b);
      assert_eq!(rust_bit, Some(*bit), "`{name}` differs between TS and Rust");
    }

    // The hooks JS cannot register hold the highest Rust bits.
    let highest = &rust[ts.len()..];
    for disabled in DISABLE_JS_HOOK {
      assert!(highest.iter().any(|(name, _)| *name == disabled.to_upper_camel_case()));
    }
    assert!(highest.iter().all(|(_, bit)| *bit as usize >= ts.len()));
  }
}
