use std::{borrow::Cow, sync::LazyLock};

use cow_utils::CowUtils;
use regex::Regex;
use rustc_hash::FxHashSet;

use super::html_tag::{AttrValue, HtmlTagChildren, HtmlTagDescriptor};

// Regex patterns for HTML injection
static HEAD_INJECT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([ \t]*)</head>").unwrap());
static HEAD_PREPEND_INJECT_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"([ \t]*)<head[^>]*>").unwrap());

static BODY_PREPEND_INJECT_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"([ \t]*)<body[^>]*>").unwrap());

static DOCTYPE_PREPEND_INJECT_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?i)<!doctype html>").unwrap());

static HTML_PREPEND_INJECT_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"([ \t]*)<html[^>]*>").unwrap());

// Unary/void tags that don't have closing tags
// Aligned with Vite's implementation
static UNARY_TAGS: LazyLock<FxHashSet<&'static str>> =
  LazyLock::new(|| FxHashSet::from_iter(["link", "meta", "base"]));

/// Increment indentation - adds a tab if indent uses tabs, otherwise adds two spaces
fn increment_indent(indent: &str) -> String {
  if indent.is_empty() {
    return "  ".to_string();
  }

  // Check if the first character is a tab
  rolldown_utils::concat_string!(indent, if indent.starts_with('\t') { "\t" } else { "  " })
}

/// Serialize attributes to string
/// Aligned with Vite's serializeAttrs function
fn serialize_attrs(attrs: Option<&rustc_hash::FxHashMap<&'static str, AttrValue>>) -> String {
  let Some(attrs) = attrs else {
    return String::new();
  };
  attrs
    .iter()
    .map(|(key, value)| {
      match value {
        AttrValue::String(s) => {
          // Escape HTML entities in attribute values
          rolldown_utils::concat_string!(
            " ",
            key,
            "=\"",
            s.cow_replace('&', "&amp;")
              .cow_replace('"', "&quot;")
              .cow_replace('<', "&lt;")
              .cow_replace('>', "&gt;"),
            "\""
          )
        }
        AttrValue::Boolean(true) => {
          rolldown_utils::concat_string!(" ", key)
        }
        AttrValue::Boolean(false) | AttrValue::Undefined => String::new(),
      }
    })
    .collect()
}

/// Serialize a single HTML tag to string
/// Aligned with Vite's serializeTag function
fn serialize_tag(tag: &HtmlTagDescriptor, indent: &str) -> String {
  let attrs_str = serialize_attrs(tag.attrs.as_ref());

  if UNARY_TAGS.contains(tag.tag) {
    // Unary tags: <tag attrs>
    rolldown_utils::concat_string!("<", tag.tag, attrs_str, ">")
  } else {
    // Normal tags: <tag attrs>children</tag>
    rolldown_utils::concat_string!(
      "<",
      tag.tag,
      attrs_str,
      ">",
      match &tag.children {
        Some(HtmlTagChildren::String(s)) => Cow::Borrowed(s.as_str()),
        Some(HtmlTagChildren::Tags(tags)) if !tags.is_empty() => Cow::Owned(
          tags
            .iter()
            .map(|tag| rolldown_utils::concat_string!(indent, serialize_tag(tag, indent)))
            .collect::<Vec<_>>()
            .join("\n")
        ),
        _ => Cow::Borrowed(""),
      },
      "</",
      tag.tag,
      ">"
    )
  }
}

/// Serialize multiple HTML tags to string with proper indentation
/// Aligned with Vite's serializeTags function (when called with array of tags)
fn serialize_tags(tags: &[HtmlTagDescriptor], indent: &str) -> String {
  tags
    .iter()
    .map(|tag| rolldown_utils::concat_string!(indent, serialize_tag(tag, indent)))
    .collect::<Vec<_>>()
    .join("\n")
}

/// Fallback for prepending when no head tag is present
/// Aligned with Vite's prependInjectFallback function
fn prepend_inject_fallback<'a>(html: &'a str, tags: &[HtmlTagDescriptor]) -> Cow<'a, str> {
  // prepend to the html tag, append after doctype, or the document start
  if HTML_PREPEND_INJECT_RE.is_match(html) {
    return HTML_PREPEND_INJECT_RE.replace(html, |caps: &regex::Captures| {
      rolldown_utils::concat_string!(&caps[0], "\n", serialize_tags(tags, ""))
    });
  }

  if DOCTYPE_PREPEND_INJECT_RE.is_match(html) {
    return DOCTYPE_PREPEND_INJECT_RE.replace(html, |caps: &regex::Captures| {
      rolldown_utils::concat_string!(&caps[0], "\n", serialize_tags(tags, ""))
    });
  }

  // Last resort: prepend to the beginning
  Cow::Owned(rolldown_utils::concat_string!(serialize_tags(tags, ""), html))
}

/// Inject tags to head section
pub fn inject_to_head<'a>(
  html: &'a str,
  tags: &[HtmlTagDescriptor],
  prepend: bool,
) -> Cow<'a, str> {
  if tags.is_empty() {
    return Cow::Borrowed(html);
  }

  if prepend {
    // inject as the first element of head
    if HEAD_PREPEND_INJECT_RE.is_match(html) {
      return HEAD_PREPEND_INJECT_RE.replace(html, |caps: &regex::Captures| {
        rolldown_utils::concat_string!(
          &caps[0],
          "\n",
          serialize_tags(tags, &increment_indent(&caps[1]))
        )
      });
    }
  } else {
    // inject before head close
    if HEAD_INJECT_RE.is_match(html) {
      return HEAD_INJECT_RE.replace(html, |caps: &regex::Captures| {
        serialize_tags(tags, &increment_indent(&caps[1])) + &caps[0]
      });
    }

    // try to inject before the body tag
    if BODY_PREPEND_INJECT_RE.is_match(html) {
      return BODY_PREPEND_INJECT_RE
        .replace(html, |caps: &regex::Captures| serialize_tags(tags, &caps[1]) + "\n" + &caps[0]);
    }
  }

  // if no head tag is present, we prepend the tag for both prepend and append
  prepend_inject_fallback(html, tags)
}
