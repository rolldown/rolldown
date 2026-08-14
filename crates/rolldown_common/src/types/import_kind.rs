use std::fmt::Display;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum ImportKind {
  /// import foo from 'foo'
  Import,
  /// `import('foo')`
  DynamicImport,
  /// `require('foo')`
  Require,
  AtImport,
  /// css url import, e.g. `url(foo.png)`
  UrlImport,
  // `new URL('path', import.meta.url)`
  NewUrl,
  // `import.meta.hot.accept(...)`
  HotAccept,
}

impl ImportKind {
  #[inline]
  pub fn is_static(&self) -> bool {
    matches!(self, Self::Import | Self::Require | Self::AtImport | Self::UrlImport | Self::NewUrl)
  }
  #[inline]
  pub fn is_dynamic(&self) -> bool {
    matches!(self, Self::DynamicImport)
  }
}

impl TryFrom<&str> for ImportKind {
  type Error = String;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    Ok(match value {
      "import-statement" => Self::Import,
      "dynamic-import" => Self::DynamicImport,
      "require-call" => Self::Require,
      "import-rule" => Self::AtImport,
      "url-token" => Self::UrlImport,
      "new-url" => Self::NewUrl,
      "hot-accept" => Self::HotAccept,
      _ => return Err(format!("Invalid import kind: {value:?}")),
    })
  }
}

impl Display for ImportKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::NewUrl => write!(f, "new-url"),
      Self::HotAccept => write!(f, "hot-accept"),
      // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/ast/ast.go#L42
      Self::Import => write!(f, "import-statement"),
      Self::DynamicImport => write!(f, "dynamic-import"),
      Self::Require => write!(f, "require-call"),
      Self::AtImport => write!(f, "import-rule"),
      Self::UrlImport => write!(f, "url-token"),
    }
  }
}
