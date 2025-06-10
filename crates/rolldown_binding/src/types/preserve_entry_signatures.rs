use rolldown::PreserveEntrySignatures;

#[napi_derive::napi]
#[derive(Debug)]
pub enum BindingPreserveEntrySignatures {
  Bool(bool),
  String(String),
}

impl TryFrom<BindingPreserveEntrySignatures> for PreserveEntrySignatures {
  type Error = napi::Error;

  fn try_from(value: BindingPreserveEntrySignatures) -> Result<Self, Self::Error> {
    match value {
      BindingPreserveEntrySignatures::String(str) => match str.as_str() {
        "exports-only" => Ok(PreserveEntrySignatures::ExportsOnly),
        "strict" => Ok(PreserveEntrySignatures::Strict),
        "allow-extension" => Ok(PreserveEntrySignatures::AllowExtension),
        _ => Err(napi::Error::new(
          napi::Status::GenericFailure,
          format!(
            "Invalid value for `preserveEntrySignatures` option: {str}, expected one of 'exports-only', 'strict', 'allow-extension', or false"
          ),
        )),
      },
      BindingPreserveEntrySignatures::Bool(bool) => {
        if bool {
          Err(napi::Error::new(
            napi::Status::GenericFailure,
            format!(
              "Invalid value for `preserveEntrySignatures` option: {bool}, expected one of 'exports-only', 'strict', 'allow-extension', or false"
            ),
          ))
        } else {
          Ok(PreserveEntrySignatures::False)
        }
      }
    }
  }
}
