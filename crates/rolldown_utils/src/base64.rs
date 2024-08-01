pub fn to_url_safe_base64(input: impl AsRef<[u8]>) -> String {
  base64_simd::URL_SAFE_NO_PAD.encode_to_string(input)
}

pub fn to_standard_base64(input: impl AsRef<[u8]>) -> String {
  base64_simd::STANDARD.encode_to_string(input)
}

pub fn from_standard_base64(input: impl AsRef<[u8]>) -> anyhow::Result<Vec<u8>> {
  Ok(base64_simd::STANDARD.decode_to_vec(input)?)
}
