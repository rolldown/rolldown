// Destructuring a dynamic-import namespace binding in a statement body should
// tree-shake unused exports the same as `ns.foo` member access does. Covers both
// the `.then(ns => ...)` callback param and a non-inlinable `await` binding.

// `.then` body: plain destructure, default value, and rest capture, merged with
// member access. keep a, b (default kept conservatively), c, d (member), e (rest)
// -> drop f.
import('./then_lib.js').then((ns) => {
  const { a } = ns;
  const { b = 1 } = ns;
  const { c, ...rest } = ns;
  console.log(a, b, c, ns.d, rest.e);
});

// Non-inlinable top-level await (`m` read 4x): accumulate keys across statements,
// merge with member access, and forward a re-exported key never read locally.
// keep used (destructure), reExported (re-export), member (member) -> drop unused.
const m = await import('./await_lib.js');
const { used } = m;
const { used: used2 } = m;
export const { reExported } = m;
console.log(used, used2, m.member);
