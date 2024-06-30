use urlencoding::encode;

pub fn url_encode(s: &str) -> String {
  encode(s).to_string()
}
