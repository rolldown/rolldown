use napi::Either;
use rolldown::InjectImport;
use serde::Deserialize;

pub type BindingInjectImport = Either<BindingInjectImportNamed, BindingInjectImportNamespace>;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingInjectImportNamed {
  #[napi(ts_type = "true")]
  pub tag_named: bool,
  pub imported: String,
  pub alias: Option<String>,
  pub from: String,
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingInjectImportNamespace {
  #[napi(ts_type = "true")]
  pub tag_namespace: bool,
  pub alias: String,
  pub from: String,
}

pub fn normalize_binding_inject_import(item: BindingInjectImport) -> InjectImport {
  match item {
    Either::A(named) => {
      InjectImport::Named { imported: named.imported, alias: named.alias, from: named.from }
    }
    Either::B(namespace) => {
      InjectImport::Namespace { alias: namespace.alias, from: namespace.from }
    }
  }
}
