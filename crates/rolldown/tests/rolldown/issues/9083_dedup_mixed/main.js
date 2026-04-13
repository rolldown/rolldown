// Mixed re-export: both `export *` through barrel AND a direct named
// re-export of the same TLA leaf module.  This exercises the case where
// the outer wrapper would naively emit both `await init_barrel()` and
// `await init_deep()`, the latter being transitive-redundant because
// init_barrel already awaits init_deep.  The test verifies that the
// generated code is still *correct* (deep is fully initialised before
// any consumer runs) even though the dedup is not transitive-aware.
export * from './barrel.js';
export { value } from './deep.js';
