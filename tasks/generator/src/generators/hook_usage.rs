use heck::{ToLowerCamelCase, ToUpperCamelCase};

use crate::{
  define_generator,
  output::{add_header, output_path, rust_output_path},
};

use super::{Context, Generator, Runner};

pub struct HookUsageGenerator {}

define_generator!(HookUsageGenerator);

const HOOK_KIND: [&str; 21] = [
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
];

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

fn generate_hook_usage_ts() -> String {
  let hook_usage_kind_list = HOOK_KIND
    .iter()
    .enumerate()
    .map(|(i, &kind)| format!("  {} = 1 << {},", kind.to_lower_camel_case(), i))
    .collect::<Vec<_>>()
    .join("\n");

  let union_hook_usage_list = HOOK_KIND
    .iter()
    .filter_map(|kind| {
      if DISABLE_JS_HOOK.contains(kind) {
        return None;
      }
      Some(format!(
        r"
      if (plugin.{}) {{
        hookUsage.union(HookUsageKind.{});
      
      }}
      ",
        kind.to_lower_camel_case(),
        kind.to_lower_camel_case()
      ))
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

import {{ Plugin }} from '../..';
export function extractHookUsage(plugin: Plugin): HookUsage {{
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
  for (i, item) in HOOK_KIND.iter().enumerate() {
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
