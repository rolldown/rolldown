// Authored by @ikkz and adapted by @7086cmd.

const HEX: [char; 16] =
  ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F'];
// adapted from "https://github.com/evanw/esbuild/blob/67cbf87a4909d87a902ca8c3b69ab5330defab0a/scripts/dataurl-escapes.html" for how this was derived
pub fn encode_as_percent_escaped(buf: &[u8]) -> Option<String> {
  simdutf8::basic::from_utf8(buf)
    .map(|text| {
      let mut url = String::with_capacity(text.len() * 3);
      let chars = text.chars().collect::<Vec<_>>();
      let mut trailing_start = chars.len();
      while trailing_start > 0 {
        let c = chars[trailing_start - 1];
        if c > 0x20 as char || matches!(c, '\t' | '\n' | '\r') {
          break;
        }
        trailing_start -= 1;
      }
      for (i, &c) in chars.iter().enumerate() {
        if matches!(c, '\t' | '\n' | '\r' | '#')
          || i >= trailing_start
          || (c == '%'
            && i + 2 < chars.len()
            && chars[i + 1].is_ascii_hexdigit()
            && chars[i + 2].is_ascii_hexdigit())
        {
          url.push('%');
          url.push(HEX[c as usize >> 4]);
          url.push(HEX[c as usize & 15]);
        } else {
          url.push(c);
        }
      }
      url
    })
    .ok()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn check(raw: &str, expected: &str) {
    let result = encode_as_percent_escaped(raw.as_bytes());
    assert!(result.is_some(), "Failed to encode {raw:?}");
    assert_eq!(result.unwrap(), expected, "Test failed for input {raw:?}");
  }

  #[test]
  fn test_encode_data_url() {
    for i in 0..=0x7F {
      let always_escape = i == b'\t' || i == b'\r' || i == b'\n' || i == b'#';
      let trailing_escape = i <= 0x20 || i == b'#';

      let char_str = String::from_utf8(vec![i]).unwrap();

      if trailing_escape {
        check(&char_str, &format!("%{i:02X}"));
        check(&format!("foo{char_str}"), &format!("foo%{i:02X}"));
      } else {
        check(&format!("foo{char_str}"), &format!("foo{char_str}"));
      }

      if always_escape {
        check(&format!("{char_str}foo"), &format!("%{i:02X}foo"));
      } else {
        check(&format!("{char_str}foo"), &format!("{char_str}foo"));
      }
    }

    // Test leading vs. trailing
    check(" \t ", " %09%20");
    check(" \n ", " %0A%20");
    check(" \r ", " %0D%20");
    check(" # ", " %23%20");
    check("\x08#\x08", "\x08%23%08");

    // Only "%" symbols that could form an escape need to be escaped
    check("%, %3, %33, %333", "%, %3, %2533, %25333");
  }
}
