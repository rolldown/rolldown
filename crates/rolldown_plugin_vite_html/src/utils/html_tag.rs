#![expect(dead_code)]

use rustc_hash::FxHashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagKind {
  Script,
  Link,
  Meta,
}

impl TagKind {
  pub fn as_str(&self) -> &'static str {
    match self {
      TagKind::Script => "script",
      TagKind::Link => "link",
      TagKind::Meta => "meta",
    }
  }
}

#[derive(Debug, Default, Clone)]
pub enum InjectTo {
  Head,
  Body,
  #[default]
  HeadPrepend,
  BodyPrepend,
}

impl InjectTo {
  pub fn as_str(&self) -> &'static str {
    match self {
      InjectTo::Head => "head",
      InjectTo::Body => "body",
      InjectTo::HeadPrepend => "head-prepend",
      InjectTo::BodyPrepend => "body-prepend",
    }
  }
}

/// Represents an attribute value in HTML tag descriptor
/// Corresponds to TypeScript: string | boolean | undefined
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
  String(String),
  Boolean(bool),
  Undefined,
}

/// Represents children in HTML tag descriptor
/// Corresponds to TypeScript: string | HtmlTagDescriptor[]
#[derive(Debug, Clone)]
pub enum HtmlTagChildren {
  String(String),
  Tags(Vec<HtmlTagDescriptor>),
}

/// HTML tag descriptor
/// Corresponds to TypeScript interface:
/// ```typescript
/// interface HtmlTagDescriptor {
///   tag: string
///   attrs?: Record<string, string | boolean | undefined>
///   children?: string | HtmlTagDescriptor[]
///   injectTo?: 'head' | 'body' | 'head-prepend' | 'body-prepend' // default: 'head-prepend'
/// }
/// ```
#[derive(Debug, Default, Clone)]
pub struct HtmlTagDescriptor {
  pub tag: &'static str,
  pub attrs: Option<FxHashMap<&'static str, AttrValue>>,
  pub children: Option<HtmlTagChildren>,
  pub inject_to: InjectTo,
}

impl HtmlTagDescriptor {
  pub fn new(tag: &'static str) -> Self {
    Self { tag, attrs: None, children: None, inject_to: InjectTo::default() }
  }

  pub fn with_attrs(mut self, attrs: FxHashMap<&'static str, AttrValue>) -> Self {
    self.attrs = Some(attrs);
    self
  }

  pub fn with_children(mut self, children: HtmlTagChildren) -> Self {
    self.children = Some(children);
    self
  }

  pub fn with_inject_to(mut self, inject_to: InjectTo) -> Self {
    self.inject_to = inject_to;
    self
  }
}
