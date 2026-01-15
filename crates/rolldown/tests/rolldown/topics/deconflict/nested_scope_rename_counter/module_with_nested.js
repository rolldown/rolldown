// Function with parameters `a` and `a$1`.
// With the optimization, nested `a` keeps its name because:
// - This module has no top-level `a`, only the function `test`
// - top_level_canonical_names = {"test"}
// - Nested `a` doesn't match, so it's not processed
// - JavaScript scoping naturally shadows other.js's `a`
export function test(a, a$1) {
  console.log(a, a$1);
  return [a, a$1];
}
