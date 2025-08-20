use crate::MagicString;

pub struct JoinerOptions {
  pub separator: Option<String>,
}

#[derive(Default)]
pub struct Joiner<'s> {
  sources: Vec<MagicString<'s>>,
  separator: Option<String>,
}

impl<'s> Joiner<'s> {
  // --- public
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_options(options: JoinerOptions) -> Self {
    Self { separator: options.separator, ..Default::default() }
  }

  pub fn append(&mut self, source: MagicString<'s>) -> &mut Self {
    self.sources.push(source);
    self
  }

  pub fn append_raw(&mut self, raw: &'s str) -> &mut Self {
    self.sources.push(MagicString::new(raw));
    self
  }

  pub fn len(&self) -> usize {
    self.fragments().map(|s| s.len()).sum()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn join(&self) -> String {
    let mut ret = String::with_capacity(self.len());
    self.fragments().for_each(|frag| {
      ret.push_str(frag);
    });
    ret
  }

  // --- private

  fn fragments(&'s self) -> impl Iterator<Item = &'s str> {
    let mut iter =
      self.sources.iter().flat_map(|c| self.separator.as_deref().into_iter().chain(c.fragments()));
    // Drop the first separator
    if self.separator.is_some() {
      iter.next();
    }
    iter
  }
}
