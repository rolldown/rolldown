use super::SourceMutation;

#[derive(Debug)]
pub struct Append {
  pub content: String,
}

impl SourceMutation for Append {
  fn apply<'me>(&'me self, _ctx: &super::Context, s: &mut string_wizard::MagicString<'me>) {
    s.append(&self.content);
  }
}
