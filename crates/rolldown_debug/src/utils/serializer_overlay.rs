use serde::ser::{Serialize, Serializer};
use serde_json::{
  Serializer as JsonSerializer,
  ser::{CompactFormatter, Formatter},
};
use std::io::Write;

/// A Serializer overlay specifically for `serde_json::Serializer`.
/// This allows intercepting or modifying serialization behavior while
/// delegating the core JSON serialization logic.
pub struct SerializerOverlay<W, F = CompactFormatter>
where
  W: Write,
  F: Formatter,
{
  inner: JsonSerializer<W, F>,
}

impl<W> SerializerOverlay<W>
where
  W: Write,
{
  /// Creates a new JSON serializer overlay using the compact formatter.
  #[inline]
  pub fn new(writer: W) -> Self {
    Self { inner: JsonSerializer::new(writer) }
  }
}

impl<W, F> SerializerOverlay<W, F>
where
  W: Write,
  F: Formatter,
{
  /// Creates a new JSON serializer overlay with a specific formatter.
  #[inline]
  pub fn with_formatter(writer: W, formatter: F) -> Self {
    Self { inner: JsonSerializer::with_formatter(writer, formatter) }
  }
}

// Implement the `Serializer` trait for a mutable reference to `SerializerOverlay`.
// This allows the overlay to be used wherever a `Serializer` is expected.
impl<'a, W, F> Serializer for &'a mut SerializerOverlay<W, F>
where
  W: Write,
  F: Formatter,
{
  // The success type is `()` because `serde_json::Serializer` methods return `Result<()>`
  type Ok = ();
  // The error type is `serde_json::Error` as that's what the inner serializer returns.
  type Error = serde_json::Error;

  // Delegate associated types directly to the inner `serde_json::Serializer`.
  // We need to qualify the path because `Serializer` is implemented for `&'a mut JsonSerializer`.
  type SerializeSeq = <&'a mut JsonSerializer<W, F> as Serializer>::SerializeSeq;
  type SerializeTuple = <&'a mut JsonSerializer<W, F> as Serializer>::SerializeTuple;
  type SerializeTupleStruct = <&'a mut JsonSerializer<W, F> as Serializer>::SerializeTupleStruct;
  type SerializeTupleVariant = <&'a mut JsonSerializer<W, F> as Serializer>::SerializeTupleVariant;
  type SerializeMap = <&'a mut JsonSerializer<W, F> as Serializer>::SerializeMap;
  type SerializeStruct = <&'a mut JsonSerializer<W, F> as Serializer>::SerializeStruct;
  type SerializeStructVariant =
    <&'a mut JsonSerializer<W, F> as Serializer>::SerializeStructVariant;

  // Delegate all serialization methods to the inner `serde_json::Serializer`.
  #[inline]
  fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_bool(v)
  }

  #[inline]
  fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_i8(v)
  }

  #[inline]
  fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_i16(v)
  }

  #[inline]
  fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_i32(v)
  }

  #[inline]
  fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_i64(v)
  }

  #[inline]
  fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_u8(v)
  }

  #[inline]
  fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_u16(v)
  }

  #[inline]
  fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_u32(v)
  }

  #[inline]
  fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_u64(v)
  }

  #[inline]
  fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_f32(v)
  }

  #[inline]
  fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_f64(v)
  }

  #[inline]
  fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_char(v)
  }

  #[inline]
  fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_str(v)
  }

  #[inline]
  fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_bytes(v)
  }

  #[inline]
  fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_none()
  }

  #[inline]
  fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
  where
    T: ?Sized + Serialize,
  {
    self.inner.serialize_some(value)
  }

  #[inline]
  fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_unit()
  }

  #[inline]
  fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_unit_struct(name)
  }

  #[inline]
  fn serialize_unit_variant(
    self,
    name: &'static str,
    variant_index: u32,
    variant: &'static str,
  ) -> Result<Self::Ok, Self::Error> {
    self.inner.serialize_unit_variant(name, variant_index, variant)
  }

  #[inline]
  fn serialize_newtype_struct<T>(
    self,
    name: &'static str,
    value: &T,
  ) -> Result<Self::Ok, Self::Error>
  where
    T: ?Sized + Serialize,
  {
    self.inner.serialize_newtype_struct(name, value)
  }

  #[inline]
  fn serialize_newtype_variant<T>(
    self,
    name: &'static str,
    variant_index: u32,
    variant: &'static str,
    value: &T,
  ) -> Result<Self::Ok, Self::Error>
  where
    T: ?Sized + Serialize,
  {
    self.inner.serialize_newtype_variant(name, variant_index, variant, value)
  }

  #[inline]
  fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
    self.inner.serialize_seq(len)
  }

  #[inline]
  fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
    self.inner.serialize_tuple(len)
  }

  #[inline]
  fn serialize_tuple_struct(
    self,
    name: &'static str,
    len: usize,
  ) -> Result<Self::SerializeTupleStruct, Self::Error> {
    self.inner.serialize_tuple_struct(name, len)
  }

  #[inline]
  fn serialize_tuple_variant(
    self,
    name: &'static str,
    variant_index: u32,
    variant: &'static str,
    len: usize,
  ) -> Result<Self::SerializeTupleVariant, Self::Error> {
    self.inner.serialize_tuple_variant(name, variant_index, variant, len)
  }

  #[inline]
  fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
    self.inner.serialize_map(len)
  }

  #[inline]
  fn serialize_struct(
    self,
    name: &'static str,
    len: usize,
  ) -> Result<Self::SerializeStruct, Self::Error> {
    self.inner.serialize_struct(name, len)
  }

  #[inline]
  fn serialize_struct_variant(
    self,
    name: &'static str,
    variant_index: u32,
    variant: &'static str,
    len: usize,
  ) -> Result<Self::SerializeStructVariant, Self::Error> {
    self.inner.serialize_struct_variant(name, variant_index, variant, len)
  }
}
