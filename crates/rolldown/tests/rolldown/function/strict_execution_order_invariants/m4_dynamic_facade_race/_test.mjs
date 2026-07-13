import assert from 'node:assert';

const { targetPromise } = await import('./dist/a.js');
await targetPromise;
assert.deepStrictEqual(
  globalThis.events,
  ['target', 'checkpoint:true'],
  'the dynamic target must initialize before the chunk checkpoint',
);
await import('./dist/b.js');

assert.deepStrictEqual(
  globalThis.events,
  ['target', 'checkpoint:true', 'observer:true'],
  'dynamic entry facade must initialize target before observer',
);
