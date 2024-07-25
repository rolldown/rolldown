use glob;

pub fn sanitize_string(s: &str) -> String {
  if s == "" {
    return s.to_string();
  }
  if s.contains("*") {
    panic!("A dynamic import cannot contain * characters.");
  }
  glob::Pattern::escape(s)
}
