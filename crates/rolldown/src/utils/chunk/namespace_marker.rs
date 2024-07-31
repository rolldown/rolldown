use rolldown_common::EsModuleType;

pub fn determine_es_module(es_module_type: &EsModuleType, has_default_export: bool) -> bool {
  match es_module_type {
    EsModuleType::Always => true,
    EsModuleType::IfDefaultProp if has_default_export => true,
    _ => false,
  }
}

fn render_marker(es_module: bool, to_string_tag: bool) -> String {
  if es_module && to_string_tag {
    "Object.defineProperties(exports, { __esModule: { value: true }, [Symbol.toStringTag]: { value: 'Module' } });"
  } else if es_module {
    "Object.defineProperty(exports, '__esModule', { value: true });"
  } else if to_string_tag {
    "Object.defineProperty(exports, Symbol.toStringTag, { value: 'Module' });"
  } else {
    ""
  }.to_string()
}

pub fn render_namespace_markers(
  es_module: &EsModuleType,
  has_default_export: bool,
  namespace_to_string_tag: bool,
) -> String {
  let es_module = determine_es_module(es_module, has_default_export);
  let result = render_marker(es_module, namespace_to_string_tag);
  if result.is_empty() {
    String::new()
  } else {
    format!("\n{result}")
  }
}
