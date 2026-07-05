import assert from 'node:assert/strict';

assert(import.meta.dirname.includes('nest'));
assert(import.meta.filename.includes('nest'));
assert(import.meta.url.includes('nest'));
