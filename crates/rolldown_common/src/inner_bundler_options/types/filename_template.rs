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

#[derive(Debug, Default)]
pub struct FileNameRenderOptions<'me> {
  pub name: Option<&'me str>,
  pub hash: Option<&'me str>,
  pub ext: Option<&'me str>,
}

impl FilenameTemplate {
  pub fn render(&self, options: &FileNameRenderOptions) -> String {
    let mut tmp = self.template.clone();
    if let Some(name) = options.name {
      tmp = tmp.replace("[name]", name);
    }
    if let Some(hash) = options.hash {
      if let Some(start) = tmp.find("[hash") {
        if let Some(end) = tmp[start + 5..].find(']') {
          tmp.replace_range(start..=start + end + 5, hash);
        }
      }
    }
    if let Some(ext) = options.ext {
      tmp = tmp.replace("[ext]", ext).replace("[extname]", &format!(".{ext}"));
    }
    tmp
  }
}

#[test]
fn basic() {
  FilenameTemplate::new("[name]-[hash:8].js".to_string());
}

#[test]
fn hash_with_len() {
  let file_template = FilenameTemplate::new("[name]-[hash:3].js".to_string());
  let str = file_template.render(&FileNameRenderOptions {
    name: Some("hello"),
    hash: Some("abc"),
    ext: None,
  });

  assert_eq!(str, "hello-abc.js");
}
