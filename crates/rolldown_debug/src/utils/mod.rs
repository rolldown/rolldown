use tracing_subscriber::registry::LookupSpan;

use crate::debug_data_propagate_layer::ProvidedData;

pub mod custom_serde_map_visitor;
pub mod serializable_overlay;
pub mod serializer_overlay;

// pub fn inject_data(key: &'static str) -> String {
//   let current_span = tracing::Span::current();
//   let span_id = current_span.id().unwrap_or_else(|| panic!("Got a span without an ID"));
//   let mut found_data = None;
//   if let Some(registry) = Dispatch::get_default().downcast_ref::<tracing_subscriber::Registry>() {}

//   found_data.unwrap_or_else(|| panic!("Fail to get injected with key: {}", key))
// }
