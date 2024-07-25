use glob;

pub fn sanitize_string(s: &str) -> anyhow::Result<String> {
  if s == "" {
    return Ok(s.to_string());
  }
  if s.contains("*") {
    return Err(anyhow::format_err!("A dynamic import cannot contain * characters."));
  }
  Ok(glob::Pattern::escape(s))
}
