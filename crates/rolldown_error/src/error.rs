use crate::BuildError;

pub type Result<T> = std::result::Result<T, BuildError>;

pub fn collect_results<T>(results: Vec<Result<T>>) -> (Vec<T>, Vec<BuildError>) {
  let mut errors = vec![];
  let mut values = vec![];
  results.into_iter().for_each(|result| match result {
    Ok(value) => values.push(value),
    Err(e) => errors.push(e),
  });
  (values, errors)
}
