use std::fmt::Display;

use json_escape_simd::escape;
use oxc::ast::ast;
use oxc_str::CompactStr;
use rolldown_utils::{ecmascript::to_usv_str, indexmap::FxIndexMap};

use crate::ImportAttributes;

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
  /// The attributes this clause imports the module with. The legacy `assert` keyword takes no
  /// part in resolution or module identity, and only survives in the output without
  /// `experimental.importAttributes`.
  pub fn to_import_attributes(&self) -> ImportAttributes {
    match self.kind {
      ImportAttributeKind::With => {
        ImportAttributes::from_entries(self.entries.iter().map(|(key, value)| {
          let (ImportAttributeKey::String(key) | ImportAttributeKey::Identifier(key)) = key;
          (key.clone(), value.as_str())
        }))
      }
      ImportAttributeKind::Assert => ImportAttributes::default(),
    }
  }

  pub fn insert(&mut self, key: ImportAttributeKey, value: String) {
    self.entries.insert(key, value);
  }

  pub fn get(&self, key: &ImportAttributeKey) -> Option<&String> {
    self.entries.get(key)
  }

  pub fn contains_key(&self, key: &ImportAttributeKey) -> bool {
    self.entries.contains_key(key)
  }

  pub fn from_with_clause(with_clause: &ast::WithClause) -> Self {
    let kind = match with_clause.keyword {
      ast::WithClauseKeyword::With => ImportAttributeKind::With,
      ast::WithClauseKeyword::Assert => ImportAttributeKind::Assert,
    };
    let entries: FxIndexMap<ImportAttributeKey, String> = with_clause
      .with_entries
      .iter()
      .map(|entry| {
        let key = match &entry.key {
          ast::ImportAttributeKey::Identifier(identifier_name) => {
            ImportAttributeKey::Identifier(identifier_name.name.into())
          }
          ast::ImportAttributeKey::StringLiteral(string_literal) => {
            ImportAttributeKey::String(to_usv_str(string_literal.value).into())
          }
        };
        (key, to_usv_str(entry.value.value).into_owned())
      })
      .collect();
    Self { kind, entries }
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
