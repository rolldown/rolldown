// A pure star-re-export barrel (like victory-vendor's `export * from "d3-scale"`): no top-level
// side effects, only `export *` clauses. Its wrapper `init_*` is emitted as an empty
// `__esmMin(() => {})` unless the order machinery forwards it to the definers it re-exports.
export * from './linear.js';
export * from './pow.js';
