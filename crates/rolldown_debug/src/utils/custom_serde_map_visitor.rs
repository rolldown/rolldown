use std::fmt;

use rustc_hash::FxHashMap;
use serde::ser::SerializeMap;
use tracing::field::{Field, Visit};
/// Implements `tracing_core::field::Visit` for some `serde::ser::SerializeMap`.
#[derive(Debug)]
pub struct CustomSerdeMapVisitor<'a, S: SerializeMap> {
  serializer: S,
  state: Result<(), S::Error>,
  provided_data: &'a FxHashMap<String, String>,
}

impl<'a, S> CustomSerdeMapVisitor<'a, S>
where
  S: SerializeMap,
{
  /// Create a new map visitor.
  pub fn new(serializer: S, provided_data: &'a FxHashMap<String, String>) -> Self {
    Self { serializer, state: Ok(()), provided_data }
  }

  /// Completes serializing the visited object, returning `Ok(())` if all
  /// fields were serialized correctly, or `Error(S::Error)` if a field could
  /// not be serialized.
  pub fn finish(self) -> Result<S::Ok, S::Error> {
    self.state?;
    self.serializer.end()
  }

  /// Completes serializing the visited object, returning ownership of the underlying serializer
  /// if all fields were serialized correctly, or `Err(S::Error)` if a field could not be
  /// serialized.
  pub fn take_serializer(self) -> Result<S, S::Error> {
    self.state?;
    Ok(self.serializer)
  }
}

/// Implements `tracing_core::field::Visit` for some `serde::ser::SerializeMap`.
impl<'a, S> Visit for CustomSerdeMapVisitor<'a, S>
where
  S: SerializeMap,
{
  #[cfg(all(tracing_unstable))]
  fn record_value(&mut self, field: &Field, value: valuable::Value<'_>) {
    use super::serializable_overlay::SerializableOverlay;

    if self.state.is_ok() {
      self.state = self.serializer.serialize_entry(field.name(), &SerializableOverlay::new(value));
    }
  }

  fn record_bool(&mut self, field: &Field, value: bool) {
    // If previous fields serialized successfully, continue serializing,
    // otherwise, short-circuit and do nothing.
    if self.state.is_ok() {
      self.state = self.serializer.serialize_entry(field.name(), &value);
    }
  }

  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    if self.state.is_ok() {
      self.state = self.serializer.serialize_entry(field.name(), &format_args!("{:?}", value));
    }
  }

  fn record_u64(&mut self, field: &Field, value: u64) {
    if self.state.is_ok() {
      self.state = self.serializer.serialize_entry(field.name(), &value);
    }
  }

  fn record_i64(&mut self, field: &Field, value: i64) {
    if self.state.is_ok() {
      self.state = self.serializer.serialize_entry(field.name(), &value);
    }
  }

  fn record_f64(&mut self, field: &Field, value: f64) {
    if self.state.is_ok() {
      self.state = self.serializer.serialize_entry(field.name(), &value);
    }
  }

  fn record_str(&mut self, field: &Field, value: &str) {
    if self.state.is_ok() {
      if field.name() == "action" {
        let mut json_value = serde_json::from_str::<serde_json::Value>(value).unwrap();
        if let serde_json::Value::Object(obj) = &mut json_value {
          obj.iter_mut().for_each(|(key, value)| {
            println!("key: {:?}, value: {:?} {:?}", key, value, self.provided_data);
            if let serde_json::Value::String(value) = value {
              // Check if the value is a placeholder for provided data
              if value.starts_with("${") && value.ends_with("}") {
                let inject_key = &value[2..value.len() - 1];
                println!("inject_key: {}", inject_key);
                if let Some(provided_value) = self.provided_data.get(inject_key) {
                  println!("replacing value: {} with provided value: {}", value, provided_value);
                  *value = provided_value.clone();
                }
              }
            }
          });
          println!("json_value: {:?}", json_value);
        }
        self.state = self.serializer.serialize_entry(field.name(), &json_value);
      } else {
        self.state = self.serializer.serialize_entry(field.name(), &value);
      }
    }
  }
}

struct ContextDataInjector<'a> {
  provided_data: &'a FxHashMap<String, String>,
}

impl<'a> ContextDataInjector<'a> {
  fn new(provided_data: &'a FxHashMap<String, String>) -> Self {
    Self { provided_data }
  }
}

impl valuable::Visit for ContextDataInjector<'_> {
  fn visit_value(&mut self, value: valuable::Value<'_>) {
    match value {
      valuable::Value::Mappable(mappable) => {}
      valuable::Value::Structable(structable) => todo!(),
      valuable::Value::Enumerable(enumerable) => todo!(),
      valuable::Value::Tuplable(tuplable) => todo!(),
      valuable::Value::Unit => todo!(),
      _ => todo!(),
    }
  }
}
