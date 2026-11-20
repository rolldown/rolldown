import assert from 'node:assert';

// The comment keeps the call out of the module graph, so the output loads `node:path` when it runs.
const ns = await import.defer(/* @vite-ignore */ 'node:path');
assert.strictEqual(typeof ns.join, 'function');
