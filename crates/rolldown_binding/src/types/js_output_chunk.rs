// use std::collections::HashMap;

// use derivative::Derivative;
// use serde::Deserialize;

// use super::binding_rendered_module::BindingRenderedModule;

// #[napi_derive::napi(object)]
// #[derive(Deserialize, Default, Derivative)]
// #[serde(rename_all = "camelCase")]
// #[derivative(Debug)]
// pub struct JsOutputChunk {
//   // PreRenderedChunk
//   pub is_entry: bool,
//   pub is_dynamic_entry: bool,
//   pub facade_module_id: Option<String>,
//   pub module_ids: Vec<String>,
//   pub exports: Vec<String>,
//   // RenderedChunk
//   pub file_name: String,
//   pub modules: HashMap<String, BindingRenderedModule>,
//   pub imports: Vec<String>,
//   pub dynamic_imports: Vec<String>,
//   // OutputChunk
//   pub code: String,
//   pub map: Option<String>,
//   pub sourcemap_file_name: Option<String>,
// }
