import { test } from './module_with_conflict.js';
import { a } from './other.js';

// Test: nested symbols don't get renamed when they don't match canonical names.
//
// - module_with_conflict.js: top-level `a` → renamed to `a$1`
// - module_with_conflict.js: nested parameter `a` keeps original name
// - Because nested `a` doesn't match canonical name `a$1`, it's not processed
console.log(a, test('x'));
