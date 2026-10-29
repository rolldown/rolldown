use indexmap::IndexMap;
use rolldown_common::ImportAttributes;

/// Import attributes as a JS `Record<string, string>`.
pub type BindingImportAttributes = IndexMap<String, String>;

/// `None` without attributes, so a hook call for a module without them sends nothing extra to JS.
pub fn to_binding_import_attributes(
  attributes: &ImportAttributes,
) -> Option<BindingImportAttributes> {
  (!attributes.is_empty())
    .then(|| attributes.iter().map(|(key, value)| (key.to_owned(), value.to_owned())).collect())
}

pub fn from_binding_import_attributes(attributes: BindingImportAttributes) -> ImportAttributes {
  ImportAttributes::from_entries(attributes)
}
