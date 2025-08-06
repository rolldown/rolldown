use heck::ToUpperCamelCase;

use crate::{
  define_generator,
  output::{add_header, output_path},
  utils::extract_toplevel_bindings_name,
};

use super::{Context, Generator, Runner};

pub struct RuntimeHelperGenerator {}

define_generator!(RuntimeHelperGenerator);

const RUNTIME_PATH_LIST: [&str; 2] = [
  "crates/rolldown/src/runtime/runtime-base.js",
  "crates/rolldown/src/runtime/runtime-tail-node.js",
];

impl Generator for RuntimeHelperGenerator {
  fn generate_many(&self, _ctx: &Context) -> anyhow::Result<Vec<crate::output::Output>> {
    Ok(vec![crate::output::Output::RustString {
      path: output_path("crates/rolldown_common/src", "runtime_helper.rs"),
      code: add_header(&generate_hook_usage_rs(), self.file_path(), "//"),
    }])
  }
}

/// `quote!` can not generate bitflags properly(The format is mess)
fn generate_hook_usage_rs() -> String {
  let root_dir = rolldown_workspace::root_dir();
  let mut top_level_items: Vec<String> = vec![];
  for p in RUNTIME_PATH_LIST {
    let runtime_path = root_dir.join(p);
    let runtime_source =
      std::fs::read_to_string(&runtime_path).expect("Failed to read runtime source file");
    top_level_items.extend(extract_toplevel_bindings_name(&runtime_source, p));
  }
  let mut fields = vec![];
  let type_size = match top_level_items.len() {
    0..=8 => 8,
    9..=16 => 16,
    17..=32 => 32,
    33..=64 => 64,
    65..=128 => 128,
    _ => panic!("Too many variants"),
  };
  for (i, item) in top_level_items.iter().enumerate() {
    fields.push(format!("const {} = 1 << {};", item.to_upper_camel_case(), i));
  }
  let runtime_helper_flag = format!(
    r"
  use bitflags::bitflags;
  bitflags! {{
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct RuntimeHelper: u{type_size} {{
      {}
    }}
  }}
    ",
    fields.join("\n      "),
  );

  let runtime_helper_names =
    top_level_items.iter().map(|item| format!("\"{item}\"")).collect::<Vec<_>>().join(", ");
  let runtime_helper_names_code = format!(
    r"
pub const RUNTIME_HELPER_NAMES: [&str; {}] = [
  {runtime_helper_names}
];
  ",
    top_level_items.len()
  );

  let runtime_helper_impl = r"
impl RuntimeHelper {
  /// # Use with caution
  /// Only used when there is only one bit is set in the `RuntimeHelper`.
  /// The function is used to get the index of the bit that is set.
  #[inline]
  pub fn bit_index(&self) -> usize {
    self.bits().trailing_zeros() as usize
  }
}
  ";

  format!("{runtime_helper_flag}\n{runtime_helper_impl}\n{runtime_helper_names_code}")
}
