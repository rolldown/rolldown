import assert from 'node:assert';

// The build cannot inline this specifier.
globalThis.__nonLiteralSpecifier = 'node:path';
const ns = await import.defer(globalThis.__nonLiteralSpecifier);
assert.strictEqual(typeof ns.join, 'function');
delete globalThis.__nonLiteralSpecifier;
