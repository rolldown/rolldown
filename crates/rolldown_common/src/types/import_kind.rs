use std::fmt::Display;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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
  pub fn is_static(&self) -> bool {
    matches!(self, Self::Import | Self::Require | Self::AtImport | Self::UrlImport | Self::NewUrl)
  }

  pub fn is_dynamic(&self) -> bool {
    matches!(self, Self::DynamicImport)
  }
}

impl TryFrom<&str> for ImportKind {
  type Error = String;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "import-statement" => Ok(Self::Import),
      "dynamic-import" => Ok(Self::DynamicImport),
      "require-call" => Ok(Self::Require),
      "import-rule" => Ok(Self::AtImport),
      "url-import" => Ok(Self::UrlImport),
      _ => Err(format!("Invalid import kind: {value:?}")),
    }
  }
}

impl Display for ImportKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/ast/ast.go#L42
    match self {
      Self::Import => write!(f, "import-statement"),
      Self::DynamicImport => write!(f, "dynamic-import"),
      Self::Require => write!(f, "require-call"),
      // TODO(hyf0): check if this literal is the same as esbuild's
      Self::AtImport => write!(f, "import-rule"),
      ImportKind::UrlImport => write!(f, "url-token"),
      ImportKind::NewUrl => write!(f, "new-url"),
      ImportKind::HotAccept => write!(f, "hot-accept"),
    }
  }
}
