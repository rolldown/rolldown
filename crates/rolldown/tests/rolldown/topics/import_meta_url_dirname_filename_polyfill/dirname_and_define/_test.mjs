import assert from 'node:assert/strict';

globalThis.define = Object.assign(() => {}, { amd: true });
const { result } = await import('./dist/main.js');

assert.deepEqual(result, { dir: 'string', file: 'string', amd: 'undefined' });
