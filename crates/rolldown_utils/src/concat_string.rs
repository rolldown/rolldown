// https://github.com/FaultyRAM/concat-string
//
// Copyright (c) 2017-2018 FaultyRAM
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be copied, modified, or
// distributed except according to those terms.

//! Macros for concatenating string slices into owned strings.
//!
//! This crate provides the `concat_string!` macro for efficiently concatenating string slices into
//! owned strings. `concat_string!` accepts any number of arguments that implement `AsRef<str>` and
//! creates a `String` with the appropriate capacity, without the need for format strings and their
//! associated runtime overhead.
//!
//! # Example
//!
//! ```rust
//! #[macro_use(concat_string)]
//! extern crate concat_string;
//!
//! fn main() {
//!     println!("{}", concat_string!("Hello", String::from(" "), "world"));
//! }
//! ```

#[macro_export]
/// Concatenates a series of string slices into an owned string.
///
/// This macro accepts zero or more arguments, where each argument implements `AsRef<str>`, and
/// efficiently combines their string representations into a `String` in order of declaration.
///
/// This is mainly useful for cases where the cost of parsing a format string outweighs the cost
/// of converting its arguments. Because `concat_string` avoids format strings entirely, it can
/// achieve a higher level of performance than using `format!` or other formatting utilities that
/// return a `String`.
///
/// # Example
///
/// ```rust
/// use rolldown_utils::concat_string;
///
/// println!("{}", concat_string!("Hello", String::from(" "), "world"));
/// ```
macro_rules! concat_string {
    () => { String::with_capacity(0) };
    ($($s:expr_2021),+) => {{
        use std::ops::AddAssign;
        let mut len = 0;
        $(len.add_assign(AsRef::<str>::as_ref(&$s).len());)+
        let mut buf = String::with_capacity(len);
        $(buf.push_str($s.as_ref());)+
        buf
    }};
}

#[cfg(test)]
mod tests {
  #[test]
  fn concat_string_0_args() {
    let s = concat_string!();
    assert_eq!(s, String::new());
  }

  #[test]
  fn concat_string_1_arg() {
    let s = concat_string!("foo");
    assert_eq!(s, String::from("foo"));
  }

  #[test]
  fn concat_string_str_string() {
    let s = concat_string!("foo", String::from("bar"));
    assert_eq!(s, String::from("foobar"));
  }
}
