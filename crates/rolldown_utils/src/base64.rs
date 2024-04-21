use base64::{engine::general_purpose, Engine as _};

pub fn to_url_safe_base64(input: impl AsRef<[u8]>) -> String {
  general_purpose::URL_SAFE_NO_PAD.encode(input)
}
