// This module has:
// - Top-level `a` that conflicts with other.js's `a` → renamed to `a$1`
// - Nested parameter `a` in test()
//
// With the optimization, nested `a` is NOT renamed because:
// - top_level_canonical_names = {"a$1"}
// - Nested `a` doesn't match any canonical name, so it's skipped
// - JavaScript's natural scoping handles the shadowing correctly
const a = 'from-this-module';

export function test(a) {
  return a + '-test';
}

console.log(a);