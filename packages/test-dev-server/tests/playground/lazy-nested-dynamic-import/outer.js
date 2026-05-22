export const outerName = 'outer';

// Nested dynamic import: this `import('./inner.js')` runs inside outer.js's body,
// which is itself a lazy chunk. After the lazy-compilation plugin resolves the
// dynamic import to `inner.js?rolldown-lazy=1`, the HMR AST finalizer rewrites
// this call so the result mirrors the full-build proxy contract (a namespace
// with the `'rolldown:exports'` key for `__unwrap_lazy_compilation_entry`).
export async function loadInner() {
  return await import('./inner.js');
}
