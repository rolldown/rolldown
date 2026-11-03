use std::sync::Arc;

use rolldown_common::{ImportAttributes, ImportKind};

use super::custom_field::CustomField;

#[derive(Debug)]
pub struct HookResolveIdArgs<'a> {
  pub importer: Option<&'a str>,
  pub specifier: &'a str,
  pub is_entry: bool,
  // Rollup doesn't have a `kind` field, but rolldown supports cjs, css by default. So we need this
  // field to determine the import kind.
  pub kind: ImportKind,
  pub custom: Arc<CustomField>,
  /// The attributes of the import being resolved.
  pub attributes: &'a ImportAttributes,
  /// `importer` without its import attributes.
  pub importer_raw_id: Option<&'a str>,
  /// The import attributes of the importer itself.
  pub importer_attributes: &'a ImportAttributes,
}
