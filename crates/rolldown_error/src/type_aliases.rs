use crate::{BuildDiagnostic, build_error::BatchedBuildDiagnostic};

pub type SingleBuildResult<T> = std::result::Result<T, BuildDiagnostic>;

pub type BuildResult<T> = Result<T, BatchedBuildDiagnostic>;
