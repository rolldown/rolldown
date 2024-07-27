use super::hook_resolve_id_extra_options::HookResolveIdExtraOptions;

#[derive(Debug)]
pub struct HookResolveIdArgs<'a> {
  pub importer: Option<&'a str>,
  pub specifier: &'a str,
  pub options: HookResolveIdExtraOptions,
}
