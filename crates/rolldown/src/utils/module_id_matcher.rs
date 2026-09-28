use itertools::Itertools;
use rolldown_common::MatchGroupTest;
use rolldown_error::BuildResult;

/// Evaluates a module id matcher over `ids`, in order: a regex per id, a function once with every
/// id (one call across the JS boundary), whose answer must have one entry per id. `option` names
/// the option in the error.
pub async fn match_module_ids(
  test: &MatchGroupTest,
  ids: &[&str],
  option: &str,
) -> BuildResult<Vec<bool>> {
  match test {
    MatchGroupTest::Regex(regex) => Ok(ids.iter().map(|id| regex.matches(id)).collect()),
    MatchGroupTest::Function(func) => {
      let results = func(ids.iter().map(|id| (*id).to_string()).collect_vec()).await?;
      if results.len() != ids.len() {
        return Err(
          anyhow::anyhow!("{option} returned {} results for {} modules", results.len(), ids.len())
            .into(),
        );
      }
      Ok(results)
    }
  }
}
