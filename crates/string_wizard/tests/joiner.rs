use string_wizard::{Joiner, JoinerOptions, MagicString};
mod append {
  use super::*;

  #[test]
  fn should_append_content() {
    let mut j = Joiner::default();
    j.append(MagicString::new("*"));
    j.append_raw("123").append_raw("456");
    assert_eq!(j.join(), "*123456");
  }
}

#[test]
fn separator() {
  let mut j = Joiner::with_options(JoinerOptions { separator: Some(",".to_string()) });
  j.append_raw("123");
  assert_eq!(j.join(), "123");
  j.append_raw("123");
  assert_eq!(j.join(), "123,123");
  j.append_raw("123");
  assert_eq!(j.join(), "123,123,123");
}
