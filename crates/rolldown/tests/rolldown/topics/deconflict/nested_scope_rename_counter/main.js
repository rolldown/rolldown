import { a } from './other.js';
import { test } from './module_with_nested.js';

// Entry module does NOT have nested 'a', so other.js's 'a' keeps original name.
// module_with_nested.js has nested 'a' and 'a$1'.
// Bug: nested 'a' shadows top-level 'a' (different owner), gets renamed to 'a$1',
// but original 'a$1' also keeps its name -> duplicate 'a$1'!
console.log(a, test('x', 'y'));
