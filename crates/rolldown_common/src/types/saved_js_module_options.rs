use std::{
  borrow::Cow,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use arcstr::ArcStr;
use rolldown_utils::dashmap::FxDashMap;

use crate::{ImportAttributes, side_effects::HookSideEffects};

pub type SharedSavedJsModuleOptions = Arc<SavedJsModuleOptions>;

/// A value that a JavaScript hook set for a module. The JavaScript side keeps these values in
/// its `moduleOptionMap`, where `this.getModuleInfo` reads them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsModuleOption {
  /// One top-level key of a `meta` that a hook returned. `json` is the JSON text of its value.
  /// Rust does not parse it. `None` stands for the value `undefined`, which has no JSON text.
  Meta { key: String, json: Option<String> },
  /// A hook returned `moduleSideEffects`.
  SideEffects(HookSideEffects),
}

impl JsModuleOption {
  fn meta_key(&self) -> Option<&str> {
    match self {
      Self::Meta { key, .. } => Some(key),
      Self::SideEffects(_) => None,
    }
  }

  /// A module has one value for each `meta` key, and one `moduleSideEffects` value.
  fn replaces(&self, other: &Self) -> bool {
    self.meta_key() == other.meta_key()
  }
}

/// The newest `moduleSideEffects` value of a module from the saved results that the persistent
/// cache reused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSideEffects {
  /// The key of the module in `moduleOptionMap`.
  pub module_key: ArcStr,
  /// The JavaScript side compares it with the stamp that it has applied for the module.
  pub stamp: u64,
  pub value: HookSideEffects,
}

/// The values of one module that the JavaScript side has not applied yet.
#[derive(Debug)]
pub struct NewerJsModuleOptions {
  /// The stamp to pass as `after` the next time.
  pub stamp: u64,
  pub options: Vec<JsModuleOption>,
}

#[derive(Debug)]
struct StampedOption {
  stamp: u64,
  option: JsModuleOption,
}

#[derive(Debug, Default)]
struct ModuleOptions {
  /// The newest stamp in `options`.
  newest_stamp: u64,
  /// At most one value for each `meta` key and one `moduleSideEffects` value.
  ///
  /// The `meta` keys are in the order in which they were set for the first time. This is the
  /// order of the keys of a JavaScript object, and the JavaScript side writes the keys in this
  /// order. So `Object.keys(meta)` is the same as when the hooks run.
  options: Vec<StampedOption>,
}

/// Stamps grow over the whole process. So a stamp that the JavaScript side remembers is never
/// larger than the stamp of a value that is set later, also in another bundler.
static NEXT_STAMP: AtomicU64 = AtomicU64::new(1);

/// The JavaScript-side values of the modules whose saved result the persistent cache reused.
///
/// The hooks that set these values did not run, so the JavaScript side does not have them. Rust
/// does not call JavaScript for each reused module. Instead, the JavaScript side takes the values
/// from here when it reads a module, for example in `this.getModuleInfo`. A plugin can read a
/// module in any hook of any later build, so the values stay here as long as the bundler lives.
///
/// See internal-docs/persistent-cache/implementation.md.
#[derive(Debug)]
pub struct SavedJsModuleOptions {
  import_attributes: bool,
  /// The newest stamp in `modules`. `0` while `modules` is empty.
  last_stamp: AtomicU64,
  /// The newest stamp of a `moduleSideEffects` value. `0` while there is none. Most builds
  /// have none, so `side_effects` mostly returns without a look at `modules`.
  last_side_effects_stamp: AtomicU64,
  /// By the key of the module in `moduleOptionMap`.
  modules: FxDashMap<ArcStr, ModuleOptions>,
}

impl SavedJsModuleOptions {
  pub fn new(import_attributes: bool) -> Self {
    Self {
      import_attributes,
      last_stamp: AtomicU64::new(0),
      last_side_effects_stamp: AtomicU64::new(0),
      modules: FxDashMap::default(),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.last_stamp.load(Ordering::Relaxed) == 0
  }

  /// The key of the module `id` in `moduleOptionMap`. It must give the same result as
  /// `moduleOptionKey` in `packages/rolldown/src/plugin/plugin-context-data.ts`.
  pub fn key_of<'a>(&self, id: &'a str) -> &'a str {
    if self.import_attributes {
      id
    } else {
      ImportAttributes::parse_module_id(id).map_or(id, |(raw_id, _)| raw_id)
    }
  }

  /// The key in `moduleOptionMap` of the module that a resolve result names. The JavaScript
  /// side builds it from the same two values: `moduleOptionKey(toModuleId(rawId, attributes))`.
  pub fn key_of_resolved<'a>(
    &self,
    raw_id: &'a str,
    attributes: &ImportAttributes,
  ) -> Cow<'a, str> {
    if self.import_attributes && !attributes.is_empty() {
      Cow::Owned(attributes.compose_module_id(raw_id))
    } else {
      Cow::Borrowed(self.key_of(raw_id))
    }
  }

  /// A new value replaces the old value of the same `meta` key, or the old `moduleSideEffects`
  /// value, as on the JavaScript side.
  pub fn set(&self, key: ArcStr, option: JsModuleOption) {
    let is_side_effects = option.meta_key().is_none();
    let mut module = self.modules.entry(key).or_default();
    // Taken under the lock of the entry, so the stamps of one module grow in the order of the
    // calls.
    let stamp = NEXT_STAMP.fetch_add(1, Ordering::Relaxed);
    module.newest_stamp = stamp;
    let new = StampedOption { stamp, option };
    match module.options.iter().position(|existing| new.option.replaces(&existing.option)) {
      // A replaced `meta` key keeps its place, as in a JavaScript object.
      Some(index) => module.options[index] = new,
      None => module.options.push(new),
    }
    // Under the lock of the entry, so a reader that sees this stamp also sees the value.
    self.last_stamp.fetch_max(stamp, Ordering::Relaxed);
    if is_side_effects {
      self.last_side_effects_stamp.fetch_max(stamp, Ordering::Relaxed);
    }
  }

  /// The newest stamp of the values under `key`, or `None` when it has no values.
  pub fn last_stamp_of(&self, key: &str) -> Option<u64> {
    if self.is_empty() {
      return None;
    }
    self.modules.get(key).map(|module| module.newest_stamp)
  }

  /// The `moduleSideEffects` value of every module that has one.
  pub fn side_effects(&self) -> Vec<SavedSideEffects> {
    if self.last_side_effects_stamp.load(Ordering::Relaxed) == 0 {
      return Vec::new();
    }
    self
      .modules
      .iter()
      .filter_map(|module| {
        module.options.iter().find_map(|option| match option.option {
          JsModuleOption::SideEffects(value) => {
            Some(SavedSideEffects { module_key: module.key().clone(), stamp: option.stamp, value })
          }
          JsModuleOption::Meta { .. } => None,
        })
      })
      .collect()
  }

  /// The values under `key` that were set after the stamp `after`, in the order of
  /// `ModuleOptions::options`.
  pub fn newer_than(&self, key: &str, after: u64) -> Option<NewerJsModuleOptions> {
    if self.last_stamp.load(Ordering::Relaxed) <= after {
      return None;
    }
    let module = self.modules.get(key)?;
    if module.newest_stamp <= after {
      return None;
    }
    let options = module
      .options
      .iter()
      .filter(|option| option.stamp > after)
      .map(|option| option.option.clone())
      .collect();
    Some(NewerJsModuleOptions { stamp: module.newest_stamp, options })
  }
}

#[cfg(test)]
mod tests {
  use super::{JsModuleOption, SavedJsModuleOptions};
  use crate::side_effects::HookSideEffects;

  fn meta(key: &str, json: &str) -> JsModuleOption {
    JsModuleOption::Meta { key: key.to_string(), json: Some(json.to_string()) }
  }

  fn cleared(key: &str) -> JsModuleOption {
    JsModuleOption::Meta { key: key.to_string(), json: None }
  }

  #[test]
  fn gives_only_newer_values_and_replaces_by_key() {
    let saved = SavedJsModuleOptions::new(false);
    assert!(saved.is_empty());
    assert!(saved.newer_than("/a.js", 0).is_none());

    saved.set("/a.js".into(), meta("icons", "[1]"));
    saved.set("/a.js".into(), JsModuleOption::SideEffects(HookSideEffects::False));
    let first = saved.newer_than("/a.js", 0).unwrap();
    assert_eq!(
      first.options,
      vec![meta("icons", "[1]"), JsModuleOption::SideEffects(HookSideEffects::False)]
    );
    assert_eq!(saved.last_stamp_of("/a.js"), Some(first.stamp));
    assert!(saved.newer_than("/a.js", first.stamp).is_none());
    assert!(saved.newer_than("/b.js", 0).is_none());

    saved.set("/a.js".into(), meta("icons", "[2]"));
    saved.set("/a.js".into(), meta("other", "1"));
    let second = saved.newer_than("/a.js", first.stamp).unwrap();
    assert_eq!(second.options, vec![meta("icons", "[2]"), meta("other", "1")]);
    assert_eq!(
      saved.newer_than("/a.js", 0).unwrap().options,
      vec![
        meta("icons", "[2]"),
        JsModuleOption::SideEffects(HookSideEffects::False),
        meta("other", "1")
      ]
    );
  }

  #[test]
  fn replaced_meta_key_keeps_its_place() {
    let saved = SavedJsModuleOptions::new(false);
    saved.set("/a.js".into(), meta("shared", r#""first""#));
    saved.set("/a.js".into(), meta("other", "1"));
    let applied = saved.newer_than("/a.js", 0).unwrap().stamp;

    saved.set("/a.js".into(), meta("shared", r#""second""#));
    saved.set("/a.js".into(), meta("last", "2"));
    saved.set("/a.js".into(), cleared("other"));

    let all = saved.newer_than("/a.js", 0).unwrap();
    assert_eq!(
      all.options,
      vec![meta("shared", r#""second""#), cleared("other"), meta("last", "2"),]
    );
    assert_eq!(saved.last_stamp_of("/a.js"), Some(all.stamp));

    let newer = saved.newer_than("/a.js", applied).unwrap();
    assert_eq!(newer.options, all.options);
    assert_eq!(newer.stamp, all.stamp);
    assert!(saved.newer_than("/a.js", all.stamp).is_none());
  }

  #[test]
  fn side_effects_gives_the_newest_value_of_each_module() {
    let saved = SavedJsModuleOptions::new(false);
    assert_eq!(saved.side_effects(), vec![]);

    saved.set("/b.js".into(), meta("icons", "[]"));
    assert_eq!(saved.side_effects(), vec![]);

    saved.set("/a.js".into(), JsModuleOption::SideEffects(HookSideEffects::False));
    let first = saved.side_effects();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].module_key.as_str(), "/a.js");
    assert_eq!(first[0].value, HookSideEffects::False);
    assert_eq!(saved.last_stamp_of("/a.js"), Some(first[0].stamp));

    saved.set("/a.js".into(), JsModuleOption::SideEffects(HookSideEffects::NoTreeshake));
    let replaced = saved.side_effects();
    assert_eq!(replaced.len(), 1);
    assert_eq!(replaced[0].value, HookSideEffects::NoTreeshake);
    assert!(replaced[0].stamp > first[0].stamp);
    assert_eq!(
      saved.newer_than("/a.js", 0).unwrap().options,
      vec![JsModuleOption::SideEffects(HookSideEffects::NoTreeshake)]
    );
  }

  #[test]
  fn key_drops_attributes_only_without_import_attributes() {
    let id = "/a.json?attributes=%7B%22type%22%3A%22json%22%7D";
    assert_eq!(SavedJsModuleOptions::new(false).key_of(id), "/a.json");
    assert_eq!(SavedJsModuleOptions::new(true).key_of(id), id);
    assert_eq!(SavedJsModuleOptions::new(false).key_of("/a.js?raw"), "/a.js?raw");
  }
}
