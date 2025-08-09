use rolldown_common::EsModuleFlag;

/// Combine the `es_module_flag` and whether it has a default export to determine if there need
/// to have a namespace marker in the output.
pub fn determine_es_module(es_module_flag: EsModuleFlag, has_default_export: bool) -> bool {
  match es_module_flag {
    EsModuleFlag::Always => true,
    EsModuleFlag::IfDefaultProp if has_default_export => true,
    _ => false,
  }
}

/// Render namespace markers for the module.
/// It contains the `__esModule` and `Symbol.toStringTag` properties.
/// Since rolldown doesn't support `generatedCode.symbol` yet,
/// it's not possible to use `Symbol.toStringTag` in the output.
pub fn render_namespace_markers(
  es_module_flag: EsModuleFlag,
  has_default_export: bool,
  namespace_to_string_tag: bool,
) -> Option<&'static str> {
  let es_module = determine_es_module(es_module_flag, has_default_export);
  if es_module && namespace_to_string_tag {
    Some(
      "Object.defineProperties(exports, { __esModule: { value: true }, [Symbol.toStringTag]: { value: 'Module' } });",
    )
  } else if es_module {
    Some("Object.defineProperty(exports, '__esModule', { value: true });")
  } else if namespace_to_string_tag {
    Some("Object.defineProperty(exports, Symbol.toStringTag, { value: 'Module' });")
  } else {
    None
  }
}
