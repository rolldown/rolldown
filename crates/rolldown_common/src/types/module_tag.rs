#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

// See meta/design/module-tags.md

/// A `u64`-based bitset for module tags.
///
/// Unlike the general `BitSet` (heap-allocated `Vec<u8>`), this is `Copy` and
/// `contains_all` is a single AND + CMP instruction. Supports up to 64 tags.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct ModuleTagBitSet(u64);

impl ModuleTagBitSet {
  pub fn set_bit(&mut self, bit: u32) {
    self.0 |= 1u64 << bit;
  }

  pub fn has_bit(&self, bit: u32) -> bool {
    (self.0 & (1u64 << bit)) != 0
  }

  /// True if all bits in `required` are also set in `self`.
  pub fn contains_all(&self, required: &Self) -> bool {
    (self.0 & required.0) == required.0
  }

  pub fn is_empty(&self) -> bool {
    self.0 == 0
  }
}

/// Module tag enum. Built-in tags have fixed bit indices for zero-cost matching.
/// `Custom(String)` is reserved for future user-defined tags.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModuleTag {
  Initial,
  // Phase 2: Async,
  Custom(String),
}

impl ModuleTag {
  pub const INITIAL_BIT: u32 = 0;
  // Phase 2: pub const ASYNC_BIT: u32 = 1;
}

impl From<String> for ModuleTag {
  fn from(s: String) -> Self {
    match s.as_str() {
      "$initial" => ModuleTag::Initial,
      // Phase 2: "$async" => ModuleTag::Async,
      _ => ModuleTag::Custom(s),
    }
  }
}

#[cfg(feature = "deserialize_bundler_options")]
impl<'de> Deserialize<'de> for ModuleTag {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let s = String::deserialize(deserializer)?;
    Ok(ModuleTag::from(s))
  }
}

/// Registry for tag bit allocation. Built-in tags use fixed consts on `ModuleTag`.
/// Custom tag support will be added in a future phase.
pub struct ModuleTagRegistry {
  // Future: custom_tag_bits: FxHashMap<String, u32>,
}

impl ModuleTagRegistry {
  pub fn new() -> Self {
    Self {}
  }

  /// Compile tags into a `ModuleTagBitSet`. Unknown custom tags are silently ignored.
  pub fn compile_tags_to_bit_set(&self, tags: &[ModuleTag]) -> ModuleTagBitSet {
    let mut bits = ModuleTagBitSet::default();
    for tag in tags {
      match tag {
        ModuleTag::Initial => bits.set_bit(ModuleTag::INITIAL_BIT),
        // Phase 2: ModuleTag::Async => bits.set_bit(ModuleTag::ASYNC_BIT),
        ModuleTag::Custom(_name) => {
          // Custom tags not yet supported — silently ignored.
        }
      }
    }
    bits
  }
}
