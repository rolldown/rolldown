import assert from 'node:assert';

// The dynamic import wraps `lazy.js` and the modules it imports.
const { star, named, value } = await import('./lazy.js');
assert.strictEqual(star() + named() + value, 'star named imported');
