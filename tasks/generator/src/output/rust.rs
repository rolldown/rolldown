use std::{
  io::Write,
  process::{Command, Stdio},
  sync::LazyLock,
};

use oxc::span::Span;
use proc_macro2::TokenStream;
use regex::{Captures, Regex, Replacer};
use syn::parse2;

use super::add_header;

static COMMENT_REGEX: LazyLock<Regex> =
  std::sync::LazyLock::new(|| Regex::new(r"[ \t]*//[/!]@(.*)").unwrap());

/// Format Rust code, and add header.
pub fn print_rust(tokens: TokenStream, generator_path: &str) -> String {
  let code = prettyplease::unparse(&parse2(tokens).unwrap());
  let code = COMMENT_REGEX.replace_all(&code, CommentReplacer).to_string();
  let code = add_header(&code, generator_path, "//");
  rust_fmt(&code)
}

/// Format Rust code with `rustfmt`.
///
/// Does not format on disk - interfaces with `rustfmt` via stdin/stdout.
pub fn rust_fmt(source_text: &str) -> String {
  let mut rustfmt = Command::new("rustfmt")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .expect("Failed to run `rustfmt` (is it installed?)");

  let stdin = rustfmt.stdin.as_mut().unwrap();
  stdin.write_all(source_text.as_bytes()).unwrap();
  stdin.flush().unwrap();

  let output = rustfmt.wait_with_output().unwrap();
  String::from_utf8(output.stdout).unwrap()
}

pub fn ecma_fmt(source_text: &str, path: &str) -> String {
  let npx = if cfg!(target_os = "windows") { "npx.cmd" } else { "npx" };
  let mut npx = Command::new(npx)
    .args(["prettier", "--stdin-filepath", path])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .expect("Failed to run `npx prettier` (is it installed?)");

  let stdin = npx.stdin.as_mut().unwrap();
  stdin.write_all(source_text.as_bytes()).unwrap();
  stdin.flush().unwrap();

  let output = npx.wait_with_output().unwrap();
  String::from_utf8(output.stdout).unwrap()
}

pub fn replace_range_string(original: &str, span: Span, replacement: &str) -> String {
  let prefix = &original[..span.start as usize]; // Get the part before the span
  let suffix = &original[span.end as usize..]; // Get the part after the span
  format!("{prefix}{replacement}{suffix}") // Concatenate prefix, replacement, and suffix
}

/// Replace doc comments which start with `@` with plain comments or line breaks.
///
/// Original comment can be either `///@` or `//!@`.
///
/// * `///@ foo` becomes `// foo`.
/// * `//!@ foo` becomes `// foo`.
/// * `///@@line_break` is removed - i.e. line break.
/// * `//!@@line_break` is removed - i.e. line break.
///
/// `quote!` macro ignores plain comments, but we can use these to generate plain comments
/// in generated code.
///
/// `//!@` form can be used to insert a line break in a position where `///@ ...`
/// is not valid syntax e.g. before an `#![expect(...)]`.
///
/// To dynamically generate a comment:
/// ```no_run
/// let name = "foo";
/// let comment = format!("@ NOTE: {} doesn't exist!", name);
/// // quote!( #[doc = #comment] )
/// // or `quote!( #![doc = #comment] )`
/// ```
struct CommentReplacer;

impl Replacer for CommentReplacer {
  fn replace_append(&mut self, caps: &Captures, dst: &mut String) {
    assert_eq!(caps.len(), 2);
    let body = caps.get(1).unwrap().as_str();
    if body != "@line_break" {
      dst.push_str("//");
      dst.push_str(body);
    }
  }
}
