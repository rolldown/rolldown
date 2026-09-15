// When the destructured namespace can't be reasoned about statically the analysis
// must keep every export (bail to Complete) so the fix never over-shakes.

// Computed key: bundler can't know which export is read -> keep a and b.
const k = globalThis.k;
import('./computed_lib.js').then((ns) => {
  const { [k]: x } = ns;
  console.log(x);
});

// Reassigned before the destructure, which now reads a plain object rather than
// the namespace -> keep a and b.
let m = await import('./reassigned_lib.js');
m = { a: 1 };
const { a } = m;
console.log(a);
