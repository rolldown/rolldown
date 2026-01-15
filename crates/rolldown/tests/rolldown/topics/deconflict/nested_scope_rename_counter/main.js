import { a } from './other.js';
import { test } from './module_with_nested.js';

// Test: nested symbols keep original names when no same-module top-level conflict.
//
// - other.js: top-level `a` (keeps name)
// - module_with_nested.js: no top-level `a`, only function `test`
// - Nested parameters `a` and `a$1` keep their names because they don't
//   match any canonical name in top_level_canonical_names = {"test"}
console.log(a, test('x', 'y'));
