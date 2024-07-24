use super::hook_resolve_id_extra_options::HookResolveIdExtraOptions;

#[derive(Debug)]
pub struct HookResolveDynamicImportArgs<'a> {
  pub importer: Option<&'a str>,
  pub source: &'a str,
  pub options: &'a HookResolveIdExtraOptions,
}
