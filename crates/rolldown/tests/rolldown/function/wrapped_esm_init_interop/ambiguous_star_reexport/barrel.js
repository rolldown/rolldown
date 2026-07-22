// Both leaves export `x`, so `x` is an ambiguous export on the barrel (resolves to undefined). The
// ambiguous owners still have observable side effects that must run.
export * from './mod_a.js';
export * from './mod_b.js';
// Dynamic exports keep main's `export *` statement included so its IsExportStar collector path is
// finalized; this module intentionally exports no `x` of its own.
export * from './dynamic.cjs';
