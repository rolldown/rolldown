#[derive(Debug)]
#[napi_derive::napi(object, object_to_js = false)]
pub struct ViteImportGlobMeta {
  pub is_sub_imports_pattern: Option<bool>,
}

#[derive(Debug)]
#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingVitePluginCustom {
  #[napi(js_name = "vite:import-glob")]
  pub vite_import_glob: Option<ViteImportGlobMeta>,
}
