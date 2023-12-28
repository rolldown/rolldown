use derivative::Derivative;
use napi::JsFunction;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct PluginOptions {
  pub name: String,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: PluginContext) => Promise<void>")]
  pub build_start: Option<JsFunction>,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: PluginContext, specifier: string, importer?: string, options?: HookResolveIdArgsOptions) => Promise<undefined | ResolveIdResult>"
  )]
  pub resolve_id: Option<JsFunction>,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: PluginContext, id: string) => Promise<undefined | SourceResult>")]
  pub load: Option<JsFunction>,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(ctx: TransformPluginContext, id: string, code: string) => Promise<undefined | SourceResult>"
  )]
  pub transform: Option<JsFunction>,

  #[derivative(Debug = "ignore")]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(ctx: PluginContext, error: string) => Promise<void>")]
  pub build_end: Option<JsFunction>,
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct HookResolveIdArgsOptions {
  pub is_entry: bool,
  pub kind: String,
}

impl From<rolldown::HookResolveIdArgsOptions> for HookResolveIdArgsOptions {
  fn from(value: rolldown::HookResolveIdArgsOptions) -> Self {
    Self { is_entry: value.is_entry, kind: value.kind.to_string() }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct ResolveIdResult {
  pub id: String,
  pub external: Option<bool>,
}

impl From<ResolveIdResult> for rolldown::HookResolveIdOutput {
  fn from(value: ResolveIdResult) -> Self {
    Self { id: value.id, external: value.external }
  }
}

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct SourceResult {
  pub code: String,
}

impl From<SourceResult> for rolldown::HookLoadOutput {
  fn from(value: SourceResult) -> Self {
    Self { code: value.code }
  }
}
