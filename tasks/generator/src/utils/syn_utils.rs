use syn::{Attribute, Lit};

pub fn extract_doc_comments(attrs: &[Attribute]) -> Option<String> {
  let ret = attrs
    .iter()
    .filter_map(|attr| {
      if let syn::Meta::NameValue(ref meta) = attr.meta {
        if meta.path.is_ident("doc") {
          if let syn::Expr::Lit(ref lit) = meta.value {
            if let Lit::Str(ref str) = lit.lit {
              return Some(str.value());
            }
          }
        }
      }
      None
    })
    .collect::<Vec<String>>();
  (!ret.is_empty()).then_some(ret.join("\n"))
}
