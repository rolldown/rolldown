use string_wizard::{IndentOptions, MagicString, UpdateOptions};

pub trait MagicStringExt {
  fn overwrite(&mut self, start: u32, end: u32, content: String) -> &mut Self;
  fn indent2(&mut self, indentor: &str, exclude: &[(u32, u32)]) -> &mut Self;
}
impl<'text> MagicStringExt for MagicString<'text> {
  fn overwrite(&mut self, start: u32, end: u32, content: String) -> &mut Self {
    self.update_with(start, end, content, UpdateOptions { overwrite: true, ..Default::default() })
  }

  fn indent2(&mut self, indentor: &str, exclude: &[(u32, u32)]) -> &mut Self {
    self.indent_with(IndentOptions { indentor: Some(&indentor.repeat(2)), exclude })
  }
}
