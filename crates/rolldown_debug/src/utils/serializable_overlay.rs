use std::fmt;

use rolldown_debug_action::PROVIDED_DATA;
use serde::Serialize;
use valuable::{Valuable, Value, Visit};
use valuable_serde::Serializable;

pub struct SerializableOverlay<V>(Serializable<V>);

pub struct MappableVisitor<T: Visit> {
  inner: T,
}

impl<T: Visit> valuable::Visit for MappableVisitor<T> {
  fn visit_value(&mut self, value: Value<'_>) {
    self.inner.visit_value(value);
  }

  fn visit_entry(&mut self, key: Value<'_>, value: Value<'_>) {
    match &key {
      Value::String(filed_value) if filed_value.starts_with("${") && filed_value.ends_with("}") => {
        let inject_key = &filed_value[2..filed_value.len() - 1];
        PROVIDED_DATA.with(|data| {
          let Some(provided) = data.get(inject_key) else {
            return;
          };
          self.inner.visit_entry(key, Value::String(provided.as_str().into()));
        })
      }
      _ => {
        self.inner.visit_entry(key, value);
      }
    }
  }
}

pub struct ValuableOverlay<V: Valuable>(pub V);

impl<V: Valuable> Valuable for ValuableOverlay<V> {
  fn as_value(&self) -> Value<'_> {
    self.0.as_value()
  }

  fn visit(&self, visit: &mut dyn Visit) {
    match self.0.as_value() {
      Value::Mappable(mappable) => {
        let mut mappable_visitor = MappableVisitor { inner: visit };
        mappable.visit(&mut mappable_visitor);
      }
      _ => {
        self.0.visit(visit);
      }
    }
  }
}

impl<V: Valuable> SerializableOverlay<V> {
  pub fn new(value: V) -> Self {
    Self(Serializable::new(value))
  }
}

impl<V> fmt::Debug for SerializableOverlay<V>
where
  V: Valuable,
{
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Debug::fmt(&self.as_value(), f)
  }
}

impl<V> Valuable for SerializableOverlay<V>
where
  V: Valuable,
{
  fn as_value(&self) -> Value<'_> {
    self.0.as_value()
  }

  fn visit(&self, visit: &mut dyn Visit) {
    self.0.visit(visit);
  }
}

impl<V> Serialize for SerializableOverlay<V>
where
  V: Valuable,
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    self.0.serialize(serializer)
  }
}
