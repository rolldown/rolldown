use std::fmt::Write as _;

pub fn uuid_v4_string_from_u128(u: u128) -> String {
  let mut bytes = u.to_le_bytes();
  let mut uuid = String::with_capacity(36);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  for (i, byte) in bytes.iter().enumerate() {
    if i == 4 || i == 6 || i == 8 || i == 10 {
      uuid.push('-');
    }
    write!(uuid, "{byte:02x}").unwrap();
  }
  uuid
}
