import assert from 'node:assert';
import './data.json' with { type: 'json' };
const ns = await import('./data.json', { with: { type: 'json' } });
assert.strictEqual(ns.default.k, 1);
