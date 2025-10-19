use std::fmt::Write as _;

/// A hierarchical structure for building snapshot content with automatic heading level management
#[derive(Default)]
pub struct SnapshotSection {
  pub title: Option<String>,
  pub content: String,
  pub children: Vec<SnapshotSection>,
}

impl SnapshotSection {
  pub fn new() -> Self {
    Self::default()
  }

  /// - Root section doesn't have a title and content, so its children will begin with heading level 1
  pub fn root() -> Self {
    Self { title: None, content: String::new(), children: Vec::new() }
  }

  pub fn with_title(title: impl Into<String>) -> Self {
    Self { title: Some(title.into()), content: String::new(), children: Vec::new() }
  }

  pub fn add_title(&mut self, title: impl Into<String>) {
    self.title = Some(title.into());
  }

  pub fn add_content(&mut self, content: &str) {
    self.content.push_str(content);
  }

  pub fn add_child(&mut self, child: SnapshotSection) {
    self.children.push(child);
  }

  pub fn render(&self) -> String {
    self.render_with_level(1)
  }

  pub fn render_with_level(&self, heading_level: usize) -> String {
    let mut ret = String::new();
    self.render_inner(&mut ret, heading_level);
    ret.trim().to_owned()
  }

  fn render_inner(&self, ret: &mut String, heading_level: usize) {
    if let Some(title) = &self.title {
      let heading_prefix = "#".repeat(heading_level);
      writeln!(ret, "{heading_prefix} {title}").unwrap();
      if !self.content.is_empty() || !self.children.is_empty() {
        writeln!(ret).unwrap();
      }
    }

    if !self.content.is_empty() {
      writeln!(ret, "{}", &self.content).unwrap();
      if !self.children.is_empty() {
        writeln!(ret).unwrap();
      }
    }

    for (i, child) in self.children.iter().enumerate() {
      let child_heading_level =
        if self.title.is_some() { heading_level + 1 } else { heading_level };
      child.render_inner(ret, child_heading_level);
      let is_last_one = i == self.children.len() - 1;
      if !is_last_one {
        writeln!(ret).unwrap();
      }
    }
  }
}
