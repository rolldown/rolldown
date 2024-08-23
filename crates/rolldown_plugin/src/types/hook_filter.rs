use rolldown_common::ModuleType;
use rolldown_utils::pattern_filter::StringOrRegex;

#[derive(Default, Debug)]
pub struct GeneralHookFilter {
  pub include: Option<Vec<StringOrRegex>>,
  pub exclude: Option<Vec<StringOrRegex>>,
}

#[derive(Default, Debug)]
pub struct TransformHookFilter {
  pub code: Option<GeneralHookFilter>,
  pub module_type: Option<Vec<ModuleType>>,
  pub id: Option<GeneralHookFilter>,
}

#[derive(Default, Debug)]
pub struct ResolvedIdHookFilter {
  pub id: Option<GeneralHookFilter>,
}

pub type LoadHookFilter = ResolvedIdHookFilter;

#[derive(Debug)]
pub struct HookFilterOptions {
  pub load: Option<LoadHookFilter>,
  pub resolve_id: Option<ResolvedIdHookFilter>,
  pub transform: Option<TransformHookFilter>,
}
