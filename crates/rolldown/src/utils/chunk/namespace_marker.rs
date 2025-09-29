use rolldown_common::{EsModuleFlag, GeneratedCodeOptions};

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
/// The `Symbol.toStringTag` usage is controlled by the `generatedCode.symbols` option.
#[expect(clippy::trivially_copy_pass_by_ref)]
pub fn render_namespace_markers(
  es_module_flag: EsModuleFlag,
  has_default_export: bool,
  generated_code: &GeneratedCodeOptions,
  namespace_to_string_tag: bool,
) -> Option<&'static str> {
  let es_module = determine_es_module(es_module_flag, has_default_export);
  let use_symbols = generated_code.symbols && namespace_to_string_tag;

  if es_module && use_symbols {
    Some(
      "Object.defineProperties(exports, { __esModule: { value: true }, [Symbol.toStringTag]: { value: 'Module' } });",
    )
  } else if es_module {
    Some("Object.defineProperty(exports, '__esModule', { value: true });")
  } else if use_symbols {
    Some("Object.defineProperty(exports, Symbol.toStringTag, { value: 'Module' });")
  } else {
    None
  }
}
