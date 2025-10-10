use rolldown_utils::replace_all_placeholder::{ReplaceAllPlaceholder, Replacer};

#[derive(Debug)]
pub struct FilenameTemplate {
  template: String,
}

impl FilenameTemplate {
  pub fn new(template: String) -> Self {
    Self { template }
  }

  pub fn template(&self) -> &str {
    &self.template
  }
}

impl From<String> for FilenameTemplate {
  fn from(template: String) -> Self {
    Self::new(template)
  }
}

impl FilenameTemplate {
  pub fn render(
    self,
    name: Option<&str>,
    format: Option<&str>,
    extension: Option<&str>,
    hash_replacer: Option<impl Replacer>,
  ) -> String {
    let mut tmp = self.template;
    if let Some(name) = name {
      tmp = tmp.replace_all("[name]", name);
    }
    if let Some(format) = format {
      tmp = tmp.replace_all("[format]", format);
    }
    if let Some(hash_replacer) = hash_replacer {
      tmp = tmp.replace_all_with_len("[hash]", hash_replacer);
    }
    if let Some(ext) = extension {
      let extname = if ext.is_empty() { "" } else { &format!(".{ext}") };
      tmp = tmp.replace_all("[ext]", ext);
      tmp = tmp.replace_all("[extname]", extname);
    }
    tmp
  }

  pub fn has_hash_pattern(&self) -> bool {
    let start = self.template.find("[hash");
    start.is_some_and(|start| {
      let pattern = &self.template[start + 5..];
      pattern.starts_with(']') || (pattern.starts_with(':') && pattern.contains(']'))
    })
  }
}

#[test]
fn basic() {
  FilenameTemplate::new("[name]-[hash:8].js".to_string());
}

#[test]
fn hash_with_len() {
  let filename_template = FilenameTemplate::new("[name]-[hash:3]-[hash:3].js".to_string());

  let mut hash_iter = ["abc", "def"].iter();
  let hash_replacer = filename_template.has_hash_pattern().then_some(|_| hash_iter.next().unwrap());

  let filename = filename_template.render(Some("hello"), None, None, hash_replacer);

  assert_eq!(filename, "hello-abc-def.js");
}

#[test]
fn format_placeholder() {
  let filename = FilenameTemplate::new("[name]-[format].js".to_string()).render(
    Some("entry"),
    Some("esm"),
    None,
    Option::<fn(Option<usize>) -> String>::None,
  );

  assert_eq!(filename, "entry-esm.js");
}

#[test]
fn format_placeholder_cjs() {
  let filename = FilenameTemplate::new("[name]-[format].js".to_string()).render(
    Some("entry"),
    Some("cjs"),
    None,
    Option::<fn(Option<usize>) -> String>::None,
  );

  assert_eq!(filename, "entry-cjs.js");
}

#[test]
fn format_placeholder_iife() {
  let filename = FilenameTemplate::new("[name].[format].js".to_string()).render(
    Some("bundle"),
    Some("iife"),
    None,
    Option::<fn(Option<usize>) -> String>::None,
  );

  assert_eq!(filename, "bundle.iife.js");
}

#[test]
fn format_placeholder_umd() {
  let filename = FilenameTemplate::new("[format]/[name].js".to_string()).render(
    Some("main"),
    Some("umd"),
    None,
    Option::<fn(Option<usize>) -> String>::None,
  );

  assert_eq!(filename, "umd/main.js");
}

#[test]
fn format_placeholder_with_hash() {
  let filename_template = FilenameTemplate::new("[name]-[format]-[hash:8].js".to_string());

  let mut hash_iter = ["abcd1234"].iter();
  let hash_replacer = filename_template.has_hash_pattern().then_some(|_| hash_iter.next().unwrap());

  let filename = filename_template.render(Some("chunk"), Some("esm"), None, hash_replacer);

  assert_eq!(filename, "chunk-esm-abcd1234.js");
}

#[test]
fn format_placeholder_multiple_occurrences() {
  let filename = FilenameTemplate::new("[format]/[name]-[format].js".to_string()).render(
    Some("output"),
    Some("cjs"),
    None,
    Option::<fn(Option<usize>) -> String>::None,
  );

  assert_eq!(filename, "cjs/output-cjs.js");
}

#[test]
fn format_placeholder_with_extension() {
  let filename = FilenameTemplate::new("dist/[name]-[format][extname]".to_string()).render(
    Some("app"),
    Some("esm"),
    Some("mjs"),
    Option::<fn(Option<usize>) -> String>::None,
  );

  assert_eq!(filename, "dist/app-esm.mjs");
}
