use once_cell::sync::Lazy;

static IGNORED_PROTOCOLS: Lazy<Vec<&str>> = Lazy::new(|| {
  vec!["data:", "http:", "https:"]
});

pub fn should_ignore(glob: &String) -> bool {
  if !glob.contains("*") {
    return true;
  }

  return  IGNORED_PROTOCOLS.iter().any(|protocol| glob.starts_with(protocol));
}
