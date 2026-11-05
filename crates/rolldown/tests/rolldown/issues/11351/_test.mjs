import assert from 'node:assert';

const { ns } = await import('./dist/other.js');
assert.strictEqual(ns.a, 1);
const { extra } = await import('./dist/secondary.js');
assert.strictEqual(extra, 3);
const { load } = await import('./dist/index.js');
assert.strictEqual((await load()).extra, 3);
