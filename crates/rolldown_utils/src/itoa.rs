pub use itoa::Buffer;

#[macro_export]
macro_rules! itoa {
  ($i:expr) => {{
    $crate::itoa::Buffer::new().format($i)
  }};
}
