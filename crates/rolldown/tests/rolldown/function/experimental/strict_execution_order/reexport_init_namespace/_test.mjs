import assert from 'node:assert';

globalThis.reexportNamespaceHits = 0;
const { ns } = await import('./dist/main.js');

assert.strictEqual(ns.value, 1);
