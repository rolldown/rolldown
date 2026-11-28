#[inline]
pub fn to_url_safe_base64(input: impl AsRef<[u8]>) -> String {
  base64_simd::URL_SAFE_NO_PAD.encode_to_string(input)
}

#[inline]
pub fn to_standard_base64(input: impl AsRef<[u8]>) -> String {
  base64_simd::STANDARD.encode_to_string(input)
}

/// `None` when `input` is not valid unpadded URL-safe base64.
#[inline]
pub fn from_url_safe_base64(input: impl AsRef<[u8]>) -> Option<Vec<u8>> {
  base64_simd::URL_SAFE_NO_PAD.decode_to_vec(input).ok()
}
