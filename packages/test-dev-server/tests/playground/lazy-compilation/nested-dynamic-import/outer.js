export const outerName = 'outer';

// This `import('./inner.js')` runs inside outer.js, which is itself a lazy
// chunk. The HMR finalizer rewrites the call so the result looks like the
// full build's proxy (a namespace with the `'rolldown:exports'` key).
export async function loadInner() {
  return await import('./inner.js');
}
