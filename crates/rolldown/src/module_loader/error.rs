use rolldown_resolver::ResolveError;

pub enum ResolveDependenciesError {
  AnyhowError(anyhow::Error),
  ResolveError(Vec<ResolveError>),
}
