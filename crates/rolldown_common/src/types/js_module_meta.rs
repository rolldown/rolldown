/// The `meta` of JS hook results, in the form that the persistent cache saves: one entry for each
/// top-level key, in the key order of the merged object.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JsModuleMeta {
  entries: Vec<JsModuleMetaEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsModuleMetaEntry {
  pub key: String,
  /// The JSON text of the value. `None` stands for `undefined`, which JSON cannot hold.
  pub json: Option<String>,
}

impl JsModuleMeta {
  pub fn new(entries: Vec<JsModuleMetaEntry>) -> Self {
    let mut meta = Self::default();
    meta.merge(entries);
    meta
  }

  pub fn entries(&self) -> &[JsModuleMetaEntry] {
    &self.entries
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// Merges a later hook result as `Object.assign` does: a replaced key keeps its place, and a
  /// new key goes to the end.
  pub fn merge(&mut self, entries: impl IntoIterator<Item = JsModuleMetaEntry>) {
    for entry in entries {
      match self.entries.iter_mut().find(|existing| existing.key == entry.key) {
        Some(existing) => existing.json = entry.json,
        None => self.entries.push(entry),
      }
    }
  }

  /// The JSON text that JS reads back: `[["key", value], ["key without a value"]]`.
  pub fn to_entries_json(&self) -> String {
    let mut text = String::from("[");
    for (i, entry) in self.entries.iter().enumerate() {
      if i > 0 {
        text.push(',');
      }
      text.push('[');
      text.push_str(&serde_json::Value::String(entry.key.clone()).to_string());
      if let Some(json) = &entry.json {
        text.push(',');
        text.push_str(json);
      }
      text.push(']');
    }
    text.push(']');
    text
  }
}

#[cfg(test)]
mod tests {
  use super::{JsModuleMeta, JsModuleMetaEntry};

  fn entry(key: &str, json: Option<&str>) -> JsModuleMetaEntry {
    JsModuleMetaEntry { key: key.to_string(), json: json.map(ToString::to_string) }
  }

  #[test]
  fn merges_like_object_assign() {
    let mut meta = JsModuleMeta::new(vec![entry("shared", Some("1")), entry("other", Some("2"))]);
    meta.merge([entry("last", Some("3")), entry("shared", None)]);
    assert_eq!(
      meta.entries(),
      [entry("shared", None), entry("other", Some("2")), entry("last", Some("3"))]
    );
  }

  #[test]
  fn writes_the_entries_as_json() {
    let meta = JsModuleMeta::new(vec![entry("a\"b", Some(r#"{"x":[1]}"#)), entry("gone", None)]);
    assert_eq!(meta.to_entries_json(), r#"[["a\"b",{"x":[1]}],["gone"]]"#);
    assert_eq!(JsModuleMeta::default().to_entries_json(), "[]");
  }
}
