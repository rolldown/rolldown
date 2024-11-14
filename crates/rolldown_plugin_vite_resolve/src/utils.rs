use oxc_resolver::NODEJS_BUILTINS;

const NODE_BUILTIN_NAMESPACE: &str = "node:";
const NPM_BUILTIN_NAMESPACE: &str = "npm:";
const BUN_BUILTIN_NAMESPACE: &str = "bun:";

pub fn clean_url(url: &str) -> &str {
  url.find(['?', '#']).map(|pos| (&url[..pos])).unwrap_or(url)
}

pub fn is_builtin(id: &str, runtime: &str) -> bool {
  if runtime == "deno" && id.starts_with(NPM_BUILTIN_NAMESPACE) {
    return true;
  }
  if runtime == "bun" && id.starts_with(BUN_BUILTIN_NAMESPACE) {
    return true;
  }
  is_node_builtin(id)
}

fn is_node_builtin(id: &str) -> bool {
  id.starts_with(NODE_BUILTIN_NAMESPACE) || NODEJS_BUILTINS.binary_search(&id).is_ok()
}

pub fn get_extension(id: &str) -> &str {
  id.rsplit_once('.').map_or("", |(_, ext)| ext)
}

// https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Regular_expressions/Character_class_escape#w
pub fn is_regex_w_character_class(c: char) -> bool {
  c.is_ascii_alphanumeric() || c == '_'
}

pub fn is_windows_drive_path(id: &str) -> bool {
  let id_bytes = id.as_bytes();
  id_bytes.len() >= 2 && id_bytes[0].is_ascii_alphabetic() && id_bytes[1] == b':'
}
