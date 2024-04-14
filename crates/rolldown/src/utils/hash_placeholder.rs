use once_cell::sync::Lazy;
use regex::Regex;

const HASH_PLACEHOLDER_LEFT: &'static str = "!~{";
const HASH_PLACEHOLDER_RIGHT: &'static str = "}~";
const HASH_PLACEHOLDER_OVERHEAD: usize = HASH_PLACEHOLDER_LEFT.len() + HASH_PLACEHOLDER_RIGHT.len();

// This is the size of a 128-bits xxhash with base64url encoding
const MAX_HASH_SIZE: usize = 22;
const DEFAULT_HASH_SIZE: usize = 8;

static REPLACER_REGEX: Lazy<Regex> = Lazy::new(|| {
  Regex::new(&format!(
    r#"{}[0-9a-zA-Z_$]{{1,{}}}{}"#,
    HASH_PLACEHOLDER_LEFT,
    MAX_HASH_SIZE - HASH_PLACEHOLDER_OVERHEAD,
    HASH_PLACEHOLDER_RIGHT
  ))
  .expect("failed to compile regex")
});

const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";
const BASE: u32 = 64;

fn to_base64(mut value: u32) -> String {
  let mut out_string = String::new();
  loop {
    let current_digit = value % BASE;
    value /= BASE;
    out_string.push(CHARS[current_digit as usize] as char);
    if value == 0 {
      break;
    }
  }
  out_string
}

#[test]
fn test_to_base64() {
  assert_eq!(to_base64(0), "0");
  assert_eq!(to_base64(1), "1");
  assert_eq!(to_base64(10), "a");
  assert_eq!(to_base64(64), "01");
  assert_eq!(to_base64(65), "11");
  assert_eq!(to_base64(128), "02");
  assert_eq!(to_base64(100000000), "04uZ5");
}
