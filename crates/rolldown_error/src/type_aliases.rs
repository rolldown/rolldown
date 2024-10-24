use crate::{build_error::BatchedBuildDiagnostic, BuildDiagnostic};

pub type UnaryBuildResult<T> = std::result::Result<T, BuildDiagnostic>;

pub type BuildResult<T> = Result<T, BatchedBuildDiagnostic>;
