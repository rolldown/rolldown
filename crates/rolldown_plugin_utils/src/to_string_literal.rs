pub fn to_string_literal(input: &str) -> String {
  // this is just a rough estimate of the number of characters that requires escaping
  const ADDITIONAL: usize = 4;

  let mut output = String::with_capacity(input.len() + 2 + 1 + ADDITIONAL);
  output.push('"');
  for c in input.chars() {
    match c {
      '\n' => {
        output.push('\\');
        output.push('n');
      }
      '\r' => {
        output.push('\\');
        output.push('r');
      }
      '\\' | '"' => {
        output.push('\\');
        output.push(c);
      }
      _ => output.push(c),
    }
  }
  output.push('"');
  output
}

#[test]
fn test() {
  assert_eq!(to_string_literal("foo"), r#""foo""#);
  assert_eq!(to_string_literal(r#"foo"\n\rbar"#), r#""foo\"\\n\\rbar""#);
  assert_eq!(to_string_literal(r"foo\bar"), r#""foo\\bar""#);
}
