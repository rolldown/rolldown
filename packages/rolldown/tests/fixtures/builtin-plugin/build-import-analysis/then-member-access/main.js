// Test various .then((m) => m.prop) patterns
import assert from 'node:assert';

// Pattern 1: With await keyword
const a = await import('./lib.js').then((m) => m.foo);

// Pattern 2: Without await keyword - just the promise chain
const promiseB = import('./lib.js').then((m) => m.bar);

// Pattern 3: Nested property access
const c = await import('./lib.js').then((m) => m.nested.value);

// Pattern 4: Regular function (not arrow)
const d = await import('./lib.js').then(function(m) { return m.foo; });

export { a, promiseB, c, d };
