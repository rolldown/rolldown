import { test } from './module_a.js';
import { unused } from './module_b.js';

// Test verifying Copilot's concern is NOT valid.
//
// Copilot claimed: if module_a's `unused` (with no local refs) gets renamed to
// `unused$1`, and there's a nested `unused$1`, they would conflict.
//
// Reality: The renamer's `is_name_available` checks nested scope bindings when
// choosing renamed candidates, so `unused` is renamed to `unused$2` (skipping
// `unused$1` which exists in nested scope). No conflict occurs.
console.log(test('x'), unused);
