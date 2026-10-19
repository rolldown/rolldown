use std::fmt::Display;

use json_escape_simd::escape;
use oxc::{ast::ast, span::Span};
use oxc_str::CompactStr;
use rolldown_utils::indexmap::FxIndexMap;

#[derive(Debug, Clone, Default)]
pub struct ImportAttribute {
  kind: ImportAttributeKind,
  entries: FxIndexMap<ImportAttributeKey, String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ImportAttributeKind {
  #[default]
  With,
  Assert,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImportAttributeKey {
  String(CompactStr),
  Identifier(CompactStr),
}

impl ImportAttribute {
  pub fn insert(&mut self, key: ImportAttributeKey, value: String) {
    self.entries.insert(key, value);
  }

  pub fn get(&self, key: &ImportAttributeKey) -> Option<&String> {
    self.entries.get(key)
  }

  pub fn contains_key(&self, key: &ImportAttributeKey) -> bool {
    self.entries.contains_key(key)
  }

  /// Fails with the span of the first key or value that has a lone surrogate, which the UTF-8
  /// entries can't hold.
  pub fn from_with_clause(with_clause: &ast::WithClause) -> Result<Self, Span> {
    let kind = match with_clause.keyword {
      ast::WithClauseKeyword::With => ImportAttributeKind::With,
      ast::WithClauseKeyword::Assert => ImportAttributeKind::Assert,
    };
    let entries = with_clause
      .with_entries
      .iter()
      .map(|entry| {
        let key = match &entry.key {
          ast::ImportAttributeKey::Identifier(identifier_name) => {
            ImportAttributeKey::Identifier(identifier_name.name.into())
          }
          ast::ImportAttributeKey::StringLiteral(string_literal) => ImportAttributeKey::String(
            string_literal.value.as_str().ok_or(string_literal.span)?.into(),
          ),
        };
        let value = entry.value.value.as_str().ok_or(entry.value.span)?;
        Ok((key, value.to_string()))
      })
      .collect::<Result<FxIndexMap<_, _>, Span>>()?;
    Ok(Self { kind, entries })
  }
}

impl Display for ImportAttribute {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let with_entries = self
      .entries
      .iter()
      .map(|(key, value)| match key {
        ImportAttributeKey::String(s) => format!("{}: {}", escape(s), escape(value)),
        ImportAttributeKey::Identifier(id) => format!("{id}: {}", escape(value)),
      })
      .collect::<Vec<_>>()
      .join(", ");

    write!(
      f,
      "{} {{ {with_entries} }}",
      match self.kind {
        ImportAttributeKind::With => "with",
        ImportAttributeKind::Assert => "assert",
      },
    )
  }
}
