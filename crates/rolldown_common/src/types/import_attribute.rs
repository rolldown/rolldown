use std::fmt::Display;

use oxc::{
  allocator::{GetAllocator, Vec as ArenaVec},
  ast::{ast, builder::GetAstBuilder},
  span::SPAN,
};
use oxc_str::{CompactStr, Ident, Str};
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
            ImportAttributeKey::String(string_literal.value.into())
          }
        };
        (key, entry.value.value.to_string())
      })
      .collect();
    Self { kind, entries }
  }

  /// Rebuild the clause as AST, for a generated import standing in for the record this came
  /// from. The attributes are part of the request — a host resolves and validates the module
  /// differently without them — so a synthesized import has to carry them too.
  pub fn to_with_clause<'ast, B: GetAstBuilder<'ast> + GetAllocator<'ast>>(
    &self,
    builder: &B,
  ) -> ast::WithClause<'ast> {
    let keyword = match self.kind {
      ImportAttributeKind::With => ast::WithClauseKeyword::With,
      ImportAttributeKind::Assert => ast::WithClauseKeyword::Assert,
    };
    let with_entries = ArenaVec::from_iter_in(
      self.entries.iter().map(|(key, value)| {
        let key = match key {
          ImportAttributeKey::Identifier(name) => ast::ImportAttributeKey::new_identifier(
            SPAN,
            Ident::from_str_in(name.as_str(), builder),
            builder,
          ),
          ImportAttributeKey::String(name) => ast::ImportAttributeKey::new_string_literal(
            SPAN,
            Str::from_str_in(name.as_str(), builder),
            None,
            builder,
          ),
        };
        ast::ImportAttribute::new(
          SPAN,
          key,
          ast::StringLiteral::new(SPAN, Str::from_str_in(value, builder), None, builder),
          builder,
        )
      }),
      builder,
    );
    ast::WithClause::new(SPAN, keyword, with_entries, builder)
  }
}

impl Display for ImportAttribute {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let with_entries = self
      .entries
      .iter()
      .map(|(key, value)| match key {
        ImportAttributeKey::String(s) => format!("\"{s}\": \"{value}\""),
        ImportAttributeKey::Identifier(id) => format!("{id}: \"{value}\""),
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
